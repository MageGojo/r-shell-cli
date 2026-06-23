use axum::{
    Router,
    body::Body,
    http::{
        Request, StatusCode,
        header::{HOST, ORIGIN},
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Implementation, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    result::Result as StdResult,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio_util::sync::CancellationToken;

use crate::model::SavedConnection;
use crate::native_backend::{AuthMethod, NativeConnectionManager, SshConfig};
use crate::storage;

pub const MCP_PORT: u16 = 9123;
pub const MCP_ENDPOINT_PATH: &str = "/mcp";
pub const MCP_ENDPOINT: &str = "http://127.0.0.1:9123/mcp";

static MCP_SERVER_RUNNING: AtomicBool = AtomicBool::new(false);

/// Bridge between the MCP HTTP handlers and the locally persisted workspace.
///
/// Connection-management tool calls operate directly on the persisted
/// `workspace.json`, guarded by a mutex so concurrent MCP requests serialize
/// their read-modify-write cycles.
///
/// The bridge also owns a long-lived [`NativeConnectionManager`] so that
/// `ssh_session_*` tools can keep an SSH connection alive across many tool
/// calls. This is the key to persistence: open a session once, then run
/// commands / read / write files repeatedly without a fresh TCP+SSH handshake
/// each time (which both wastes time and looks like an attack to the server).
#[derive(Default)]
pub struct McpBridge {
    lock: Mutex<()>,
    manager: Arc<NativeConnectionManager>,
}

impl McpBridge {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn auth_method_label(auth_method: &SshAuthMethod) -> &'static str {
        match auth_method {
            SshAuthMethod::Password => "password",
            SshAuthMethod::PublicKey => "publickey",
        }
    }

    fn list_connections(&self) -> StdResult<Value, String> {
        let _guard = self.lock.lock().map_err(|_| "bridge lock poisoned")?;
        let workspace = storage::load_workspace();
        let connections = workspace
            .connections
            .iter()
            .map(SavedConnection::sanitized)
            .collect::<Vec<_>>();
        Ok(json!({ "connections": connections }))
    }

    fn list_tabs(&self) -> StdResult<Value, String> {
        // The CLI build has no concept of open terminal tabs.
        Ok(json!({ "tabs": [] }))
    }

    fn create_connection(&self, params: CreateSshConnectionParams) -> StdResult<Value, String> {
        let name = params.name.trim().to_string();
        let host = params.host.trim().to_string();
        let username = params.username.trim().to_string();
        if name.is_empty() || host.is_empty() || username.is_empty() || params.port == 0 {
            return Err("name, host, username, and a valid port are required".to_string());
        }

        let folder = params.folder.trim();
        let connection = SavedConnection {
            id: SavedConnection::new_id(),
            name,
            host,
            port: params.port,
            username,
            protocol: "SSH".to_string(),
            folder: if folder.is_empty() {
                "All Connections".to_string()
            } else {
                folder.to_string()
            },
            tags: params.tags,
            description: params.description.unwrap_or_default(),
            auth_method: Self::auth_method_label(&params.auth_method).to_string(),
            password: params.password.filter(|value| !value.is_empty()),
            private_key_path: params.private_key_path.filter(|value| !value.is_empty()),
            passphrase: params.passphrase.filter(|value| !value.is_empty()),
            status: crate::model::ConnectionStatus::Disconnected,
        };

        let _guard = self.lock.lock().map_err(|_| "bridge lock poisoned")?;
        let mut workspace = storage::load_workspace();
        workspace.connections.push(connection.clone());
        workspace.active_connection_id = Some(connection.id.clone());
        storage::save_workspace(&workspace).map_err(|error| error.to_string())?;

        Ok(json!({ "connection": connection.sanitized() }))
    }

    fn update_connection(&self, params: UpdateSshConnectionParams) -> StdResult<Value, String> {
        let _guard = self.lock.lock().map_err(|_| "bridge lock poisoned")?;
        let mut workspace = storage::load_workspace();
        let Some(connection) = workspace
            .connections
            .iter_mut()
            .find(|connection| connection.id == params.connection_id)
        else {
            return Err(format!(
                "SSH connection not found: {}",
                params.connection_id
            ));
        };

        if let Some(name) = params.name.map(|value| value.trim().to_string()) {
            if !name.is_empty() {
                connection.name = name;
            }
        }
        if let Some(host) = params.host.map(|value| value.trim().to_string()) {
            if !host.is_empty() {
                connection.host = host;
            }
        }
        if let Some(username) = params.username.map(|value| value.trim().to_string()) {
            if !username.is_empty() {
                connection.username = username;
            }
        }
        if let Some(port) = params.port {
            if port == 0 {
                return Err("port must be greater than 0".to_string());
            }
            connection.port = port;
        }
        if let Some(auth_method) = params.auth_method {
            connection.auth_method = Self::auth_method_label(&auth_method).to_string();
        }
        if let Some(password) = params.password {
            connection.password = if password.is_empty() {
                None
            } else {
                Some(password)
            };
        }
        if let Some(private_key_path) = params.private_key_path {
            connection.private_key_path = if private_key_path.is_empty() {
                None
            } else {
                Some(private_key_path)
            };
        }
        if let Some(passphrase) = params.passphrase {
            connection.passphrase = if passphrase.is_empty() {
                None
            } else {
                Some(passphrase)
            };
        }
        if let Some(folder) = params.folder.map(|value| value.trim().to_string()) {
            if !folder.is_empty() {
                connection.folder = folder;
            }
        }
        if let Some(tags) = params.tags {
            connection.tags = tags;
        }
        if let Some(description) = params.description {
            connection.description = description;
        }

        let updated = connection.clone();
        storage::save_workspace(&workspace).map_err(|error| error.to_string())?;

        Ok(json!({ "connection": updated.sanitized() }))
    }

    fn delete_connection(&self, params: DeleteSshConnectionParams) -> StdResult<Value, String> {
        let _guard = self.lock.lock().map_err(|_| "bridge lock poisoned")?;
        let mut workspace = storage::load_workspace();
        let Some(index) = workspace
            .connections
            .iter()
            .position(|connection| connection.id == params.connection_id)
        else {
            return Err(format!(
                "SSH connection not found: {}",
                params.connection_id
            ));
        };

        let removed = workspace.connections.remove(index);
        let mut closed_tabs: Vec<String> = Vec::new();
        if params.close_open_tabs {
            closed_tabs = workspace
                .tabs
                .iter()
                .filter(|tab| tab.connection_id == removed.id)
                .map(|tab| tab.id.clone())
                .collect();
            workspace.tabs.retain(|tab| tab.connection_id != removed.id);
        }

        if workspace.active_connection_id.as_deref() == Some(removed.id.as_str()) {
            workspace.active_connection_id = workspace
                .connections
                .first()
                .map(|connection| connection.id.clone());
        }
        storage::save_workspace(&workspace).map_err(|error| error.to_string())?;

        Ok(json!({
            "connection": removed.sanitized(),
            "closed_tabs": closed_tabs,
            "open_tabs_preserved": !params.close_open_tabs,
        }))
    }

    fn dispatch(&self, tool: &str, params: Value) -> StdResult<Value, String> {
        match tool {
            "r_shell_ssh_connections_list" => self.list_connections(),
            "r_shell_ssh_tabs_list" => self.list_tabs(),
            "r_shell_ssh_connection_create" => self.create_connection(parse_params(params)?),
            "r_shell_ssh_connection_update" => self.update_connection(parse_params(params)?),
            "r_shell_ssh_connection_delete" => self.delete_connection(parse_params(params)?),
            other => Err(format!("Unknown MCP tool: {other}")),
        }
    }

    // -- Persistent session tools -------------------------------------------
    //
    // These keep one SSH connection alive inside `self.manager`, keyed by a
    // `session_id`, so subsequent exec/read/write calls reuse the same socket.

    /// Build an [`SshConfig`] for `ssh_session_open`, resolving either a saved
    /// connection (by id/name) or ad-hoc host params. Credential overrides in
    /// the params win over anything stored on the saved connection.
    fn resolve_open_config(
        &self,
        params: &OpenSessionParams,
    ) -> StdResult<(String, SshConfig), String> {
        if let Some(reference) = params
            .connection
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            let _guard = self.lock.lock().map_err(|_| "bridge lock poisoned")?;
            let workspace = storage::load_workspace();
            let connection = workspace
                .connections
                .iter()
                .find(|connection| connection.id == reference || connection.name == reference)
                .ok_or_else(|| format!("saved connection not found: {reference}"))?;

            let auth_method = match SavedConnection::normalize_auth_method(&connection.auth_method)
            {
                "publickey" => {
                    let key_path = params
                        .private_key_path
                        .clone()
                        .or_else(|| connection.private_key_path.clone())
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| {
                            format!("private key path required for {}", connection.name)
                        })?;
                    AuthMethod::PublicKey {
                        key_path,
                        passphrase: params
                            .passphrase
                            .clone()
                            .or_else(|| connection.passphrase.clone()),
                    }
                }
                _ => {
                    let password = params
                        .password
                        .clone()
                        .or_else(|| connection.password.clone())
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| {
                            format!(
                                "no password available for {}; pass `password` to ssh_session_open \
                                 or store one on the connection",
                                connection.name
                            )
                        })?;
                    AuthMethod::Password { password }
                }
            };

            let session_id = connection.id.clone();
            return Ok((
                session_id,
                SshConfig {
                    host: connection.host.clone(),
                    port: connection.port,
                    username: connection.username.clone(),
                    auth_method,
                    insecure: params.insecure,
                },
            ));
        }

        // Ad-hoc target.
        let host = params
            .host
            .clone()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "provide either `connection` or `host`".to_string())?;
        let username = params
            .username
            .clone()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "`username` is required when using `host`".to_string())?;
        let port = params.port.unwrap_or(22);
        if port == 0 {
            return Err("port must be greater than 0".to_string());
        }

        let auth_method = if let Some(key_path) = params
            .private_key_path
            .clone()
            .filter(|value| !value.trim().is_empty())
        {
            AuthMethod::PublicKey {
                key_path,
                passphrase: params.passphrase.clone(),
            }
        } else {
            let password = params
                .password
                .clone()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    "ad-hoc host requires `password` or `private_key_path`".to_string()
                })?;
            AuthMethod::Password { password }
        };

        let session_id = params
            .session_id
            .clone()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("{username}@{host}:{port}"));

        Ok((
            session_id,
            SshConfig {
                host,
                port,
                username,
                auth_method,
                insecure: params.insecure,
            },
        ))
    }

    async fn open_session(&self, params: OpenSessionParams) -> StdResult<Value, String> {
        let (session_id, config) = self.resolve_open_config(&params)?;
        let host = config.host.clone();
        let port = config.port;
        let username = config.username.clone();

        let reused = self.manager.has_connection(&session_id).await && !params.reconnect;
        if !reused {
            self.manager
                .create_connection(session_id.clone(), config)
                .await
                .map_err(|error| format!("failed to open SSH session: {error:#}"))?;
        }

        Ok(json!({
            "session_id": session_id,
            "host": host,
            "port": port,
            "username": username,
            "reused": reused,
            "open_sessions": self.manager.list_connection_ids().await,
        }))
    }

    async fn session_exec(&self, params: SessionExecParams) -> StdResult<Value, String> {
        let command = params.command.trim();
        if command.is_empty() {
            return Err("`command` must not be empty".to_string());
        }
        self.ensure_session(&params.session_id).await?;
        let output = self
            .manager
            .execute_command(&params.session_id, command)
            .await
            .map_err(|error| format!("command failed: {error:#}"))?;
        Ok(json!({
            "session_id": params.session_id,
            "command": command,
            "output": output,
        }))
    }

    async fn session_read_file(&self, params: SessionReadFileParams) -> StdResult<Value, String> {
        self.ensure_session(&params.session_id).await?;
        let bytes = self
            .manager
            .read_file_to_memory(&params.session_id, &params.path)
            .await
            .map_err(|error| format!("read failed: {error:#}"))?;

        match String::from_utf8(bytes.clone()) {
            Ok(content) => Ok(json!({
                "session_id": params.session_id,
                "path": params.path,
                "encoding": "utf-8",
                "size": content.len(),
                "content": content,
            })),
            Err(_) => Ok(json!({
                "session_id": params.session_id,
                "path": params.path,
                "encoding": "base64",
                "size": bytes.len(),
                "content_base64": base64_encode(&bytes),
            })),
        }
    }

    async fn session_write_file(&self, params: SessionWriteFileParams) -> StdResult<Value, String> {
        self.ensure_session(&params.session_id).await?;
        let data: Vec<u8> = match (&params.content, &params.content_base64) {
            (Some(_), Some(_)) => {
                return Err("provide only one of `content` or `content_base64`".to_string());
            }
            (Some(text), None) => text.clone().into_bytes(),
            (None, Some(encoded)) => base64_decode(encoded)?,
            (None, None) => Vec::new(),
        };

        let written = self
            .manager
            .write_file_from_bytes(&params.session_id, &params.path, &data)
            .await
            .map_err(|error| format!("write failed: {error:#}"))?;
        Ok(json!({
            "session_id": params.session_id,
            "path": params.path,
            "bytes_written": written,
        }))
    }

    async fn session_list_dir(&self, params: SessionListDirParams) -> StdResult<Value, String> {
        self.ensure_session(&params.session_id).await?;
        let path = if params.path.trim().is_empty() {
            "."
        } else {
            params.path.trim()
        };
        let entries = self
            .manager
            .list_directory(&params.session_id, path)
            .await
            .map_err(|error| format!("list failed: {error:#}"))?;
        let entries: Vec<Value> = entries
            .iter()
            .map(|entry| {
                json!({
                    "name": entry.name,
                    "kind": entry.kind.label(),
                    "permissions": entry.permissions,
                    "size": entry.size,
                    "modified": entry.modified,
                })
            })
            .collect();
        Ok(json!({
            "session_id": params.session_id,
            "path": path,
            "entries": entries,
        }))
    }

    async fn list_sessions(&self) -> StdResult<Value, String> {
        Ok(json!({ "open_sessions": self.manager.list_connection_ids().await }))
    }

    async fn close_session(&self, params: CloseSessionParams) -> StdResult<Value, String> {
        let existed = self.manager.has_connection(&params.session_id).await;
        if existed {
            self.manager
                .close_connection(&params.session_id)
                .await
                .map_err(|error| format!("failed to close session: {error:#}"))?;
        }
        Ok(json!({
            "session_id": params.session_id,
            "closed": existed,
            "open_sessions": self.manager.list_connection_ids().await,
        }))
    }

    async fn ensure_session(&self, session_id: &str) -> StdResult<(), String> {
        if self.manager.has_connection(session_id).await {
            Ok(())
        } else {
            Err(format!(
                "no open session '{session_id}'. Open one first with ssh_session_open."
            ))
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SshAuthMethod {
    Password,
    #[serde(rename = "publickey")]
    PublicKey,
}

fn default_ssh_port() -> u16 {
    22
}

fn default_folder() -> String {
    "All Connections".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CreateSshConnectionParams {
    pub name: String,
    pub host: String,
    pub username: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub auth_method: SshAuthMethod,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub private_key_path: Option<String>,
    #[serde(default)]
    pub passphrase: Option<String>,
    #[serde(default = "default_folder")]
    pub folder: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct UpdateSshConnectionParams {
    pub connection_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub auth_method: Option<SshAuthMethod>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub private_key_path: Option<String>,
    #[serde(default)]
    pub passphrase: Option<String>,
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DeleteSshConnectionParams {
    pub connection_id: String,
    #[serde(default)]
    pub close_open_tabs: bool,
}

/// Open (or reuse) a persistent SSH session. Use a saved `connection` (by id or
/// name) or ad-hoc `host`/`username`. Returns a `session_id` for the other
/// `ssh_session_*` tools.
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct OpenSessionParams {
    /// Saved connection id or name to open. Mutually exclusive with `host`.
    #[serde(default)]
    pub connection: Option<String>,
    /// Ad-hoc host (used instead of `connection`).
    #[serde(default)]
    pub host: Option<String>,
    /// Ad-hoc username (required with `host`).
    #[serde(default)]
    pub username: Option<String>,
    /// Ad-hoc port (defaults to 22).
    #[serde(default)]
    pub port: Option<u16>,
    /// Password override (or the password for an ad-hoc password-auth host).
    #[serde(default)]
    pub password: Option<String>,
    /// Private key path override (or the key for an ad-hoc publickey host).
    #[serde(default)]
    pub private_key_path: Option<String>,
    /// Passphrase for an encrypted private key.
    #[serde(default)]
    pub passphrase: Option<String>,
    /// Explicit session id for an ad-hoc target (defaults to user@host:port).
    #[serde(default)]
    pub session_id: Option<String>,
    /// Skip host-key verification (dangerous).
    #[serde(default)]
    pub insecure: bool,
    /// Force a fresh connection even if a session with this id is already open.
    #[serde(default)]
    pub reconnect: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SessionExecParams {
    /// Session id returned by ssh_session_open.
    pub session_id: String,
    /// Command to run on the already-open session.
    pub command: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SessionReadFileParams {
    pub session_id: String,
    /// Absolute (or remote-relative) path to read.
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SessionWriteFileParams {
    pub session_id: String,
    /// Remote path to create/overwrite.
    pub path: String,
    /// UTF-8 text content. Use this for text files. Mutually exclusive with
    /// `content_base64`.
    #[serde(default)]
    pub content: Option<String>,
    /// Base64-encoded bytes for binary content.
    #[serde(default)]
    pub content_base64: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SessionListDirParams {
    pub session_id: String,
    /// Remote directory to list (defaults to ".").
    #[serde(default = "default_list_dir")]
    pub path: String,
}

fn default_list_dir() -> String {
    ".".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CloseSessionParams {
    pub session_id: String,
}

#[derive(Clone)]
pub struct RShellMcpServer {
    bridge: Arc<McpBridge>,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl RShellMcpServer {
    pub fn new(bridge: Arc<McpBridge>) -> Self {
        Self {
            bridge,
            tool_router: Self::tool_router(),
        }
    }

    async fn forward(&self, tool: &str, params: Value) -> CallToolResult {
        Self::into_result(self.bridge.dispatch(tool, params))
    }

    /// Convert a bridge result into an MCP `CallToolResult`.
    fn into_result(result: StdResult<Value, String>) -> CallToolResult {
        match result {
            Ok(value) => CallToolResult::structured(value),
            Err(error) => {
                CallToolResult::structured_error(json!({ "success": false, "error": error }))
            }
        }
    }
}

#[tool_router]
impl RShellMcpServer {
    #[tool(description = "List saved R-Shell SSH connections without returning credentials")]
    async fn r_shell_ssh_connections_list(&self) -> CallToolResult {
        self.forward("r_shell_ssh_connections_list", json!({}))
            .await
    }

    #[tool(description = "Create a persistent SSH connection, connect it, and open a terminal tab")]
    async fn r_shell_ssh_connection_create(
        &self,
        Parameters(params): Parameters<CreateSshConnectionParams>,
    ) -> CallToolResult {
        self.forward(
            "r_shell_ssh_connection_create",
            serde_json::to_value(params)
                .unwrap_or_else(|error| json!({ "serialization_error": error.to_string() })),
        )
        .await
    }

    #[tool(
        description = "Update a saved SSH connection and sync metadata to any open terminal tabs"
    )]
    async fn r_shell_ssh_connection_update(
        &self,
        Parameters(params): Parameters<UpdateSshConnectionParams>,
    ) -> CallToolResult {
        self.forward(
            "r_shell_ssh_connection_update",
            serde_json::to_value(params)
                .unwrap_or_else(|error| json!({ "serialization_error": error.to_string() })),
        )
        .await
    }

    #[tool(description = "Delete a saved SSH connection, optionally closing open terminal tabs")]
    async fn r_shell_ssh_connection_delete(
        &self,
        Parameters(params): Parameters<DeleteSshConnectionParams>,
    ) -> CallToolResult {
        self.forward(
            "r_shell_ssh_connection_delete",
            serde_json::to_value(params)
                .unwrap_or_else(|error| json!({ "serialization_error": error.to_string() })),
        )
        .await
    }

    #[tool(description = "List currently open R-Shell SSH terminal tabs")]
    async fn r_shell_ssh_tabs_list(&self) -> CallToolResult {
        self.forward("r_shell_ssh_tabs_list", json!({})).await
    }

    #[tool(
        description = "Open (or reuse) a persistent SSH session that stays connected across calls. \
                       Returns a session_id to use with ssh_exec / ssh_read_file / ssh_write_file / \
                       ssh_list_dir. Use a saved `connection` (id|name) or ad-hoc `host`/`username`."
    )]
    async fn ssh_session_open(
        &self,
        Parameters(params): Parameters<OpenSessionParams>,
    ) -> CallToolResult {
        Self::into_result(self.bridge.open_session(params).await)
    }

    #[tool(
        description = "Run a command on an already-open SSH session (reuses the connection, no \
                       reconnect). Requires a session_id from ssh_session_open."
    )]
    async fn ssh_exec(&self, Parameters(params): Parameters<SessionExecParams>) -> CallToolResult {
        Self::into_result(self.bridge.session_exec(params).await)
    }

    #[tool(
        description = "Read a remote file over an open SSH session. Returns UTF-8 text when \
                       possible, otherwise base64. Ideal for editing a file without reconnecting."
    )]
    async fn ssh_read_file(
        &self,
        Parameters(params): Parameters<SessionReadFileParams>,
    ) -> CallToolResult {
        Self::into_result(self.bridge.session_read_file(params).await)
    }

    #[tool(
        description = "Write/overwrite a remote file over an open SSH session. Provide `content` \
                       for text or `content_base64` for binary. No reconnect needed."
    )]
    async fn ssh_write_file(
        &self,
        Parameters(params): Parameters<SessionWriteFileParams>,
    ) -> CallToolResult {
        Self::into_result(self.bridge.session_write_file(params).await)
    }

    #[tool(description = "List a remote directory over an open SSH session (Linux hosts).")]
    async fn ssh_list_dir(
        &self,
        Parameters(params): Parameters<SessionListDirParams>,
    ) -> CallToolResult {
        Self::into_result(self.bridge.session_list_dir(params).await)
    }

    #[tool(description = "List the session ids that currently have a live SSH connection open.")]
    async fn ssh_sessions_list(&self) -> CallToolResult {
        Self::into_result(self.bridge.list_sessions().await)
    }

    #[tool(description = "Close a persistent SSH session opened with ssh_session_open.")]
    async fn ssh_session_close(
        &self,
        Parameters(params): Parameters<CloseSessionParams>,
    ) -> CallToolResult {
        Self::into_result(self.bridge.close_session(params).await)
    }
}

#[tool_handler]
impl ServerHandler for RShellMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions(
                "R-Shell MCP server. Manage saved SSH connections, and open persistent SSH \
                 sessions (ssh_session_open) that stay connected so you can run commands and \
                 read/write remote files repeatedly without reconnecting."
                    .to_string(),
            )
    }
}

pub async fn start_mcp_server(bridge: Arc<McpBridge>) -> anyhow::Result<()> {
    let bind_address = format!("127.0.0.1:{MCP_PORT}");
    let cancellation_token = CancellationToken::new();
    let service_bridge = bridge.clone();

    let service = StreamableHttpService::new(
        move || Ok(RShellMcpServer::new(service_bridge.clone())),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default()
            .with_cancellation_token(cancellation_token.child_token()),
    );

    let router = Router::new()
        .nest_service(MCP_ENDPOINT_PATH, service)
        .layer(middleware::from_fn(validate_local_origin));

    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    MCP_SERVER_RUNNING.store(true, Ordering::SeqCst);
    eprintln!("R-Shell MCP server listening on {MCP_ENDPOINT}");

    if let Err(error) = axum::serve(listener, router).await {
        MCP_SERVER_RUNNING.store(false, Ordering::SeqCst);
        return Err(error.into());
    }

    MCP_SERVER_RUNNING.store(false, Ordering::SeqCst);
    Ok(())
}

#[allow(dead_code)]
pub fn server_running() -> bool {
    MCP_SERVER_RUNNING.load(Ordering::SeqCst)
}

/// Guard the MCP endpoint against cross-origin and DNS-rebinding access.
///
/// The server binds to loopback, but a malicious web page (or a hostname that
/// resolves to 127.0.0.1) could still try to reach it from the user's browser.
/// We therefore require BOTH:
///
/// 1. The `Host` header to be a loopback host (`127.0.0.1` / `localhost` /
///    `[::1]`). This blocks DNS-rebinding, where an attacker domain resolves to
///    127.0.0.1 — the browser sends the attacker's hostname in `Host`.
/// 2. If an `Origin` header is present, it must be a loopback origin. A missing
///    `Origin` is allowed only because non-browser MCP clients omit it; browser
///    `fetch`/XHR always attach one (including the literal `null`, which is
///    rejected).
async fn validate_local_origin(request: Request<Body>, next: Next) -> Response {
    // 1. Host header must be loopback (defeats DNS rebinding).
    let host_ok = request
        .headers()
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .map(is_loopback_host_header)
        .unwrap_or(false);
    if !host_ok {
        return StatusCode::FORBIDDEN.into_response();
    }

    // 2. If Origin is present it must be loopback. `null` and cross-site are out.
    if let Some(origin) = request.headers().get(ORIGIN).and_then(|v| v.to_str().ok()) {
        if !is_allowed_local_origin(origin) {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    next.run(request).await
}

/// True if `value` is the host portion of a loopback address. Accepts an
/// optional port (`127.0.0.1:9123`) and bracketed IPv6 (`[::1]:9123`).
fn is_loopback_host(value: &str) -> bool {
    let host = if let Some(rest) = value.strip_prefix('[') {
        // [::1] or [::1]:port
        match rest.split_once(']') {
            Some((inner, _)) => inner,
            None => return false,
        }
    } else if let Some((h, _port)) = value.rsplit_once(':') {
        // host:port — but only strip when the remainder isn't itself IPv6.
        if h.contains(':') { value } else { h }
    } else {
        value
    };

    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// Validate the `Host` request header.
fn is_loopback_host_header(host: &str) -> bool {
    is_loopback_host(host)
}

/// Validate an `Origin` header value: must be an http(s) loopback origin.
/// The literal `null` and any non-loopback host are rejected.
fn is_allowed_local_origin(origin: &str) -> bool {
    let Ok(uri) = origin.parse::<axum::http::Uri>() else {
        return false;
    };

    let scheme_allowed = matches!(uri.scheme_str(), Some("http" | "https"));
    // `Uri::host()` returns the IPv6 literal with brackets, e.g. `[::1]`.
    let host_allowed = matches!(
        uri.host(),
        Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
    );

    scheme_allowed && host_allowed
}

fn parse_params<T: for<'de> Deserialize<'de>>(value: Value) -> StdResult<T, String> {
    serde_json::from_value(value).map_err(|error| error.to_string())
}

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Minimal standard base64 encoder (with `=` padding). Kept dependency-free so
/// binary file contents can round-trip through MCP JSON responses.
fn base64_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(BASE64_ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        out.push(BASE64_ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 {
            BASE64_ALPHABET[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            BASE64_ALPHABET[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Minimal standard base64 decoder. Ignores ASCII whitespace; rejects other
/// invalid characters.
fn base64_decode(input: &str) -> StdResult<Vec<u8>, String> {
    fn value_of(byte: u8) -> StdResult<u32, String> {
        match byte {
            b'A'..=b'Z' => Ok((byte - b'A') as u32),
            b'a'..=b'z' => Ok((byte - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((byte - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(format!("invalid base64 character: {byte:#x}")),
        }
    }

    let filtered: Vec<u8> = input
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace() && *byte != b'=')
        .collect();

    let mut out = Vec::with_capacity(filtered.len() / 4 * 3);
    for chunk in filtered.chunks(4) {
        if chunk.len() == 1 {
            return Err("invalid base64 length".to_string());
        }
        let mut buffer = 0u32;
        for &byte in chunk {
            buffer = (buffer << 6) | value_of(byte)?;
        }
        // Left-align the bits we actually have.
        buffer <<= 6 * (4 - chunk.len());

        out.push(((buffer >> 16) & 0xff) as u8);
        if chunk.len() >= 3 {
            out.push(((buffer >> 8) & 0xff) as u8);
        }
        if chunk.len() >= 4 {
            out.push((buffer & 0xff) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_local_origins() {
        assert!(is_allowed_local_origin("http://127.0.0.1:1420"));
        assert!(is_allowed_local_origin("http://localhost:1420"));
        assert!(is_allowed_local_origin("http://[::1]:1420"));
    }

    #[test]
    fn rejects_non_local_origins() {
        assert!(!is_allowed_local_origin("https://example.com"));
        assert!(!is_allowed_local_origin("http://192.168.1.10:1420"));
        assert!(!is_allowed_local_origin("http://127.0.0.1.evil.test"));
        assert!(!is_allowed_local_origin("http://localhost.evil.test"));
    }

    #[test]
    fn rejects_null_origin() {
        // `null` is sent by sandboxed iframes / file:// pages and must be denied.
        assert!(!is_allowed_local_origin("null"));
    }

    #[test]
    fn base64_round_trips() {
        for sample in [
            &b""[..],
            b"f",
            b"fo",
            b"foo",
            b"foob",
            b"fooba",
            b"foobar",
            b"\x00\xff\x10\x80binary\xfe",
        ] {
            let encoded = base64_encode(sample);
            let decoded = base64_decode(&encoded).expect("decode");
            assert_eq!(decoded, sample, "round-trip failed for {sample:?}");
        }
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_decode("Zm9vYmFy").unwrap(), b"foobar");
        // Whitespace (incl. newlines) is ignored.
        assert_eq!(base64_decode("Zm9v\n").unwrap(), b"foo");
        assert_eq!(base64_decode("Zm 9v").unwrap(), b"foo");
        // A character outside the alphabet is rejected.
        assert!(base64_decode("Zm9v!").is_err());
        // A single trailing char cannot form a byte.
        assert!(base64_decode("Zm9vY").is_err());
    }

    #[test]
    fn accepts_loopback_host_headers() {
        assert!(is_loopback_host_header("127.0.0.1:9123"));
        assert!(is_loopback_host_header("127.0.0.1"));
        assert!(is_loopback_host_header("localhost:9123"));
        assert!(is_loopback_host_header("localhost"));
        assert!(is_loopback_host_header("[::1]:9123"));
        assert!(is_loopback_host_header("[::1]"));
        assert!(is_loopback_host_header("::1"));
    }

    #[test]
    fn rejects_non_loopback_host_headers() {
        // DNS-rebinding: attacker domain resolving to 127.0.0.1 still sends its
        // own hostname in the Host header.
        assert!(!is_loopback_host_header("attacker.com"));
        assert!(!is_loopback_host_header("attacker.com:9123"));
        assert!(!is_loopback_host_header("192.168.1.10:9123"));
        assert!(!is_loopback_host_header("127.0.0.1.evil.test"));
        assert!(!is_loopback_host_header("localhost.evil.test"));
        assert!(!is_loopback_host_header(""));
    }
}
