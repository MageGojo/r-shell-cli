use anyhow::Result;
use russh::*;
use russh_keys::*;
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::pty::PtySession;

/// Preferred host-key algorithms advertised to the server, ordered from most to
/// least preferred.  RSA variants (including the legacy `ssh-rsa` / SHA-1) are
/// included so that older servers that only offer RSA host keys are still
/// reachable.  The `openssl` feature on `russh` / `russh-keys` must be enabled
/// for the RSA entries to have any effect.
pub static PREFERRED_HOST_KEY_ALGOS: &[russh_keys::key::Name] = &[
    russh_keys::key::ED25519,
    russh_keys::key::ECDSA_SHA2_NISTP256,
    russh_keys::key::ECDSA_SHA2_NISTP521,
    russh_keys::key::RSA_SHA2_256,
    russh_keys::key::RSA_SHA2_512,
    russh_keys::key::SSH_RSA,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    /// When true, skip host-key verification entirely (dangerous; opt-in via
    /// `--insecure`). When false (default), host keys are verified against
    /// `~/.ssh/known_hosts` with trust-on-first-use.
    #[serde(default)]
    pub insecure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuthMethod {
    Password {
        password: String,
    },
    PublicKey {
        key_path: String,
        passphrase: Option<String>,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct SshSession {
    pub id: String,
    pub config: SshConfig,
    pub connected: bool,
}

/// One entry from an SFTP directory listing.
///
/// Unlike the legacy `ls -la` parser ([`crate::native_backend::RemoteFileEntry`]),
/// this carries structured metadata straight from the SFTP protocol, so it works
/// on non-Linux hosts (e.g. Windows OpenSSH) and needs no text parsing.
#[derive(Debug, Clone)]
pub struct SftpEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    /// File size in bytes (0 for directories / unknown).
    pub size: u64,
    /// Unix-style permission string, e.g. `rwxr-xr-x` (empty if unknown).
    pub permissions: String,
    /// Last-modified time as Unix seconds (0 if the server omitted it).
    pub modified_unix: i64,
}

pub struct SshClient {
    session: Option<Arc<client::Handle<Client>>>,
}

pub struct Client {
    /// Host used to look up / record the key in `known_hosts`.
    host: String,
    /// Port used to look up / record the key in `known_hosts`.
    port: u16,
    /// When true, accept any host key without verification (dangerous).
    insecure: bool,
}

#[async_trait::async_trait]
impl client::Handler for Client {
    type Error = russh::Error;

    /// Verify the server's host key against `~/.ssh/known_hosts`.
    ///
    /// - `insecure`: accept anything (opt-in escape hatch).
    /// - known + matching key: accept.
    /// - unknown host (first connection): record the key (trust-on-first-use)
    ///   and accept.
    /// - known host but the key changed: reject. This is the MITM signal.
    async fn check_server_key(
        &mut self,
        server_public_key: &key::PublicKey,
    ) -> Result<bool, Self::Error> {
        if self.insecure {
            return Ok(true);
        }

        match russh_keys::check_known_hosts(&self.host, self.port, server_public_key) {
            // Key matches a recorded entry.
            Ok(true) => Ok(true),
            // Host not seen before: record it (TOFU) and accept.
            Ok(false) => {
                if let Err(error) =
                    russh_keys::learn_known_hosts(&self.host, self.port, server_public_key)
                {
                    eprintln!(
                        "[ssh] warning: could not record host key for {}:{} in known_hosts: {}",
                        self.host, self.port, error
                    );
                }
                eprintln!(
                    "[ssh] warning: permanently added '{}:{}' ({}) to known hosts.",
                    self.host,
                    self.port,
                    server_public_key.name()
                );
                Ok(true)
            }
            // Recorded key differs from what the server presented.
            Err(russh_keys::Error::KeyChanged { line }) => {
                eprintln!(
                    "[ssh] REMOTE HOST IDENTIFICATION HAS CHANGED for {}:{}!",
                    self.host, self.port
                );
                eprintln!(
                    "[ssh] The host key does not match the one in known_hosts (line {line}). \
                     This could be a man-in-the-middle attack. Connection refused. \
                     If you trust this change, remove the offending line from \
                     ~/.ssh/known_hosts, or reconnect with --insecure."
                );
                Ok(false)
            }
            // Could not read known_hosts (e.g. no home dir): fail closed.
            Err(error) => {
                eprintln!("[ssh] host key verification error: {error}");
                Ok(false)
            }
        }
    }
}

/// Password auth with keyboard-interactive fallback.
///
/// Many OpenSSH / jailbreak hosts advertise both `password` and
/// `keyboard-interactive`; some only accept the latter (PAM). Try password
/// first, then answer every kbd-interactive prompt with the same password.
/// Auth can hang on flaky jailbreak OpenSSH; keep it bounded.
const SSH_AUTH_TIMEOUT: Duration = Duration::from_secs(15);
/// Non-interactive exec (MCP / probe). Prevents a stuck channel from wedging
/// the whole session manager behind a Backend read lock.
const SSH_EXEC_TIMEOUT: Duration = Duration::from_secs(45);

async fn authenticate_with_password<H: client::Handler + Send>(
    session: &mut client::Handle<H>,
    username: &str,
    password: &str,
) -> Result<bool>
where
    H::Error: From<russh::Error> + Send,
{
    let auth = async {
        match session.authenticate_password(username, password).await {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(error) => {
                // Fall through to keyboard-interactive; some servers reject the
                // password method outright before kbd-interactive succeeds.
                eprintln!("[ssh] password auth error, trying keyboard-interactive: {error}");
            }
        }

        use client::KeyboardInteractiveAuthResponse as Kbd;
        let mut response = session
            .authenticate_keyboard_interactive_start(username, None)
            .await
            .map_err(|e| {
                anyhow::anyhow!("Password / keyboard-interactive authentication failed: {e}")
            })?;

        for _ in 0..8 {
            match response {
                Kbd::Success => return Ok(true),
                Kbd::Failure => return Ok(false),
                Kbd::InfoRequest { prompts, .. } => {
                    let answers = prompts.iter().map(|_| password.to_string()).collect();
                    response = session
                        .authenticate_keyboard_interactive_respond(answers)
                        .await
                        .map_err(|e| {
                            anyhow::anyhow!("keyboard-interactive response failed: {e}")
                        })?;
                }
            }
        }
        Ok(false)
    };

    tokio::time::timeout(SSH_AUTH_TIMEOUT, auth)
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "Authentication timed out after {} seconds",
                SSH_AUTH_TIMEOUT.as_secs()
            )
        })?
}

#[allow(dead_code)]
impl SshClient {
    pub fn new() -> Self {
        Self { session: None }
    }

    pub async fn connect(&mut self, config: &SshConfig) -> Result<()> {
        let public_key = match &config.auth_method {
            AuthMethod::PublicKey {
                key_path,
                passphrase,
            } => Some(load_private_key(key_path, passphrase.as_deref())?),
            AuthMethod::Password { .. } => None,
        };

        let ssh_config = client::Config {
            preferred: russh::Preferred {
                key: PREFERRED_HOST_KEY_ALGOS,
                ..russh::Preferred::DEFAULT
            },
            // Send a keepalive every 60 s. After 3 missed replies russh closes
            // the connection, preventing the server from silently dropping idle
            // sessions after hours of inactivity.
            keepalive_interval: Some(Duration::from_secs(60)),
            keepalive_max: 3,
            ..client::Config::default()
        };

        // Wi-Fi / USB-forwarded jailbreak hosts are often slower than LAN Linux boxes.
        let connection_timeout = Duration::from_secs(12);

        let handler = Client {
            host: config.host.clone(),
            port: config.port,
            insecure: config.insecure,
        };

        let mut ssh_session = tokio::time::timeout(
            connection_timeout,
            client::connect(Arc::new(ssh_config), (&config.host[..], config.port), handler)
        ).await
            .map_err(|_| anyhow::anyhow!("Connection timed out after 12 seconds. Please check the host address and network connectivity."))?
            .map_err(|e| anyhow::anyhow!("Failed to connect to {}:{}: {}", config.host, config.port, e))?;

        let authenticated = match &config.auth_method {
            AuthMethod::Password { password } => {
                authenticate_with_password(&mut ssh_session, &config.username, password).await?
            }
            AuthMethod::PublicKey {
                key_path: _,
                passphrase: _,
            } => {
                ssh_session
                    .authenticate_publickey(
                        &config.username,
                        Arc::new(public_key.expect("public key is loaded before connecting")),
                    )
                    .await
                    .map_err(|e| anyhow::anyhow!("Public key authentication failed: {}. The key may not be authorized on the server.", e))?
            }
        };

        if !authenticated {
            return Err(anyhow::anyhow!(
                "Authentication failed. Please check your credentials and try again."
            ));
        }

        self.session = Some(Arc::new(ssh_session));
        Ok(())
    }

    // Changed to &self instead of &mut self to allow concurrent access
    pub async fn execute_command(&self, command: &str) -> Result<String> {
        self.execute_command_timeout(command, SSH_EXEC_TIMEOUT).await
    }

    /// Like [`Self::execute_command`] but with an explicit timeout (used for
    /// short iOS probes during session open).
    pub async fn execute_command_timeout(
        &self,
        command: &str,
        timeout: Duration,
    ) -> Result<String> {
        if let Some(session) = &self.session {
            let run = async {
                let mut channel = session.channel_open_session().await?;
                channel.exec(true, command).await?;

                let mut output = String::new();
                let mut code = None;
                let mut eof_received = false;
                let mut server_closed = false;

                loop {
                    let msg = channel.wait().await;
                    match msg {
                        Some(ChannelMsg::Data { ref data }) => {
                            output.push_str(&String::from_utf8_lossy(data));
                        }
                        Some(ChannelMsg::ExitStatus { exit_status }) => {
                            code = Some(exit_status);
                            if eof_received {
                                break;
                            }
                        }
                        Some(ChannelMsg::Eof) => {
                            eof_received = true;
                            if code.is_some() {
                                break;
                            }
                        }
                        Some(ChannelMsg::Close) => {
                            server_closed = true;
                            break;
                        }
                        None => {
                            server_closed = true;
                            break;
                        }
                        _ => {}
                    }
                }

                // Send SSH_MSG_CHANNEL_CLOSE if the server hasn't already closed the channel.
                // Without this, russh's session keeps the channel in its internal map until
                // the session is torn down, causing per-poll memory growth.
                if !server_closed {
                    let _ = channel.close().await;
                }

                // Consider success if we got output and no explicit error code, or code 0
                match code {
                    Some(0) => Ok(output),
                    None if !output.is_empty() => Ok(output), // No exit code but got output = success
                    _ => Err(anyhow::anyhow!("Command failed with code: {:?}", code)),
                }
            };

            tokio::time::timeout(timeout, run).await.map_err(|_| {
                anyhow::anyhow!(
                    "Remote command timed out after {} seconds",
                    timeout.as_secs()
                )
            })?
        } else {
            Err(anyhow::anyhow!("Not connected"))
        }
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(session) = self.session.take() {
            // Try to unwrap Arc, if we're the only owner
            match Arc::try_unwrap(session) {
                Ok(session) => {
                    session
                        .disconnect(Disconnect::ByApplication, "", "English")
                        .await?;
                }
                Err(arc_session) => {
                    // Other references exist, just drop our reference
                    drop(arc_session);
                }
            }
        }
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.session.is_some()
    }

    /// Create a persistent PTY shell session (like ttyd)
    /// This enables interactive commands like vim, less, more, top, etc.
    pub async fn create_pty_session(&self, cols: u32, rows: u32) -> Result<PtySession> {
        if let Some(session) = &self.session {
            // Open a new SSH channel
            let mut channel = session.channel_open_session().await?;

            // Request PTY with terminal type and dimensions
            // Similar to ttyd's approach: xterm-256color terminal
            channel
                .request_pty(
                    true,             // want_reply
                    "xterm-256color", // terminal type (like ttyd)
                    cols,             // columns
                    rows,             // rows
                    0,                // pixel_width (not used)
                    0,                // pixel_height (not used)
                    &[],              // terminal modes
                )
                .await?;

            // Start interactive shell
            channel.request_shell(true).await?;

            // Create channels for bidirectional communication (like ttyd's pty_buf)
            // Increased capacity for better buffering during fast input
            let (input_tx, mut input_rx) = mpsc::channel::<Vec<u8>>(1000); // Increased from 100
            let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>(128); // Bounded: back-pressure to SSH window

            // Clone channel for input task
            let input_channel = channel.make_writer();

            // Create a channel for resize requests
            let (resize_tx, mut resize_rx) = mpsc::channel::<(u32, u32)>(16);

            // Spawn task to handle input (frontend → SSH)
            // This is similar to ttyd's pty_write and INPUT command handling
            // Key: immediate write + flush for responsiveness
            tokio::spawn(async move {
                let mut writer = input_channel;
                while let Some(data) = input_rx.recv().await {
                    // Write data immediately
                    if let Err(e) = writer.write_all(&data).await {
                        eprintln!("[PTY] Failed to send data to SSH: {}", e);
                        break;
                    }
                    // Critical: flush immediately after write (like ttyd)
                    // This ensures data is sent to PTY without buffering delay
                    if let Err(e) = writer.flush().await {
                        eprintln!("[PTY] Failed to flush data to SSH: {}", e);
                        break;
                    }
                }
            });

            // Spawn task to handle output (SSH → frontend) AND resize requests.
            // The channel must stay in this task because `wait()` requires `&mut self`,
            // but we also need `window_change()` which only requires `&self`.
            // We use `tokio::select!` to multiplex between output reading and resize.
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        msg = channel.wait() => {
                            match msg {
                                Some(ChannelMsg::Data { data }) => {
                                    if output_tx.send(data.to_vec()).await.is_err() {
                                        break;
                                    }
                                }
                                Some(ChannelMsg::ExtendedData { data, .. }) => {
                                    // stderr data (also send to output)
                                    if output_tx.send(data.to_vec()).await.is_err() {
                                        break;
                                    }
                                }
                                Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => {
                                    eprintln!("[PTY] Channel closed");
                                    break;
                                }
                                Some(ChannelMsg::ExitStatus { exit_status }) => {
                                    eprintln!("[PTY] Process exited with status: {}", exit_status);
                                }
                                _ => {}
                            }
                        }
                        resize = resize_rx.recv() => {
                            match resize {
                                Some((cols, rows)) => {
                                    if let Err(e) = channel.window_change(cols, rows, 0, 0).await {
                                        eprintln!("[PTY] Failed to send window change: {}", e);
                                    } else {
                                        eprintln!("[PTY] Window changed to {}x{}", cols, rows);
                                    }
                                }
                                None => {
                                    // resize channel closed, session is being torn down
                                    break;
                                }
                            }
                        }
                    }
                }
            });

            Ok(PtySession {
                input_tx,
                output_rx: Arc::new(tokio::sync::Mutex::new(output_rx)),
                resize_tx,
                cancel: CancellationToken::new(),
            })
        } else {
            Err(anyhow::anyhow!("Not connected"))
        }
    }

    pub async fn download_file(&self, remote_path: &str, local_path: &str) -> Result<u64> {
        if let Some(session) = &self.session {
            // Open SFTP subsystem
            let channel = session.channel_open_session().await?;
            channel.request_subsystem(true, "sftp").await?;
            let sftp = SftpSession::new(channel.into_stream()).await?;

            // Open remote file for reading
            let mut remote_file = sftp.open(remote_path).await?;

            // Read file content
            let mut buffer = Vec::new();
            let mut temp_buf = vec![0u8; 8192];
            let mut total_bytes = 0u64;

            loop {
                let n = remote_file.read(&mut temp_buf).await?;
                if n == 0 {
                    break;
                }
                buffer.extend_from_slice(&temp_buf[..n]);
                total_bytes += n as u64;
            }

            // Write to local file
            tokio::fs::write(local_path, buffer).await?;

            Ok(total_bytes)
        } else {
            Err(anyhow::anyhow!("Not connected"))
        }
    }

    pub async fn download_file_to_memory(&self, remote_path: &str) -> Result<Vec<u8>> {
        if let Some(session) = &self.session {
            // Open SFTP subsystem
            let channel = session.channel_open_session().await?;
            channel.request_subsystem(true, "sftp").await?;
            let sftp = SftpSession::new(channel.into_stream()).await?;

            // Open remote file for reading
            let mut remote_file = sftp.open(remote_path).await?;

            // Read file content
            let mut buffer = Vec::new();
            let mut temp_buf = vec![0u8; 8192];

            loop {
                let n = remote_file.read(&mut temp_buf).await?;
                if n == 0 {
                    break;
                }
                buffer.extend_from_slice(&temp_buf[..n]);
            }

            Ok(buffer)
        } else {
            Err(anyhow::anyhow!("Not connected"))
        }
    }

    pub async fn upload_file(&self, local_path: &str, remote_path: &str) -> Result<u64> {
        if let Some(session) = &self.session {
            // Read local file
            let data = tokio::fs::read(local_path).await?;
            let total_bytes = data.len() as u64;

            // Open SFTP subsystem
            let channel = session.channel_open_session().await?;
            channel.request_subsystem(true, "sftp").await?;
            let sftp = SftpSession::new(channel.into_stream()).await?;

            // Create remote file for writing
            let mut remote_file = sftp.create(remote_path).await?;

            // Write data in chunks
            let mut offset = 0;
            let chunk_size = 8192;

            while offset < data.len() {
                let end = std::cmp::min(offset + chunk_size, data.len());
                remote_file.write_all(&data[offset..end]).await?;
                offset = end;
            }

            remote_file.flush().await?;

            Ok(total_bytes)
        } else {
            Err(anyhow::anyhow!("Not connected"))
        }
    }

    pub async fn upload_file_from_bytes(&self, data: &[u8], remote_path: &str) -> Result<u64> {
        if let Some(session) = &self.session {
            let total_bytes = data.len() as u64;

            // Open SFTP subsystem
            let channel = session.channel_open_session().await?;
            channel.request_subsystem(true, "sftp").await?;
            let sftp = SftpSession::new(channel.into_stream()).await?;

            // Create remote file for writing
            let mut remote_file = sftp.create(remote_path).await?;

            // Write data in chunks
            let mut offset = 0;
            let chunk_size = 8192;

            while offset < data.len() {
                let end = std::cmp::min(offset + chunk_size, data.len());
                remote_file.write_all(&data[offset..end]).await?;
                offset = end;
            }

            remote_file.flush().await?;

            Ok(total_bytes)
        } else {
            Err(anyhow::anyhow!("Not connected"))
        }
    }

    // ── SFTP file manager (structured, cross-platform) ──────────────────────

    /// Open a fresh SFTP subsystem channel on the live session.
    async fn open_sftp(&self) -> Result<SftpSession> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        let channel = session.channel_open_session().await?;
        channel.request_subsystem(true, "sftp").await?;
        let sftp = SftpSession::new(channel.into_stream()).await?;
        Ok(sftp)
    }

    /// List a remote directory over SFTP, returning structured metadata. Works
    /// on any SFTP server (Linux, Windows OpenSSH, …) — no `ls` parsing.
    pub async fn sftp_read_dir(&self, path: &str) -> Result<Vec<SftpEntry>> {
        let sftp = self.open_sftp().await?;
        let read_dir = sftp.read_dir(path).await?;

        let mut entries = Vec::new();
        for entry in read_dir {
            let metadata = entry.metadata();
            let file_type = entry.file_type();
            entries.push(SftpEntry {
                name: entry.file_name(),
                is_dir: file_type.is_dir(),
                is_symlink: file_type.is_symlink(),
                size: metadata.size.unwrap_or(0),
                permissions: metadata.permissions().to_string(),
                modified_unix: metadata.mtime.map(|t| t as i64).unwrap_or(0),
            });
        }

        let _ = sftp.close().await;
        Ok(entries)
    }

    /// Resolve a remote path to its canonical absolute form (also used to fetch
    /// the default/home directory via `sftp_realpath(".")`).
    pub async fn sftp_realpath(&self, path: &str) -> Result<String> {
        let sftp = self.open_sftp().await?;
        let real = sftp.canonicalize(path).await?;
        let _ = sftp.close().await;
        Ok(real)
    }

    /// Recursively delete a remote path over SFTP (file, symlink, or directory).
    pub async fn sftp_remove(&self, path: &str) -> Result<()> {
        let sftp = self.open_sftp().await?;
        let result = sftp_remove_recursive(&sftp, path).await;
        let _ = sftp.close().await;
        result
    }

    /// Rename / move a remote path over SFTP.
    pub async fn sftp_rename(&self, from: &str, to: &str) -> Result<()> {
        let sftp = self.open_sftp().await?;
        let result = sftp
            .rename(from, to)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()));
        let _ = sftp.close().await;
        result
    }

    /// Upload a local file to the remote over SFTP, invoking `on_progress`
    /// (transferred, total) as bytes are written. Streamed in 32 KiB chunks so
    /// large files never buffer fully in memory.
    pub async fn upload_file_progress<F>(
        &self,
        local_path: &str,
        remote_path: &str,
        mut on_progress: F,
    ) -> Result<u64>
    where
        F: FnMut(u64, u64) + Send,
    {
        let total = tokio::fs::metadata(local_path).await?.len();
        let mut local = tokio::fs::File::open(local_path).await?;

        let sftp = self.open_sftp().await?;
        let mut remote = sftp.create(remote_path).await?;

        let mut buffer = vec![0u8; 32 * 1024];
        let mut transferred = 0u64;
        on_progress(0, total);
        loop {
            let read = local.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            remote.write_all(&buffer[..read]).await?;
            transferred += read as u64;
            on_progress(transferred, total);
        }
        remote.flush().await?;
        remote.shutdown().await?;
        let _ = sftp.close().await;
        Ok(transferred)
    }

    /// Download a remote file to local over SFTP, invoking `on_progress`
    /// (transferred, total) as bytes arrive. Streamed in 32 KiB chunks.
    pub async fn download_file_progress<F>(
        &self,
        remote_path: &str,
        local_path: &str,
        mut on_progress: F,
    ) -> Result<u64>
    where
        F: FnMut(u64, u64) + Send,
    {
        let sftp = self.open_sftp().await?;
        let total = sftp.metadata(remote_path).await?.size.unwrap_or(0);
        let mut remote = sftp.open(remote_path).await?;
        let mut local = tokio::fs::File::create(local_path).await?;

        let mut buffer = vec![0u8; 32 * 1024];
        let mut transferred = 0u64;
        on_progress(0, total);
        loop {
            let read = remote.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            local.write_all(&buffer[..read]).await?;
            transferred += read as u64;
            on_progress(transferred, total);
        }
        local.flush().await?;
        let _ = sftp.close().await;
        Ok(transferred)
    }
}

fn load_private_key(key_path: &str, passphrase: Option<&str>) -> Result<key::KeyPair> {
    let expanded_path = expand_key_path(key_path);

    if !std::path::Path::new(&expanded_path).exists() {
        return Err(anyhow::anyhow!(
            "SSH key file not found: {}. Please check the file path and try again.",
            key_path
        ));
    }

    // Read the key file and normalise CRLF line endings so that keys created or
    // edited on Windows are parsed correctly by russh-keys' PEM/OpenSSH decoder.
    let key_content = std::fs::read_to_string(&expanded_path)
        .map_err(|e| anyhow::anyhow!("Failed to read SSH key file {}: {}", key_path, e))?;
    let key_content = key_content.replace("\r\n", "\n");

    decode_secret_key(&key_content, passphrase).map_err(|e| {
        if e.to_string().contains("encrypted") || e.to_string().contains("passphrase") {
            anyhow::anyhow!(
                "Failed to decrypt SSH key. The key may be encrypted. Please provide the correct passphrase."
            )
        } else {
            anyhow::anyhow!(
                "Failed to load SSH key from {}: {}. Ensure the file is a valid SSH private key (RSA, Ed25519, or ECDSA).",
                key_path, e
            )
        }
    })
}

fn expand_key_path(key_path: &str) -> String {
    if key_path.starts_with("~/") || key_path.starts_with("~\\") {
        if let Some(home) = dirs::home_dir() {
            let home_str = home.to_string_lossy();
            key_path.replacen('~', &home_str, 1)
        } else {
            key_path.to_string()
        }
    } else {
        key_path.to_string()
    }
}

/// Depth-first recursive remove over an open SFTP session: directories are
/// emptied then removed; files and symlinks are unlinked directly. Boxed so the
/// async recursion has a known size.
fn sftp_remove_recursive<'a>(
    sftp: &'a SftpSession,
    path: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let meta = sftp
            .symlink_metadata(path)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        // Recurse into real directories only (never follow a symlink to a dir).
        if meta.file_type().is_dir() {
            let entries = sftp
                .read_dir(path)
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            for entry in entries {
                let name = entry.file_name();
                if name == "." || name == ".." {
                    continue;
                }
                let child = format!("{}/{}", path.trim_end_matches('/'), name);
                sftp_remove_recursive(sftp, &child).await?;
            }
            sftp.remove_dir(path)
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        } else {
            sftp.remove_file(path)
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests;
