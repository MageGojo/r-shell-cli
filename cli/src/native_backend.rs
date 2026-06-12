use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Result;
use tokio::sync::RwLock;

pub use crate::ssh::{AuthMethod, SshConfig};

use crate::ssh::{PtySession, SshClient};

#[derive(Clone, Debug)]
pub struct RemoteFileEntry {
    pub name: String,
    pub kind: RemoteFileKind,
    pub permissions: String,
    pub size: String,
    pub modified: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteFileKind {
    Directory,
    Symlink,
    File,
    Other,
}

pub struct NativeConnectionManager {
    connections: RwLock<HashMap<String, Arc<RwLock<SshClient>>>>,
    pty_sessions: RwLock<HashMap<String, Arc<PtySession>>>,
}

impl Default for NativeConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            pty_sessions: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create_connection(&self, connection_id: String, config: SshConfig) -> Result<()> {
        let mut client = SshClient::new();
        client.connect(&config).await?;

        let mut connections = self.connections.write().await;
        if let Some(existing) = connections.remove(&connection_id) {
            let mut existing = existing.write().await;
            let _ = existing.disconnect().await;
        }
        connections.insert(connection_id, Arc::new(RwLock::new(client)));

        Ok(())
    }

    pub async fn close_connection(&self, connection_id: &str) -> Result<()> {
        self.pty_sessions.write().await.remove(connection_id);

        let mut connections = self.connections.write().await;
        if let Some(client) = connections.remove(connection_id) {
            let mut client = client.write().await;
            client.disconnect().await?;
        }
        Ok(())
    }

    pub async fn execute_command(&self, connection_id: &str, command: &str) -> Result<String> {
        let connections = self.connections.read().await;
        let client = connections
            .get(connection_id)
            .ok_or_else(|| anyhow::anyhow!("Connection not found"))?;

        let client = client.read().await;
        client.execute_command(command).await
    }

    /// Snapshot remote resource usage (CPU, memory, disk, network) in one call.
    /// Returns the raw combined output for [`crate::monitor::parse_snapshot`].
    pub async fn fetch_system_stats(&self, connection_id: &str) -> Result<String> {
        self.execute_command(connection_id, &crate::monitor::stats_command())
            .await
    }

    pub async fn list_directory(
        &self,
        connection_id: &str,
        path: &str,
    ) -> Result<Vec<RemoteFileEntry>> {
        let command = format!(
            "LC_ALL=C ls -la --time-style=long-iso {}",
            shell_quote(path)
        );
        let output = self.execute_command(connection_id, &command).await?;
        parse_ls_output(&output)
    }

    pub async fn download_file(
        &self,
        connection_id: &str,
        remote_path: &str,
        local_path: &str,
    ) -> Result<u64> {
        let connections = self.connections.read().await;
        let client = connections
            .get(connection_id)
            .ok_or_else(|| anyhow::anyhow!("Connection not found"))?;

        let client = client.read().await;
        client.download_file(remote_path, local_path).await
    }

    pub async fn upload_file(
        &self,
        connection_id: &str,
        local_path: &str,
        remote_path: &str,
    ) -> Result<u64> {
        let connections = self.connections.read().await;
        let client = connections
            .get(connection_id)
            .ok_or_else(|| anyhow::anyhow!("Connection not found"))?;

        let client = client.read().await;
        client.upload_file(local_path, remote_path).await
    }

    /// Read a remote file's full contents into memory over the live session's
    /// SFTP subsystem. Used by the MCP `ssh_read_file` tool so a file can be
    /// edited without reconnecting.
    pub async fn read_file_to_memory(
        &self,
        connection_id: &str,
        remote_path: &str,
    ) -> Result<Vec<u8>> {
        let connections = self.connections.read().await;
        let client = connections
            .get(connection_id)
            .ok_or_else(|| anyhow::anyhow!("Connection not found"))?;

        let client = client.read().await;
        client.download_file_to_memory(remote_path).await
    }

    /// Write bytes to a remote file over the live session's SFTP subsystem,
    /// creating/truncating it. Used by the MCP `ssh_write_file` tool.
    pub async fn write_file_from_bytes(
        &self,
        connection_id: &str,
        remote_path: &str,
        data: &[u8],
    ) -> Result<u64> {
        let connections = self.connections.read().await;
        let client = connections
            .get(connection_id)
            .ok_or_else(|| anyhow::anyhow!("Connection not found"))?;

        let client = client.read().await;
        client.upload_file_from_bytes(data, remote_path).await
    }

    /// True if a live session is currently open for `connection_id`.
    pub async fn has_connection(&self, connection_id: &str) -> bool {
        self.connections.read().await.contains_key(connection_id)
    }

    /// IDs of all currently open live sessions.
    pub async fn list_connection_ids(&self) -> Vec<String> {
        self.connections.read().await.keys().cloned().collect()
    }

    pub async fn start_pty_session(&self, connection_id: &str, cols: u32, rows: u32) -> Result<()> {
        let client = {
            let connections = self.connections.read().await;
            connections
                .get(connection_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Connection not found"))?
        };

        let client = client.read().await;
        let pty = client.create_pty_session(cols, rows).await?;
        self.pty_sessions
            .write()
            .await
            .insert(connection_id.to_string(), Arc::new(pty));

        Ok(())
    }

    pub async fn write_pty_input(&self, connection_id: &str, data: Vec<u8>) -> Result<()> {
        let pty = {
            let sessions = self.pty_sessions.read().await;
            sessions
                .get(connection_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("PTY session not found"))?
        };

        pty.input_tx
            .send(data)
            .await
            .map_err(|_| anyhow::anyhow!("PTY input channel closed"))
    }

    pub async fn read_pty_output(&self, connection_id: &str) -> Result<Option<Vec<u8>>> {
        let pty = {
            let sessions = self.pty_sessions.read().await;
            sessions
                .get(connection_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("PTY session not found"))?
        };

        let mut output = pty.output_rx.lock().await;
        match tokio::time::timeout(Duration::from_millis(150), output.recv()).await {
            Ok(Some(data)) => Ok(Some(data)),
            Ok(None) => Err(anyhow::anyhow!("PTY output channel closed")),
            Err(_) => Ok(None),
        }
    }

    #[allow(dead_code)]
    pub async fn resize_pty(&self, connection_id: &str, cols: u32, rows: u32) -> Result<()> {
        let pty = {
            let sessions = self.pty_sessions.read().await;
            sessions
                .get(connection_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("PTY session not found"))?
        };

        pty.resize_tx
            .send((cols, rows))
            .await
            .map_err(|_| anyhow::anyhow!("PTY resize channel closed"))
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn parse_ls_output(output: &str) -> Result<Vec<RemoteFileEntry>> {
    let mut entries = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("total ") {
            continue;
        }

        let fields = split_ls_line(line);
        if fields.len() < 8 {
            continue;
        }

        let permissions = fields[0].to_string();
        let name = fields[7..].join(" ");
        if name == "." {
            continue;
        }

        let kind = match permissions.chars().next() {
            Some('d') => RemoteFileKind::Directory,
            Some('l') => RemoteFileKind::Symlink,
            Some('-') => RemoteFileKind::File,
            _ => RemoteFileKind::Other,
        };
        let modified = format!("{} {}", fields[5], fields[6]);

        entries.push(RemoteFileEntry {
            name,
            kind,
            permissions,
            size: fields[4].to_string(),
            modified,
        });
    }

    entries.sort_by(|left, right| {
        left.kind
            .sort_rank()
            .cmp(&right.kind.sort_rank())
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });

    Ok(entries)
}

fn split_ls_line(line: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut start = None;

    for (index, ch) in line.char_indices() {
        if ch.is_whitespace() {
            if let Some(field_start) = start.take() {
                fields.push(&line[field_start..index]);
                if fields.len() == 7 {
                    let rest = line[index..].trim();
                    if !rest.is_empty() {
                        fields.push(rest);
                    }
                    return fields;
                }
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }

    if let Some(field_start) = start {
        fields.push(&line[field_start..]);
    }

    fields
}

impl RemoteFileKind {
    fn sort_rank(&self) -> u8 {
        match self {
            Self::Directory => 0,
            Self::Symlink => 1,
            Self::File => 2,
            Self::Other => 3,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Directory => "DIR",
            Self::Symlink => "LNK",
            Self::File => "FILE",
            Self::Other => "ITEM",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ls_output_with_spaces() {
        let output = "\
total 8
drwxr-xr-x  4 root root 4096 2026-06-09 17:20 .
drwxr-xr-x 12 root root 4096 2026-06-08 13:11 ..
-rw-r--r--  1 root root   42 2026-06-09 17:21 hello world.txt
lrwxrwxrwx  1 root root    7 2026-06-09 17:22 latest -> release
";

        let entries = parse_ls_output(output).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].name, "..");
        assert_eq!(entries[0].kind, RemoteFileKind::Directory);
        assert_eq!(entries[1].name, "latest -> release");
        assert_eq!(entries[1].kind, RemoteFileKind::Symlink);
        assert_eq!(entries[2].name, "hello world.txt");
    }

    #[test]
    fn quotes_shell_paths() {
        assert_eq!(shell_quote("/tmp/a b"), "'/tmp/a b'");
        assert_eq!(shell_quote("/tmp/a'b"), "'/tmp/a'\\''b'");
    }
}
