use std::{
    collections::HashMap,
    sync::Arc,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use anyhow::Result;
use tokio::sync::RwLock;

pub use crate::ssh::{AuthMethod, SftpEntry, SshConfig};

use crate::adb::AdbClient;
use crate::pty::PtySession;
use crate::ssh::SshClient;

/// A live backend connection: either an SSH session (russh) or an ADB device
/// (`adb` subprocess). The manager stores these uniformly so terminal tabs, the
/// SFTP file manager, and monitoring all work transparently over both.
enum Backend {
    Ssh(SshClient),
    Adb(AdbClient),
}

impl Backend {
    async fn disconnect(&mut self) -> Result<()> {
        match self {
            Backend::Ssh(c) => c.disconnect().await,
            Backend::Adb(c) => c.disconnect().await,
        }
    }

    async fn execute_command(&self, command: &str) -> Result<String> {
        match self {
            Backend::Ssh(c) => c.execute_command(command).await,
            Backend::Adb(c) => c.execute_command(command).await,
        }
    }

    async fn create_pty_session(&self, cols: u32, rows: u32) -> Result<PtySession> {
        match self {
            Backend::Ssh(c) => c.create_pty_session(cols, rows).await,
            Backend::Adb(c) => c.create_pty_session(cols, rows).await,
        }
    }

    async fn download_file(&self, remote_path: &str, local_path: &str) -> Result<u64> {
        match self {
            Backend::Ssh(c) => c.download_file(remote_path, local_path).await,
            Backend::Adb(c) => c.pull(remote_path, local_path, |_, _| {}).await,
        }
    }

    async fn upload_file(&self, local_path: &str, remote_path: &str) -> Result<u64> {
        match self {
            Backend::Ssh(c) => c.upload_file(local_path, remote_path).await,
            Backend::Adb(c) => c.push(local_path, remote_path, |_, _| {}).await,
        }
    }

    async fn sftp_read_dir(&self, path: &str) -> Result<Vec<SftpEntry>> {
        match self {
            Backend::Ssh(c) => c.sftp_read_dir(path).await,
            Backend::Adb(c) => c.list_dir(path).await,
        }
    }

    async fn sftp_realpath(&self, path: &str) -> Result<String> {
        match self {
            Backend::Ssh(c) => c.sftp_realpath(path).await,
            Backend::Adb(c) => c.realpath(path).await,
        }
    }

    async fn delete_path(&self, path: &str) -> Result<()> {
        match self {
            Backend::Ssh(c) => c.sftp_remove(path).await,
            Backend::Adb(c) => c.delete(path).await,
        }
    }

    async fn rename_path(&self, from: &str, to: &str) -> Result<()> {
        match self {
            Backend::Ssh(c) => c.sftp_rename(from, to).await,
            Backend::Adb(c) => c.rename(from, to).await,
        }
    }

    async fn upload_file_progress<F>(
        &self,
        local_path: &str,
        remote_path: &str,
        on_progress: F,
    ) -> Result<u64>
    where
        F: FnMut(u64, u64) + Send,
    {
        match self {
            Backend::Ssh(c) => {
                c.upload_file_progress(local_path, remote_path, on_progress)
                    .await
            }
            Backend::Adb(c) => c.push(local_path, remote_path, on_progress).await,
        }
    }

    async fn download_file_progress<F>(
        &self,
        remote_path: &str,
        local_path: &str,
        on_progress: F,
    ) -> Result<u64>
    where
        F: FnMut(u64, u64) + Send,
    {
        match self {
            Backend::Ssh(c) => {
                c.download_file_progress(remote_path, local_path, on_progress)
                    .await
            }
            Backend::Adb(c) => c.pull(remote_path, local_path, on_progress).await,
        }
    }

    async fn download_file_to_memory(&self, remote_path: &str) -> Result<Vec<u8>> {
        match self {
            Backend::Ssh(c) => c.download_file_to_memory(remote_path).await,
            Backend::Adb(c) => c.read_file(remote_path).await,
        }
    }

    async fn upload_file_from_bytes(&self, data: &[u8], remote_path: &str) -> Result<u64> {
        match self {
            Backend::Ssh(c) => c.upload_file_from_bytes(data, remote_path).await,
            Backend::Adb(c) => c.write_file(data, remote_path).await,
        }
    }
}

/// A GUI PTY handle: the live session plus the connection it belongs to (so the
/// manager can drop all of a connection's PTYs when it is closed).
struct PtyHandle {
    connection_id: String,
    session: Arc<PtySession>,
}

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
    connections: RwLock<HashMap<String, Arc<RwLock<Backend>>>>,
    /// Legacy single-PTY-per-connection map (keyed by connection id) used by the
    /// CLI's interactive `shell`.
    pty_sessions: RwLock<HashMap<String, Arc<PtySession>>>,
    /// GUI multi-PTY map keyed by a unique pty id — supports many concurrent
    /// terminal tabs, even several on the same connection.
    ptys: RwLock<HashMap<String, PtyHandle>>,
    next_pty_seq: AtomicU64,
    /// Detected stats platform per connection (Unix vs Windows), cached after the
    /// first successful snapshot so we don't re-probe both commands every sample.
    stats_platform: RwLock<HashMap<String, crate::monitor::StatsPlatform>>,
    /// Per-connection remote-exec policy (iOS PATH / su·sudo elevate). Applied
    /// inside [`Self::execute_command`] before the backend sees the command.
    exec_profiles: RwLock<HashMap<String, crate::ios_ssh::ExecProfile>>,
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
            ptys: RwLock::new(HashMap::new()),
            next_pty_seq: AtomicU64::new(1),
            stats_platform: RwLock::new(HashMap::new()),
            exec_profiles: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create_connection(&self, connection_id: String, config: SshConfig) -> Result<()> {
        self.create_connection_with_profile(connection_id, config, None)
            .await
    }

    /// Open an SSH session and optionally attach an [`ExecProfile`](crate::ios_ssh::ExecProfile)
    /// (iOS jailbreak PATH / privilege escalation) used by later `execute_command` calls.
    ///
    /// After a successful login the remote is probed once (`uname` + `/var/jb`);
    /// jailbroken iOS / Darwin hosts automatically get PATH wrapping so you can
    /// just type `host + root + password` without flipping a special toggle.
    pub async fn create_connection_with_profile(
        &self,
        connection_id: String,
        config: SshConfig,
        profile: Option<crate::ios_ssh::ExecProfile>,
    ) -> Result<()> {
        let mut client = SshClient::new();
        client.connect(&config).await?;

        let mut profile = profile.unwrap_or_default();
        // Best-effort platform sniff — never fails the connect.
        // Keep this short: a hung probe used to block ssh_session_open forever
        // (and hold Backend locks that deadlocked reconnect).
        if let Ok(Ok(probe)) = tokio::time::timeout(
            Duration::from_secs(8),
            client.execute_command_timeout(
                "uname -s 2>/dev/null; test -d /var/jb && echo __HAS_JB__",
                Duration::from_secs(8),
            ),
        )
        .await
        {
            let probe = probe.to_ascii_lowercase();
            if probe.contains("darwin") || probe.contains("__has_jb__") {
                profile.ios = true;
            }
        }
        // Do not auto-enable su/sudo for mobile@iOS: the SSH login password is
        // often different from the root password (as on many jailbreaks), and a
        // blind elevate wraps every exec into a failing `su`. Callers opt in via
        // `elevate:su` / `elevate:sudo` tags (or MCP `elevate=`).

        if profile.needs_wrap() {
            self.exec_profiles
                .write()
                .await
                .insert(connection_id.clone(), profile);
        } else {
            self.exec_profiles.write().await.remove(&connection_id);
        }
        self.insert_backend(connection_id, Backend::Ssh(client)).await;
        Ok(())
    }

    /// Replace (or clear) the exec profile for an already-open connection.
    pub async fn set_exec_profile(
        &self,
        connection_id: &str,
        profile: Option<crate::ios_ssh::ExecProfile>,
    ) {
        let mut profiles = self.exec_profiles.write().await;
        match profile {
            Some(p) => {
                profiles.insert(connection_id.to_string(), p);
            }
            None => {
                profiles.remove(connection_id);
            }
        }
    }

    /// Open an ADB connection to an Android device (`serial = host:port`) and
    /// register it under `connection_id`. The device must be authorized
    /// (`adb devices` shows `device`, not `unauthorized`).
    pub async fn create_adb_connection(&self, connection_id: String, serial: String) -> Result<()> {
        let mut client = AdbClient::new(serial);
        client.connect().await?;
        self.insert_backend(connection_id, Backend::Adb(client)).await;
        Ok(())
    }

    /// Insert a freshly-opened backend, tearing down any previous one registered
    /// under the same id first.
    ///
    /// Critical for iOS MCP reconnect: never await a Backend write-lock while
    /// still holding the connections map lock, and never wait forever on a
    /// hung prior exec that still holds the read lock.
    async fn insert_backend(&self, connection_id: String, backend: Backend) {
        let old = {
            let mut connections = self.connections.write().await;
            connections.remove(&connection_id)
        };
        if let Some(existing) = old {
            // Best-effort disconnect; abandon if a stuck reader holds the lock.
            match tokio::time::timeout(Duration::from_secs(2), existing.write()).await {
                Ok(mut guard) => {
                    let _ = tokio::time::timeout(Duration::from_secs(3), guard.disconnect()).await;
                }
                Err(_) => {
                    eprintln!(
                        "[ssh] abandoning prior session '{connection_id}' (disconnect lock busy)"
                    );
                }
            }
        }
        let mut connections = self.connections.write().await;
        connections.insert(connection_id, Arc::new(RwLock::new(backend)));
    }

    pub async fn close_connection(&self, connection_id: &str) -> Result<()> {
        self.pty_sessions.write().await.remove(connection_id);
        // Drop every GUI PTY that belongs to this connection.
        self.ptys
            .write()
            .await
            .retain(|_, handle| handle.connection_id != connection_id);
        // Forget the detected stats platform; a reconnect may target a different host.
        self.stats_platform.write().await.remove(connection_id);
        self.exec_profiles.write().await.remove(connection_id);

        let old = {
            let mut connections = self.connections.write().await;
            connections.remove(connection_id)
        };
        if let Some(client) = old {
            match tokio::time::timeout(Duration::from_secs(2), client.write()).await {
                Ok(mut guard) => {
                    let _ = tokio::time::timeout(Duration::from_secs(3), guard.disconnect()).await;
                }
                Err(_) => {
                    eprintln!(
                        "[ssh] close_connection: abandoning '{connection_id}' (lock busy)"
                    );
                }
            }
        }
        Ok(())
    }

    pub async fn execute_command(&self, connection_id: &str, command: &str) -> Result<String> {
        let wrapped = {
            let profiles = self.exec_profiles.read().await;
            match profiles.get(connection_id) {
                Some(profile) if profile.needs_wrap() => {
                    crate::ios_ssh::wrap_command(command, profile)
                }
                _ => command.to_string(),
            }
        };
        self.execute_command_raw(connection_id, &wrapped).await
    }

    /// Run a remote command **without** applying the session's iOS/elevate
    /// [`ExecProfile`](crate::ios_ssh::ExecProfile). Used by stats collectors
    /// (their scripts are self-contained and elevate quoting would break them).
    pub async fn execute_command_raw(
        &self,
        connection_id: &str,
        command: &str,
    ) -> Result<String> {
        // Clone the Arc so we don't hold the connections-map lock across exec.
        let backend = {
            let connections = self.connections.read().await;
            connections
                .get(connection_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Connection not found"))?
        };
        let client = backend.read().await;
        client.execute_command(command).await
    }

    /// Snapshot remote resource usage (CPU, memory, disk, network) in one call.
    /// Returns the raw combined output for [`crate::monitor::parse_snapshot`].
    pub async fn fetch_system_stats(&self, connection_id: &str) -> Result<String> {
        self.execute_command_raw(connection_id, &crate::monitor::stats_command())
            .await
    }

    /// OS-aware system snapshot: collects metrics with the right command for the
    /// remote platform (Linux `/proc`+`df`, Darwin `sysctl`/`vm_stat`, or Windows
    /// PowerShell CIM) and parses it into a [`StatsSnapshot`](crate::monitor::StatsSnapshot).
    ///
    /// The platform is detected on the first call and cached per connection, so
    /// steady-state sampling is a single round-trip. Collectors run via
    /// [`Self::execute_command_raw`] so iOS `elevate:` tags don't wrap them.
    pub async fn fetch_system_snapshot(
        &self,
        connection_id: &str,
    ) -> Result<crate::monitor::StatsSnapshot> {
        use crate::monitor::{
            darwin_stats_command, parse_darwin_snapshot, parse_snapshot,
            parse_windows_snapshot, stats_command, windows_stats_command, StatsPlatform,
        };

        // Fast path: platform already known for this connection.
        let cached = self.stats_platform.read().await.get(connection_id).copied();
        if let Some(platform) = cached {
            return match platform {
                StatsPlatform::Unix => {
                    let raw = self
                        .execute_command_raw(connection_id, &stats_command())
                        .await?;
                    Ok(parse_snapshot(&raw))
                }
                StatsPlatform::Darwin => {
                    let raw = self
                        .execute_command_raw(connection_id, &darwin_stats_command())
                        .await?;
                    Ok(parse_darwin_snapshot(&raw))
                }
                StatsPlatform::Windows => {
                    let raw = self
                        .execute_command_raw(connection_id, &windows_stats_command())
                        .await?;
                    Ok(parse_windows_snapshot(&raw))
                }
            };
        }

        // Detect: Linux → Darwin (jailbroken iOS / macOS) → Windows.
        // Darwin also has `df`, so a Unix snapshot is only trusted when /proc
        // yielded CPU jiffies or MemTotal — otherwise we'd mis-label iPhones
        // as Linux (mem=0, bogus disk headline) and never run the Darwin path.
        let unix_result = self
            .execute_command_raw(connection_id, &stats_command())
            .await;
        if let Ok(raw) = &unix_result {
            let snap = parse_snapshot(raw);
            let looks_like_linux = snap.cpu.is_some() || snap.mem_total_kb > 0;
            if !snap.is_empty() && looks_like_linux {
                self.stats_platform
                    .write()
                    .await
                    .insert(connection_id.to_string(), StatsPlatform::Unix);
                return Ok(snap);
            }
        }

        let darwin_result = self
            .execute_command_raw(connection_id, &darwin_stats_command())
            .await;
        if let Ok(raw) = &darwin_result {
            let snap = parse_darwin_snapshot(raw);
            if !snap.is_empty() {
                self.stats_platform
                    .write()
                    .await
                    .insert(connection_id.to_string(), StatsPlatform::Darwin);
                return Ok(snap);
            }
        }

        match self
            .execute_command_raw(connection_id, &windows_stats_command())
            .await
        {
            Ok(raw) => {
                let snap = parse_windows_snapshot(&raw);
                if !snap.is_empty() {
                    self.stats_platform
                        .write()
                        .await
                        .insert(connection_id.to_string(), StatsPlatform::Windows);
                    return Ok(snap);
                }
                unix_result.and_then(|_| {
                    darwin_result.and_then(|_| {
                        Err(anyhow::anyhow!(
                            "could not collect system stats (unsupported remote OS?)"
                        ))
                    })
                })
            }
            Err(win_err) => {
                if unix_result.is_ok() || darwin_result.is_ok() {
                    Err(anyhow::anyhow!(
                        "could not collect system stats (unsupported remote OS?)"
                    ))
                } else {
                    unix_result.and(darwin_result).and(Err(win_err))
                }
            }
        }
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

    /// Look up the live client for a connection (shared by the SFTP file
    /// manager methods below).
    async fn client(&self, connection_id: &str) -> Result<Arc<RwLock<Backend>>> {
        self.connections
            .read()
            .await
            .get(connection_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Connection not found"))
    }

    /// List a remote directory over SFTP (structured metadata, cross-platform).
    pub async fn sftp_list(&self, connection_id: &str, path: &str) -> Result<Vec<SftpEntry>> {
        let client = self.client(connection_id).await?;
        let client = client.read().await;
        client.sftp_read_dir(path).await
    }

    /// Resolve a remote path to its canonical absolute form (e.g. `"."` → home).
    pub async fn sftp_realpath(&self, connection_id: &str, path: &str) -> Result<String> {
        let client = self.client(connection_id).await?;
        let client = client.read().await;
        client.sftp_realpath(path).await
    }

    /// Recursively delete a remote file / directory.
    pub async fn sftp_delete(&self, connection_id: &str, path: &str) -> Result<()> {
        let client = self.client(connection_id).await?;
        let client = client.read().await;
        client.delete_path(path).await
    }

    /// Rename / move a remote path.
    pub async fn sftp_rename(&self, connection_id: &str, from: &str, to: &str) -> Result<()> {
        let client = self.client(connection_id).await?;
        let client = client.read().await;
        client.rename_path(from, to).await
    }

    /// Upload a local file to the remote with progress callbacks.
    pub async fn sftp_upload<F>(
        &self,
        connection_id: &str,
        local_path: &str,
        remote_path: &str,
        on_progress: F,
    ) -> Result<u64>
    where
        F: FnMut(u64, u64) + Send,
    {
        let client = self.client(connection_id).await?;
        let client = client.read().await;
        client
            .upload_file_progress(local_path, remote_path, on_progress)
            .await
    }

    /// Download a remote file to local with progress callbacks.
    pub async fn sftp_download<F>(
        &self,
        connection_id: &str,
        remote_path: &str,
        local_path: &str,
        on_progress: F,
    ) -> Result<u64>
    where
        F: FnMut(u64, u64) + Send,
    {
        let client = self.client(connection_id).await?;
        let client = client.read().await;
        client
            .download_file_progress(remote_path, local_path, on_progress)
            .await
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

    // ── GUI multi-PTY (keyed by unique pty id) ──────────────────────────────

    /// Open a new PTY shell on an already-open connection and return a unique
    /// pty id. Unlike [`start_pty_session`], several PTYs may coexist (one per
    /// GUI terminal tab), including multiple on the same connection.
    pub async fn open_pty(&self, connection_id: &str, cols: u32, rows: u32) -> Result<String> {
        let client = {
            let connections = self.connections.read().await;
            connections
                .get(connection_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Connection not found"))?
        };

        let pty = {
            let client = client.read().await;
            client.create_pty_session(cols, rows).await?
        };

        let seq = self.next_pty_seq.fetch_add(1, Ordering::Relaxed);
        let pty_id = format!("pty-{seq}");
        self.ptys.write().await.insert(
            pty_id.clone(),
            PtyHandle {
                connection_id: connection_id.to_string(),
                session: Arc::new(pty),
            },
        );
        Ok(pty_id)
    }

    async fn pty_by_id(&self, pty_id: &str) -> Result<Arc<PtySession>> {
        self.ptys
            .read()
            .await
            .get(pty_id)
            .map(|handle| handle.session.clone())
            .ok_or_else(|| anyhow::anyhow!("PTY session not found"))
    }

    /// Write input bytes (keystrokes) to a PTY by id.
    pub async fn write_pty(&self, pty_id: &str, data: Vec<u8>) -> Result<()> {
        let session = self.pty_by_id(pty_id).await?;
        session
            .input_tx
            .send(data)
            .await
            .map_err(|_| anyhow::anyhow!("PTY input channel closed"))
    }

    /// Read the next chunk of PTY output by id. Returns `Ok(None)` if no data
    /// arrived within a short poll window (so callers can re-check for teardown).
    pub async fn read_pty(&self, pty_id: &str) -> Result<Option<Vec<u8>>> {
        let session = self.pty_by_id(pty_id).await?;
        let mut output = session.output_rx.lock().await;
        match tokio::time::timeout(Duration::from_millis(150), output.recv()).await {
            Ok(Some(data)) => Ok(Some(data)),
            Ok(None) => Err(anyhow::anyhow!("PTY output channel closed")),
            Err(_) => Ok(None),
        }
    }

    /// Resize a PTY by id.
    pub async fn resize_pty_by_id(&self, pty_id: &str, cols: u32, rows: u32) -> Result<()> {
        let session = self.pty_by_id(pty_id).await?;
        session
            .resize_tx
            .send((cols, rows))
            .await
            .map_err(|_| anyhow::anyhow!("PTY resize channel closed"))
    }

    /// Close a PTY by id. Cancelling its token and dropping the session tears
    /// down the underlying SSH channel.
    pub async fn close_pty(&self, pty_id: &str) -> Result<()> {
        if let Some(handle) = self.ptys.write().await.remove(pty_id) {
            handle.session.cancel.cancel();
        }
        Ok(())
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
