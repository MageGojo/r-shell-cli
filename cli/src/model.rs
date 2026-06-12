use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        Self::Disconnected
    }
}

impl ConnectionStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting",
            Self::Connected => "Connected",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SavedConnection {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    #[serde(default = "default_folder")]
    pub folder: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_auth_method", alias = "authMethod")]
    pub auth_method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(
        default,
        alias = "privateKeyPath",
        skip_serializing_if = "Option::is_none"
    )]
    pub private_key_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passphrase: Option<String>,
    #[serde(default)]
    pub status: ConnectionStatus,
}

impl SavedConnection {
    /// Generate a unique connection id based on the current timestamp.
    pub fn new_id() -> String {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        format!("ssh-{millis}")
    }

    /// Canonical auth-method label: either `publickey` or `password`.
    pub fn normalize_auth_method(auth_method: &str) -> &'static str {
        match auth_method {
            "publickey" | "public_key" => "publickey",
            _ => "password",
        }
    }

    /// A credential-free JSON view of this connection, safe for MCP responses.
    pub fn sanitized(&self) -> Value {
        json!({
            "connection_id": self.id,
            "name": self.name,
            "host": self.host,
            "username": self.username,
            "port": self.port,
            "auth_method": self.auth_method,
            "folder": self.folder,
            "tags": self.tags,
            "description": self.description,
            "status": self.status.label(),
            "has_password": self.password.as_ref().map(|v| !v.is_empty()).unwrap_or(false),
            "has_private_key_path": self
                .private_key_path
                .as_ref()
                .map(|v| !v.is_empty())
                .unwrap_or(false),
        })
    }
}

fn default_protocol() -> String {
    "SSH".to_string()
}

fn default_folder() -> String {
    "All Connections".to_string()
}

fn default_auth_method() -> String {
    "password".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TerminalTab {
    pub id: String,
    pub connection_id: String,
    pub title: String,
    #[serde(default)]
    pub status: ConnectionStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PersistedWorkspace {
    #[serde(default)]
    pub connections: Vec<SavedConnection>,
    #[serde(default)]
    pub tabs: Vec<TerminalTab>,
    #[serde(default)]
    pub active_connection_id: Option<String>,
    #[serde(default)]
    pub active_tab_id: Option<String>,
}

impl PersistedWorkspace {
    pub fn normalized(mut self) -> Self {
        self.connections
            .retain(|connection| connection.protocol.eq_ignore_ascii_case("SSH"));

        for connection in &mut self.connections {
            connection.protocol = "SSH".to_string();
            connection.auth_method = match connection.auth_method.as_str() {
                "publickey" | "public_key" => "publickey".to_string(),
                _ => "password".to_string(),
            };
            if connection.folder.trim().is_empty() {
                connection.folder = "All Connections".to_string();
            }
        }

        self.tabs.retain(|tab| {
            self.connections
                .iter()
                .any(|connection| connection.id == tab.connection_id)
        });

        if self.active_connection_id.as_ref().is_none_or(|id| {
            !self
                .connections
                .iter()
                .any(|connection| connection.id == *id)
        }) {
            self.active_connection_id = self
                .connections
                .first()
                .map(|connection| connection.id.clone());
        }

        if self
            .active_tab_id
            .as_ref()
            .is_none_or(|id| !self.tabs.iter().any(|tab| tab.id == *id))
        {
            self.active_tab_id = self.tabs.first().map(|tab| tab.id.clone());
        }

        self
    }
}

impl Default for PersistedWorkspace {
    fn default() -> Self {
        let connection = SavedConnection {
            id: "local-demo".to_string(),
            name: "root@192.168.0.103:5555".to_string(),
            host: "192.168.0.103".to_string(),
            port: 5555,
            username: "root".to_string(),
            protocol: "SSH".to_string(),
            folder: "Work".to_string(),
            tags: Vec::new(),
            description: "Imported from the current R-Shell workspace".to_string(),
            auth_method: "password".to_string(),
            password: None,
            private_key_path: None,
            passphrase: None,
            status: ConnectionStatus::Disconnected,
        };

        Self {
            active_connection_id: Some(connection.id.clone()),
            connections: vec![connection],
            tabs: Vec::new(),
            active_tab_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_legacy_connection_export() {
        let workspace = serde_json::from_str::<PersistedWorkspace>(
            r#"{
                "connections": [
                    {
                        "id": "ssh-1",
                        "name": "Production",
                        "host": "203.0.113.10",
                        "port": 22,
                        "username": "deploy",
                        "protocol": "SSH",
                        "folder": "All Connections/Work",
                        "authMethod": "publickey",
                        "privateKeyPath": "~/.ssh/prod",
                        "passphrase": "secret",
                        "tags": ["prod"]
                    }
                ],
                "folders": []
            }"#,
        )
        .unwrap()
        .normalized();

        assert_eq!(workspace.connections.len(), 1);
        let connection = &workspace.connections[0];
        assert_eq!(connection.auth_method, "publickey");
        assert_eq!(connection.private_key_path.as_deref(), Some("~/.ssh/prod"));
        assert_eq!(connection.status, ConnectionStatus::Disconnected);
        assert_eq!(workspace.active_connection_id.as_deref(), Some("ssh-1"));
    }

    #[test]
    fn filters_non_ssh_connections_after_import() {
        let workspace = serde_json::from_str::<PersistedWorkspace>(
            r#"{
                "connections": [
                    {
                        "id": "ssh-1",
                        "name": "SSH",
                        "host": "203.0.113.10",
                        "port": 22,
                        "username": "deploy",
                        "protocol": "SSH"
                    },
                    {
                        "id": "ftp-1",
                        "name": "FTP",
                        "host": "203.0.113.11",
                        "port": 21,
                        "username": "ftp",
                        "protocol": "FTP"
                    }
                ],
                "active_connection_id": "ftp-1"
            }"#,
        )
        .unwrap()
        .normalized();

        assert_eq!(workspace.connections.len(), 1);
        assert_eq!(workspace.connections[0].id, "ssh-1");
        assert_eq!(workspace.active_connection_id.as_deref(), Some("ssh-1"));
    }
}
