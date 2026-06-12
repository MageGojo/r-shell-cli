//! R-Shell CLI — a Rust-native command-line SSH workspace.
//!
//! Core modules (`ssh`, `native_backend`, `model`, `storage`, `monitor`, `mcp`)
//! are exposed through a `clap`-based command-line interface:
//!
//! - `connections` — manage saved SSH connections in `workspace.json`
//! - `exec`        — run a single remote command
//! - `shell`       — open an interactive PTY shell
//! - `ls`          — list a remote directory
//! - `upload` / `download` — single-file SFTP transfer
//! - `stats`       — snapshot remote system resource usage
//! - `mcp`         — run the local MCP server

mod mcp;
mod model;
mod monitor;
mod native_backend;
mod ssh;
mod storage;

use std::io::{Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};

use model::{ConnectionStatus, SavedConnection};
use native_backend::{AuthMethod, NativeConnectionManager, RemoteFileEntry, SshConfig};

/// R-Shell — a command-line SSH workspace (connections, exec, shell, SFTP, MCP).
#[derive(Parser, Debug)]
#[command(name = "r-shell", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Manage saved SSH connections (stored in workspace.json).
    #[command(subcommand)]
    Connections(ConnectionsCommand),

    /// Run a single command on a remote host and print its output.
    Exec(ExecArgs),

    /// Open an interactive shell (PTY) on a remote host.
    Shell(TargetArgs),

    /// List a remote directory.
    Ls(LsArgs),

    /// Upload a local file to a remote host over SFTP.
    Upload(UploadArgs),

    /// Download a remote file to the local machine over SFTP.
    Download(DownloadArgs),

    /// Print a one-shot snapshot of remote system resource usage.
    Stats(TargetArgs),

    /// Run the local MCP server (Streamable HTTP on 127.0.0.1:9123/mcp).
    Mcp,
}

#[derive(Subcommand, Debug)]
enum ConnectionsCommand {
    /// List all saved connections.
    List(ListArgs),
    /// Add a new saved connection.
    Add(AddArgs),
    /// Update fields of an existing saved connection.
    Update(UpdateArgs),
    /// Remove a saved connection.
    Remove(RemoveArgs),
}

#[derive(Args, Debug)]
struct ListArgs {
    /// Output as JSON instead of a table.
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct AddArgs {
    /// Display name for the connection.
    #[arg(long)]
    name: String,
    /// Remote host (IP or hostname).
    #[arg(long)]
    host: String,
    /// SSH username.
    #[arg(long)]
    username: String,
    /// SSH port.
    #[arg(long, default_value_t = 22)]
    port: u16,
    /// Authentication method.
    #[arg(long, value_enum, default_value_t = AuthKind::Password)]
    auth: AuthKind,
    /// Password (for password auth). Prefer the interactive prompt for secrets.
    #[arg(long)]
    password: Option<String>,
    /// Private key path (for publickey auth).
    #[arg(long)]
    key_path: Option<String>,
    /// Private key passphrase (for an encrypted key).
    #[arg(long)]
    passphrase: Option<String>,
    /// Folder to organize this connection under.
    #[arg(long, default_value = "All Connections")]
    folder: String,
    /// Free-form description.
    #[arg(long, default_value = "")]
    description: String,
}

#[derive(Args, Debug)]
struct UpdateArgs {
    /// Connection id (see `connections list`).
    connection_id: String,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    host: Option<String>,
    #[arg(long)]
    username: Option<String>,
    #[arg(long)]
    port: Option<u16>,
    #[arg(long, value_enum)]
    auth: Option<AuthKind>,
    #[arg(long)]
    password: Option<String>,
    #[arg(long)]
    key_path: Option<String>,
    #[arg(long)]
    passphrase: Option<String>,
    #[arg(long)]
    folder: Option<String>,
    #[arg(long)]
    description: Option<String>,
}

#[derive(Args, Debug)]
struct RemoveArgs {
    /// Connection id (see `connections list`).
    connection_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum AuthKind {
    Password,
    Publickey,
}

/// Flags shared by every command that needs to reach a remote host. A target is
/// either a saved connection (by id or name) or an ad-hoc `--host`/`--user`.
#[derive(Args, Debug, Clone)]
struct TargetArgs {
    /// Saved connection id or name to use.
    #[arg(long, short = 'c')]
    connection: Option<String>,

    /// Ad-hoc host (IP or hostname). Use instead of `--connection`.
    #[arg(long)]
    host: Option<String>,
    /// Ad-hoc SSH username (used with `--host`).
    #[arg(long, short = 'u')]
    user: Option<String>,
    /// Ad-hoc SSH port (used with `--host`).
    #[arg(long, short = 'p', default_value_t = 22)]
    port: u16,
    /// Ad-hoc password (used with `--host`).
    #[arg(long)]
    password: Option<String>,
    /// Ad-hoc private key path (used with `--host`).
    #[arg(long)]
    key_path: Option<String>,
    /// Ad-hoc private key passphrase (used with `--host`).
    #[arg(long)]
    passphrase: Option<String>,

    /// Skip SSH host-key verification (DANGEROUS: disables MITM protection).
    #[arg(long)]
    insecure: bool,
}

#[derive(Args, Debug)]
struct ExecArgs {
    #[command(flatten)]
    target: TargetArgs,
    /// The command to run on the remote host.
    #[arg(required = true, trailing_var_arg = true)]
    command: Vec<String>,
}

#[derive(Args, Debug)]
struct LsArgs {
    #[command(flatten)]
    target: TargetArgs,
    /// Remote directory to list.
    #[arg(default_value = ".")]
    path: String,
    /// Output as JSON instead of a table.
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct UploadArgs {
    #[command(flatten)]
    target: TargetArgs,
    /// Local file to upload.
    local_path: String,
    /// Remote destination path.
    remote_path: String,
}

#[derive(Args, Debug)]
struct DownloadArgs {
    #[command(flatten)]
    target: TargetArgs,
    /// Remote file to download.
    remote_path: String,
    /// Local destination path.
    local_path: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Connections(cmd) => run_connections(cmd),
        Command::Exec(args) => block_on(cmd_exec(args)),
        Command::Shell(target) => block_on(cmd_shell(target)),
        Command::Ls(args) => block_on(cmd_ls(args)),
        Command::Upload(args) => block_on(cmd_upload(args)),
        Command::Download(args) => block_on(cmd_download(args)),
        Command::Stats(target) => block_on(cmd_stats(target)),
        Command::Mcp => block_on(cmd_mcp()),
    }
}

/// Build a fresh multi-threaded Tokio runtime and drive `future` to completion.
fn block_on<F: std::future::Future<Output = Result<()>>>(future: F) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new().context("failed to create Tokio runtime")?;
    runtime.block_on(future)
}

// ---------------------------------------------------------------------------
// connections subcommands (synchronous: pure workspace.json read/modify/write)
// ---------------------------------------------------------------------------

fn run_connections(cmd: ConnectionsCommand) -> Result<()> {
    match cmd {
        ConnectionsCommand::List(args) => connections_list(args),
        ConnectionsCommand::Add(args) => connections_add(args),
        ConnectionsCommand::Update(args) => connections_update(args),
        ConnectionsCommand::Remove(args) => connections_remove(args),
    }
}

fn connections_list(args: ListArgs) -> Result<()> {
    let workspace = storage::load_workspace();

    if args.json {
        let sanitized: Vec<_> = workspace
            .connections
            .iter()
            .map(SavedConnection::sanitized)
            .collect();
        println!("{}", serde_json::to_string_pretty(&sanitized)?);
        return Ok(());
    }

    if workspace.connections.is_empty() {
        println!("No saved connections. Add one with `r-shell connections add`.");
        return Ok(());
    }

    println!(
        "{:<20}  {:<22}  {:<22}  {:<10}  {}",
        "ID", "NAME", "HOST", "AUTH", "FOLDER"
    );
    for connection in &workspace.connections {
        let host = format!(
            "{}@{}:{}",
            connection.username, connection.host, connection.port
        );
        println!(
            "{:<20}  {:<22}  {:<22}  {:<10}  {}",
            truncate(&connection.id, 20),
            truncate(&connection.name, 22),
            truncate(&host, 22),
            connection.auth_method,
            connection.folder,
        );
    }
    Ok(())
}

fn connections_add(args: AddArgs) -> Result<()> {
    let name = args.name.trim().to_string();
    let host = args.host.trim().to_string();
    let username = args.username.trim().to_string();
    if name.is_empty() || host.is_empty() || username.is_empty() || args.port == 0 {
        bail!("name, host, username, and a valid port are required");
    }

    let auth_method = match args.auth {
        AuthKind::Password => "password",
        AuthKind::Publickey => "publickey",
    }
    .to_string();

    let folder = args.folder.trim();
    let connection = SavedConnection {
        id: SavedConnection::new_id(),
        name,
        host,
        port: args.port,
        username,
        protocol: "SSH".to_string(),
        folder: if folder.is_empty() {
            "All Connections".to_string()
        } else {
            folder.to_string()
        },
        tags: Vec::new(),
        description: args.description,
        auth_method,
        password: args.password.filter(|value| !value.is_empty()),
        private_key_path: args.key_path.filter(|value| !value.is_empty()),
        passphrase: args.passphrase.filter(|value| !value.is_empty()),
        status: ConnectionStatus::Disconnected,
    };

    let mut workspace = storage::load_workspace();
    let id = connection.id.clone();
    workspace.connections.push(connection);
    workspace.active_connection_id = Some(id.clone());
    storage::save_workspace(&workspace)?;

    println!("Added connection {id}");
    Ok(())
}

fn connections_update(args: UpdateArgs) -> Result<()> {
    let mut workspace = storage::load_workspace();
    let connection = workspace
        .connections
        .iter_mut()
        .find(|connection| connection.id == args.connection_id)
        .ok_or_else(|| anyhow!("SSH connection not found: {}", args.connection_id))?;

    if let Some(name) = args.name.map(|value| value.trim().to_string()) {
        if !name.is_empty() {
            connection.name = name;
        }
    }
    if let Some(host) = args.host.map(|value| value.trim().to_string()) {
        if !host.is_empty() {
            connection.host = host;
        }
    }
    if let Some(username) = args.username.map(|value| value.trim().to_string()) {
        if !username.is_empty() {
            connection.username = username;
        }
    }
    if let Some(port) = args.port {
        if port == 0 {
            bail!("port must be greater than 0");
        }
        connection.port = port;
    }
    if let Some(auth) = args.auth {
        connection.auth_method = match auth {
            AuthKind::Password => "password",
            AuthKind::Publickey => "publickey",
        }
        .to_string();
    }
    if let Some(password) = args.password {
        connection.password = (!password.is_empty()).then_some(password);
    }
    if let Some(key_path) = args.key_path {
        connection.private_key_path = (!key_path.is_empty()).then_some(key_path);
    }
    if let Some(passphrase) = args.passphrase {
        connection.passphrase = (!passphrase.is_empty()).then_some(passphrase);
    }
    if let Some(folder) = args.folder.map(|value| value.trim().to_string()) {
        if !folder.is_empty() {
            connection.folder = folder;
        }
    }
    if let Some(description) = args.description {
        connection.description = description;
    }

    let id = connection.id.clone();
    storage::save_workspace(&workspace)?;
    println!("Updated connection {id}");
    Ok(())
}

fn connections_remove(args: RemoveArgs) -> Result<()> {
    let mut workspace = storage::load_workspace();
    let index = workspace
        .connections
        .iter()
        .position(|connection| connection.id == args.connection_id)
        .ok_or_else(|| anyhow!("SSH connection not found: {}", args.connection_id))?;

    let removed = workspace.connections.remove(index);
    workspace.tabs.retain(|tab| tab.connection_id != removed.id);
    if workspace.active_connection_id.as_deref() == Some(removed.id.as_str()) {
        workspace.active_connection_id = workspace
            .connections
            .first()
            .map(|connection| connection.id.clone());
    }
    storage::save_workspace(&workspace)?;

    println!("Removed connection {} ({})", removed.id, removed.name);
    Ok(())
}

// ---------------------------------------------------------------------------
// Remote-target resolution
// ---------------------------------------------------------------------------

const CONNECTION_ID: &str = "cli";

/// Resolve [`TargetArgs`] into an [`SshConfig`]. Prefers an explicit
/// `--connection` (saved), otherwise falls back to ad-hoc `--host` flags.
fn resolve_target(target: &TargetArgs) -> Result<SshConfig> {
    if let Some(reference) = &target.connection {
        let workspace = storage::load_workspace();
        let connection = workspace
            .connections
            .iter()
            .find(|connection| connection.id == *reference || connection.name == *reference)
            .ok_or_else(|| anyhow!("saved connection not found: {reference}"))?;
        return ssh_config_for_connection(connection, target.insecure);
    }

    let host = target
        .host
        .clone()
        .ok_or_else(|| anyhow!("provide either --connection <id|name> or --host <host>"))?;
    let username = target
        .user
        .clone()
        .ok_or_else(|| anyhow!("--user is required when using --host"))?;

    let auth_method = if let Some(key_path) = target
        .key_path
        .clone()
        .filter(|value| !value.trim().is_empty())
    {
        AuthMethod::PublicKey {
            key_path,
            passphrase: target.passphrase.clone(),
        }
    } else {
        let password = match target.password.clone() {
            Some(password) => password,
            None => prompt_password(&format!("{username}@{host}'s password: "))?,
        };
        AuthMethod::Password { password }
    };

    Ok(SshConfig {
        host,
        port: target.port,
        username,
        auth_method,
        insecure: target.insecure,
    })
}

/// Convert a saved [`SavedConnection`] into a connectable [`SshConfig`],
/// prompting for a missing password when needed.
fn ssh_config_for_connection(connection: &SavedConnection, insecure: bool) -> Result<SshConfig> {
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
            let password = match connection
                .password
                .clone()
                .filter(|value| !value.is_empty())
            {
                Some(password) => password,
                None => prompt_password(&format!(
                    "{}@{}'s password: ",
                    connection.username, connection.host
                ))?,
            };
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

/// Connect to the resolved target and hand the live manager to `body`.
async fn with_connection<F, Fut, T>(target: &TargetArgs, body: F) -> Result<T>
where
    F: FnOnce(Arc<NativeConnectionManager>) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let config = resolve_target(target)?;
    let manager = Arc::new(NativeConnectionManager::new());
    manager
        .create_connection(CONNECTION_ID.to_string(), config)
        .await
        .context("failed to establish SSH connection")?;

    let result = body(manager.clone()).await;
    let _ = manager.close_connection(CONNECTION_ID).await;
    result
}

// ---------------------------------------------------------------------------
// One-shot remote commands
// ---------------------------------------------------------------------------

async fn cmd_exec(args: ExecArgs) -> Result<()> {
    let command = args.command.join(" ");
    with_connection(&args.target, |manager| async move {
        let output = manager.execute_command(CONNECTION_ID, &command).await?;
        print!("{output}");
        if !output.ends_with('\n') {
            println!();
        }
        Ok(())
    })
    .await
}

async fn cmd_ls(args: LsArgs) -> Result<()> {
    let path = args.path.clone();
    let as_json = args.json;
    with_connection(&args.target, |manager| async move {
        let entries = manager.list_directory(CONNECTION_ID, &path).await?;
        print_directory(&entries, as_json)
    })
    .await
}

async fn cmd_upload(args: UploadArgs) -> Result<()> {
    let local = args.local_path.clone();
    let remote = args.remote_path.clone();
    with_connection(&args.target, |manager| async move {
        let bytes = manager.upload_file(CONNECTION_ID, &local, &remote).await?;
        println!(
            "Uploaded {local} -> {remote} ({})",
            monitor::format_bytes_f64(bytes as f64)
        );
        Ok(())
    })
    .await
}

async fn cmd_download(args: DownloadArgs) -> Result<()> {
    let remote = args.remote_path.clone();
    let local = args.local_path.clone();
    with_connection(&args.target, |manager| async move {
        let bytes = manager
            .download_file(CONNECTION_ID, &remote, &local)
            .await?;
        println!(
            "Downloaded {remote} -> {local} ({})",
            monitor::format_bytes_f64(bytes as f64)
        );
        Ok(())
    })
    .await
}

async fn cmd_stats(target: TargetArgs) -> Result<()> {
    with_connection(&target, |manager| async move {
        // Two snapshots a moment apart so CPU% and network rates can be derived.
        let first_raw = manager.fetch_system_stats(CONNECTION_ID).await?;
        let first = monitor::parse_snapshot(&first_raw);
        let started = Instant::now();
        tokio::time::sleep(Duration::from_millis(800)).await;
        let second_raw = manager.fetch_system_stats(CONNECTION_ID).await?;
        let second = monitor::parse_snapshot(&second_raw);
        let elapsed = started.elapsed().as_secs_f64();

        let stats = monitor::SystemStats::from_samples(Some(&first), &second, elapsed);
        print_stats(&stats);
        Ok(())
    })
    .await
}

async fn cmd_mcp() -> Result<()> {
    let bridge = mcp::McpBridge::new();
    println!("Starting R-Shell MCP server on {}", mcp::MCP_ENDPOINT);
    println!("Press Ctrl-C to stop.");
    mcp::start_mcp_server(bridge).await
}

// ---------------------------------------------------------------------------
// Interactive PTY shell
// ---------------------------------------------------------------------------

async fn cmd_shell(target: TargetArgs) -> Result<()> {
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    let config = resolve_target(&target)?;
    let manager = Arc::new(NativeConnectionManager::new());
    manager
        .create_connection(CONNECTION_ID.to_string(), config)
        .await
        .context("failed to establish SSH connection")?;

    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    manager
        .start_pty_session(CONNECTION_ID, cols as u32, rows as u32)
        .await
        .context("failed to start PTY session")?;

    enable_raw_mode().context("failed to enable raw terminal mode")?;
    let result = run_pty_loop(manager.clone()).await;
    let _ = disable_raw_mode();
    let _ = manager.close_connection(CONNECTION_ID).await;

    // Restore a sane cursor position after raw-mode output.
    println!();
    result
}

/// Pump bytes between the local terminal and the remote PTY until either side
/// closes. Local stdin is read on a blocking thread; remote output is polled.
async fn run_pty_loop(manager: Arc<NativeConnectionManager>) -> Result<()> {
    let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);

    // Blocking stdin reader. Raw mode delivers bytes as they are typed.
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin();
        let mut buffer = [0u8; 1024];
        loop {
            match stdin.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    if stdin_tx.blocking_send(buffer[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut stdout = std::io::stdout();
    loop {
        tokio::select! {
            maybe_input = stdin_rx.recv() => {
                match maybe_input {
                    Some(data) => {
                        // Ctrl-] (0x1d) is a local escape to force-quit the shell.
                        if data.contains(&0x1d) {
                            break;
                        }
                        manager.write_pty_input(CONNECTION_ID, data).await?;
                    }
                    None => break,
                }
            }
            output = manager.read_pty_output(CONNECTION_ID) => {
                match output {
                    Ok(Some(data)) => {
                        stdout.write_all(&data)?;
                        stdout.flush()?;
                    }
                    Ok(None) => {
                        // Timed out with no output; loop again to check stdin.
                    }
                    Err(_) => break,
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Output formatting helpers
// ---------------------------------------------------------------------------

fn print_directory(entries: &[RemoteFileEntry], as_json: bool) -> Result<()> {
    if as_json {
        let rows: Vec<_> = entries
            .iter()
            .map(|entry| {
                serde_json::json!({
                    "name": entry.name,
                    "kind": entry.kind.label(),
                    "permissions": entry.permissions,
                    "size": entry.size,
                    "modified": entry.modified,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    for entry in entries {
        println!(
            "{:<4}  {:<11}  {:>10}  {:<16}  {}",
            entry.kind.label(),
            entry.permissions,
            entry.size,
            entry.modified,
            entry.name,
        );
    }
    Ok(())
}

fn print_stats(stats: &monitor::SystemStats) {
    println!(
        "OS:      {}",
        if stats.os.is_empty() {
            "unknown"
        } else {
            &stats.os
        }
    );
    println!("Uptime:  {}", monitor::format_uptime(stats.uptime_secs));
    println!(
        "CPU:     {:.1}%  ({} cores, load {:.2})",
        stats.cpu_percent, stats.cpu_cores, stats.load1
    );
    println!(
        "Memory:  {:.1}%  ({})",
        stats.mem_percent,
        monitor::format_kb_pair(stats.mem_used_kb, stats.mem_total_kb)
    );
    if stats.swap_total_kb > 0 {
        println!(
            "Swap:    {:.1}%  ({})",
            stats.swap_percent,
            monitor::format_kb_pair(stats.swap_used_kb, stats.swap_total_kb)
        );
    }
    println!(
        "Disk:    {:.1}%  ({})",
        stats.disk_percent,
        monitor::format_kb_pair(stats.disk_used_kb, stats.disk_total_kb)
    );
    println!(
        "Network: down {}  up {}",
        monitor::format_rate(stats.net_rx_per_sec),
        monitor::format_rate(stats.net_tx_per_sec)
    );
}

/// Read a secret from the controlling terminal without echoing it.
fn prompt_password(prompt: &str) -> Result<String> {
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    eprint!("{prompt}");
    std::io::stderr().flush().ok();

    enable_raw_mode().context("failed to read password")?;
    let mut password = String::new();
    let mut stdin = std::io::stdin();
    let mut byte = [0u8; 1];
    loop {
        if stdin.read(&mut byte).context("failed to read password")? == 0 {
            break;
        }
        match byte[0] {
            b'\r' | b'\n' => break,
            0x03 => {
                let _ = disable_raw_mode();
                bail!("aborted");
            }
            0x7f | 0x08 => {
                password.pop();
            }
            other => password.push(other as char),
        }
    }
    let _ = disable_raw_mode();
    eprintln!();
    Ok(password)
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        let mut result: String = value.chars().take(max.saturating_sub(1)).collect();
        result.push('…');
        result
    }
}
