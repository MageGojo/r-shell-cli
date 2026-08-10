pub mod adb;
pub mod blockterm;
pub mod connections;
pub mod mcp;
pub mod monitor;
pub mod sftp;
pub mod simple;
pub mod terminal;

use std::sync::OnceLock;

use r_shell_core::native_backend::NativeConnectionManager;

/// Process-wide connection manager shared by every GUI session, PTY, and SFTP
/// transfer. Terminal tabs and the file manager **must** use the same instance
/// so an SFTP browse/transfer reuses the live SSH session a terminal opened.
/// Driven by flutter_rust_bridge's tokio runtime.
pub(crate) fn manager() -> &'static NativeConnectionManager {
    static MANAGER: OnceLock<NativeConnectionManager> = OnceLock::new();
    MANAGER.get_or_init(NativeConnectionManager::new)
}

/// Ensure a saved connection has a live SSH session (idempotent: reuse if
/// already connected). Credentials are read from the local `workspace.json` and
/// used only here on the Rust side — never returned to Dart.
pub(crate) async fn ensure_session(connection_id: &str) -> Result<(), String> {
    let mgr = manager();
    if mgr.has_connection(connection_id).await {
        return Ok(());
    }
    let connection = r_shell_core::connections::find(connection_id).map_err(|e| e.to_string())?;

    // ADB devices are reached by host:port over the local `adb` daemon; SSH uses
    // stored credentials. Dispatch on the saved protocol.
    if connection.protocol.eq_ignore_ascii_case("ADB") {
        let serial = format!("{}:{}", connection.host, connection.port);
        return mgr
            .create_adb_connection(connection_id.to_string(), serial)
            .await
            .map_err(|e| e.to_string());
    }

    let config = r_shell_core::connections::build_ssh_config(&connection, false)
        .map_err(|e| e.to_string())?;
    let profile = r_shell_core::ios_ssh::ExecProfile::from_connection(&connection);
    mgr.create_connection_with_profile(connection_id.to_string(), config, Some(profile))
        .await
        .map_err(|e| e.to_string())
}
