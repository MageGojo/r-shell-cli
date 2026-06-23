//! Connection CRUD service — the single source of truth shared by the CLI and
//! the desktop GUI for creating, editing, and deleting saved SSH connections.
//!
//! The pure helpers ([`build_connection`], [`apply_patch`], [`remove_from`]) hold
//! the validation and mutation rules and are unit-tested without touching disk.
//! The thin [`add`] / [`update`] / [`remove`] wrappers load `workspace.json`,
//! apply a pure helper, and persist the result.

use anyhow::{Result, anyhow, bail};

use crate::model::{ConnectionStatus, PersistedWorkspace, SavedConnection};
use crate::native_backend::{AuthMethod, SshConfig};
use crate::storage;

/// Folder a connection lands in when none is provided.
pub const DEFAULT_FOLDER: &str = "All Connections";

/// Editable fields for a brand-new connection.
///
/// Secret fields are plaintext and are only ever supplied by a trusted local
/// caller (CLI flags or the local GUI form). An empty secret is treated as
/// "not set".
#[derive(Clone, Debug, Default)]
pub struct NewConnection {
    /// "SSH" (default) or "ADB". Empty / unknown values are treated as "SSH".
    pub protocol: String,
    pub name: String,
    pub host: String,
    pub username: String,
    pub port: u16,
    pub auth_method: String,
    pub folder: String,
    pub description: String,
    pub tags: Vec<String>,
    pub password: Option<String>,
    pub private_key_path: Option<String>,
    pub passphrase: Option<String>,
}

/// A partial edit of an existing connection.
///
/// Every field is optional; `None` leaves the stored value untouched. For the
/// three secret fields, `Some(value)` sets a non-empty value and `Some("")`
/// clears it. For the plain text fields, a blank (after trimming) value is
/// ignored rather than wiping a required field — except `description` and
/// `tags`, which can be set to empty intentionally.
#[derive(Clone, Debug, Default)]
pub struct ConnectionPatch {
    /// `Some("ADB"|"SSH")` switches the protocol; `None` leaves it unchanged.
    pub protocol: Option<String>,
    pub name: Option<String>,
    pub host: Option<String>,
    pub username: Option<String>,
    pub port: Option<u16>,
    pub auth_method: Option<String>,
    pub folder: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub password: Option<String>,
    pub private_key_path: Option<String>,
    pub passphrase: Option<String>,
}

/// Build a validated [`SavedConnection`] from a [`NewConnection`] (no I/O).
pub fn build_connection(input: NewConnection) -> Result<SavedConnection> {
    let protocol = if input.protocol.eq_ignore_ascii_case("ADB") {
        "ADB"
    } else {
        "SSH"
    };

    let name = input.name.trim().to_string();
    let host = input.host.trim().to_string();
    let username = input.username.trim().to_string();
    if protocol == "ADB" {
        // ADB targets a device by host:port; it has no username / auth concept.
        if name.is_empty() || host.is_empty() || input.port == 0 {
            bail!("name, host, and a valid port are required");
        }
    } else if name.is_empty() || host.is_empty() || username.is_empty() || input.port == 0 {
        bail!("name, host, username, and a valid port are required");
    }

    let folder = input.folder.trim();
    Ok(SavedConnection {
        id: SavedConnection::new_id(),
        name,
        host,
        port: input.port,
        username,
        protocol: protocol.to_string(),
        folder: if folder.is_empty() {
            DEFAULT_FOLDER.to_string()
        } else {
            folder.to_string()
        },
        tags: input.tags,
        description: input.description,
        auth_method: SavedConnection::normalize_auth_method(&input.auth_method).to_string(),
        password: input.password.filter(|value| !value.is_empty()),
        private_key_path: input.private_key_path.filter(|value| !value.is_empty()),
        passphrase: input.passphrase.filter(|value| !value.is_empty()),
        status: ConnectionStatus::Disconnected,
    })
}

/// Apply a [`ConnectionPatch`] to a connection in place.
pub fn apply_patch(connection: &mut SavedConnection, patch: ConnectionPatch) -> Result<()> {
    if let Some(protocol) = patch.protocol {
        connection.protocol = if protocol.eq_ignore_ascii_case("ADB") {
            "ADB".to_string()
        } else {
            "SSH".to_string()
        };
    }
    if let Some(name) = patch.name {
        let name = name.trim();
        if !name.is_empty() {
            connection.name = name.to_string();
        }
    }
    if let Some(host) = patch.host {
        let host = host.trim();
        if !host.is_empty() {
            connection.host = host.to_string();
        }
    }
    if let Some(username) = patch.username {
        let username = username.trim();
        if !username.is_empty() {
            connection.username = username.to_string();
        }
    }
    if let Some(port) = patch.port {
        if port == 0 {
            bail!("port must be greater than 0");
        }
        connection.port = port;
    }
    if let Some(auth_method) = patch.auth_method {
        connection.auth_method = SavedConnection::normalize_auth_method(&auth_method).to_string();
    }
    if let Some(password) = patch.password {
        connection.password = (!password.is_empty()).then_some(password);
    }
    if let Some(private_key_path) = patch.private_key_path {
        connection.private_key_path = (!private_key_path.is_empty()).then_some(private_key_path);
    }
    if let Some(passphrase) = patch.passphrase {
        connection.passphrase = (!passphrase.is_empty()).then_some(passphrase);
    }
    if let Some(folder) = patch.folder {
        let folder = folder.trim();
        if !folder.is_empty() {
            connection.folder = folder.to_string();
        }
    }
    if let Some(description) = patch.description {
        connection.description = description;
    }
    if let Some(tags) = patch.tags {
        connection.tags = tags;
    }
    Ok(())
}

/// Remove a connection from a workspace, cleaning up dependent terminal tabs and
/// the active-connection pointer.
pub fn remove_from(workspace: &mut PersistedWorkspace, id: &str) -> Result<SavedConnection> {
    let index = workspace
        .connections
        .iter()
        .position(|connection| connection.id == id)
        .ok_or_else(|| anyhow!("SSH connection not found: {id}"))?;

    let removed = workspace.connections.remove(index);
    workspace.tabs.retain(|tab| tab.connection_id != removed.id);
    if workspace.active_connection_id.as_deref() == Some(removed.id.as_str()) {
        workspace.active_connection_id = workspace
            .connections
            .first()
            .map(|connection| connection.id.clone());
    }
    Ok(removed)
}

/// Create a connection and persist it to `workspace.json`. The new connection is
/// also marked active. Returns the stored connection (with its generated id).
pub fn add(input: NewConnection) -> Result<SavedConnection> {
    let connection = build_connection(input)?;
    let mut workspace = storage::load_workspace();
    workspace.connections.push(connection.clone());
    workspace.active_connection_id = Some(connection.id.clone());
    storage::save_workspace(&workspace)?;
    Ok(connection)
}

/// Apply a patch to an existing connection and persist. Returns the updated copy.
pub fn update(id: &str, patch: ConnectionPatch) -> Result<SavedConnection> {
    let mut workspace = storage::load_workspace();
    let connection = workspace
        .connections
        .iter_mut()
        .find(|connection| connection.id == id)
        .ok_or_else(|| anyhow!("SSH connection not found: {id}"))?;
    apply_patch(connection, patch)?;
    let updated = connection.clone();
    storage::save_workspace(&workspace)?;
    Ok(updated)
}

/// Remove a connection by id and persist. Returns the removed connection.
pub fn remove(id: &str) -> Result<SavedConnection> {
    let mut workspace = storage::load_workspace();
    let removed = remove_from(&mut workspace, id)?;
    storage::save_workspace(&workspace)?;
    Ok(removed)
}

/// Look up a saved connection by id from the local `workspace.json`.
pub fn find(id: &str) -> Result<SavedConnection> {
    storage::load_workspace()
        .connections
        .into_iter()
        .find(|connection| connection.id == id)
        .ok_or_else(|| anyhow!("SSH connection not found: {id}"))
}

/// Build a connectable [`SshConfig`] from a saved connection, using only stored
/// credentials (no interactive prompting — for GUI / headless callers).
///
/// Password auth with no stored password is an error: the caller should ask the
/// user to set one in the connection settings.
pub fn build_ssh_config(connection: &SavedConnection, insecure: bool) -> Result<SshConfig> {
    let auth_method = match SavedConnection::normalize_auth_method(&connection.auth_method) {
        "publickey" => {
            let key_path = connection
                .private_key_path
                .clone()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| anyhow!("private key path required for {}", connection.name))?;
            AuthMethod::PublicKey {
                key_path,
                passphrase: connection.passphrase.clone(),
            }
        }
        _ => {
            let password = connection
                .password
                .clone()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    anyhow!(
                        "no password stored for {}; set one in the connection settings",
                        connection.name
                    )
                })?;
            AuthMethod::Password { password }
        }
    };

    Ok(SshConfig {
        host: connection.host.clone(),
        port: connection.port,
        username: connection.username.clone(),
        auth_method,
        insecure,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TerminalTab;

    fn sample() -> NewConnection {
        NewConnection {
            protocol: "SSH".to_string(),
            name: "  Prod  ".to_string(),
            host: " 10.0.0.1 ".to_string(),
            username: " deploy ".to_string(),
            port: 22,
            auth_method: "publickey".to_string(),
            folder: "   ".to_string(),
            description: "main box".to_string(),
            tags: vec!["prod".to_string()],
            password: Some(String::new()),
            private_key_path: Some("~/.ssh/id".to_string()),
            passphrase: None,
        }
    }

    #[test]
    fn build_trims_and_defaults_folder() {
        let connection = build_connection(sample()).unwrap();
        assert_eq!(connection.name, "Prod");
        assert_eq!(connection.host, "10.0.0.1");
        assert_eq!(connection.username, "deploy");
        assert_eq!(connection.folder, DEFAULT_FOLDER);
        assert_eq!(connection.auth_method, "publickey");
        assert_eq!(connection.protocol, "SSH");
        // An empty password is filtered out; a real key path is kept.
        assert!(connection.password.is_none());
        assert_eq!(connection.private_key_path.as_deref(), Some("~/.ssh/id"));
        assert_eq!(connection.status, ConnectionStatus::Disconnected);
    }

    #[test]
    fn build_rejects_missing_required_fields() {
        let mut blank_host = sample();
        blank_host.host = "   ".to_string();
        assert!(build_connection(blank_host).is_err());

        let mut zero_port = sample();
        zero_port.port = 0;
        assert!(build_connection(zero_port).is_err());
    }

    #[test]
    fn build_normalizes_unknown_auth_to_password() {
        let mut input = sample();
        input.auth_method = "totally-unknown".to_string();
        assert_eq!(build_connection(input).unwrap().auth_method, "password");
    }

    #[test]
    fn patch_updates_only_provided_fields() {
        let mut connection = build_connection(sample()).unwrap();
        let original_host = connection.host.clone();
        apply_patch(
            &mut connection,
            ConnectionPatch {
                name: Some(" Renamed ".to_string()),
                port: Some(2222),
                password: Some("s3cret".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(connection.name, "Renamed");
        assert_eq!(connection.port, 2222);
        assert_eq!(connection.host, original_host);
        assert_eq!(connection.password.as_deref(), Some("s3cret"));
    }

    #[test]
    fn patch_blank_secret_clears_it_but_blank_name_is_ignored() {
        let mut connection = build_connection(sample()).unwrap();
        connection.password = Some("old".to_string());
        let original_name = connection.name.clone();
        apply_patch(
            &mut connection,
            ConnectionPatch {
                name: Some("   ".to_string()),
                password: Some(String::new()),
                description: Some(String::new()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(connection.name, original_name);
        assert!(connection.password.is_none());
        assert_eq!(connection.description, "");
    }

    #[test]
    fn patch_rejects_zero_port() {
        let mut connection = build_connection(sample()).unwrap();
        assert!(
            apply_patch(
                &mut connection,
                ConnectionPatch {
                    port: Some(0),
                    ..Default::default()
                },
            )
            .is_err()
        );
    }

    #[test]
    fn remove_from_cleans_tabs_and_active_pointer() {
        let mut workspace = PersistedWorkspace::default();
        let connection = build_connection(sample()).unwrap();
        let id = connection.id.clone();
        workspace.connections.push(connection);
        workspace.active_connection_id = Some(id.clone());
        workspace.tabs.push(TerminalTab {
            id: "tab-1".to_string(),
            connection_id: id.clone(),
            title: "session".to_string(),
            status: ConnectionStatus::Disconnected,
        });

        let removed = remove_from(&mut workspace, &id).unwrap();
        assert_eq!(removed.id, id);
        assert!(workspace.connections.is_empty());
        assert!(workspace.tabs.is_empty());
        assert_eq!(workspace.active_connection_id, None);
    }

    #[test]
    fn remove_from_unknown_id_errors() {
        let mut workspace = PersistedWorkspace::default();
        assert!(remove_from(&mut workspace, "does-not-exist").is_err());
    }
}
