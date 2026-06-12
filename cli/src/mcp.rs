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
use crate::storage;

pub const MCP_PORT: u16 = 9123;
pub const MCP_ENDPOINT_PATH: &str = "/mcp";
pub const MCP_ENDPOINT: &str = "http://127.0.0.1:9123/mcp";

static MCP_SERVER_RUNNING: AtomicBool = AtomicBool::new(false);

/// Bridge between the MCP HTTP handlers and the locally persisted workspace.
///
/// Tool calls operate directly on the persisted `workspace.json`, guarded by a
/// mutex so concurrent MCP requests serialize their read-modify-write cycles.
#[derive(Debug, Default)]
pub struct McpBridge {
    lock: Mutex<()>,
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
        match self.bridge.dispatch(tool, params) {
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
}

#[tool_handler]
impl ServerHandler for RShellMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions("R-Shell MCP server for managing saved SSH connections.".to_string())
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
