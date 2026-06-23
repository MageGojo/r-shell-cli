//! GUI 连接管理 API —— 复用 r-shell-core 的 connections/storage/model。
//!
//! 读取走脱敏的 [`ConnectionDto`]（绝不回传明文密码 / 私钥内容 / passphrase）；
//! 写入（增 / 改 / 删）全部委托给 `r_shell_core::connections`，与 CLI 同一套
//! 校验和持久化逻辑（单一事实来源）。

use flutter_rust_bridge::frb;
use r_shell_core::connections;
use r_shell_core::model::SavedConnection;
use r_shell_core::storage;

/// 脱敏的连接信息（供 GUI 展示）。
///
/// **绝不包含明文密码 / 私钥内容 / passphrase** —— 只用布尔标记是否已配置，
/// 与 core 的 `SavedConnection::sanitized()` 策略保持一致。
pub struct ConnectionDto {
    pub id: String,
    /// "SSH" | "ADB"
    pub protocol: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub folder: String,
    pub description: String,
    /// "password" | "publickey"
    pub auth_method: String,
    /// "Disconnected" | "Connecting" | "Connected"
    pub status: String,
    pub tags: Vec<String>,
    pub has_password: bool,
    pub has_private_key_path: bool,
    pub has_passphrase: bool,
}

impl ConnectionDto {
    fn from_saved(c: &SavedConnection) -> Self {
        Self {
            id: c.id.clone(),
            protocol: c.protocol.clone(),
            name: c.name.clone(),
            host: c.host.clone(),
            port: c.port,
            username: c.username.clone(),
            folder: c.folder.clone(),
            description: c.description.clone(),
            auth_method: SavedConnection::normalize_auth_method(&c.auth_method).to_string(),
            status: c.status.label().to_string(),
            tags: c.tags.clone(),
            has_password: c.password.as_deref().is_some_and(|v| !v.is_empty()),
            has_private_key_path: c.private_key_path.as_deref().is_some_and(|v| !v.is_empty()),
            has_passphrase: c.passphrase.as_deref().is_some_and(|v| !v.is_empty()),
        }
    }
}

/// 连接编辑表单的输入（创建 / 更新共用）。
///
/// 三个密钥字段的语义（与 `r_shell_core::connections::ConnectionPatch` 对齐）：
/// - `None`   —— 更新时保持原值不变；创建时表示「未设置」。
/// - `Some("")` —— 清除已保存的值。
/// - `Some(v)`  —— 设置为 `v`。
pub struct ConnectionInput {
    /// "SSH"（默认）| "ADB"。
    pub protocol: String,
    pub name: String,
    pub host: String,
    pub username: String,
    pub port: u16,
    /// "password" | "publickey"（其它值会被归一化为 "password"）。
    pub auth_method: String,
    pub folder: String,
    pub description: String,
    pub tags: Vec<String>,
    pub password: Option<String>,
    pub private_key_path: Option<String>,
    pub passphrase: Option<String>,
}

/// 读取本地 `workspace.json`，返回脱敏后的连接列表（分组信息在 `folder` 字段）。
#[frb(sync)]
pub fn list_connections() -> Vec<ConnectionDto> {
    storage::load_workspace()
        .connections
        .iter()
        .map(ConnectionDto::from_saved)
        .collect()
}

/// 新建连接并写入 `workspace.json`，返回脱敏后的结果（含新生成的 id）。
#[frb(sync)]
pub fn create_connection(input: ConnectionInput) -> Result<ConnectionDto, String> {
    let connection = connections::add(connections::NewConnection {
        protocol: input.protocol,
        name: input.name,
        host: input.host,
        username: input.username,
        port: input.port,
        auth_method: input.auth_method,
        folder: input.folder,
        description: input.description,
        tags: input.tags,
        password: input.password,
        private_key_path: input.private_key_path,
        passphrase: input.passphrase,
    })
    .map_err(|e| e.to_string())?;
    Ok(ConnectionDto::from_saved(&connection))
}

/// 按 id 更新连接（提交整张表单），返回脱敏后的最新值。
#[frb(sync)]
pub fn update_connection(id: String, input: ConnectionInput) -> Result<ConnectionDto, String> {
    let connection = connections::update(
        &id,
        connections::ConnectionPatch {
            protocol: Some(input.protocol),
            name: Some(input.name),
            host: Some(input.host),
            username: Some(input.username),
            port: Some(input.port),
            auth_method: Some(input.auth_method),
            folder: Some(input.folder),
            description: Some(input.description),
            tags: Some(input.tags),
            password: input.password,
            private_key_path: input.private_key_path,
            passphrase: input.passphrase,
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(ConnectionDto::from_saved(&connection))
}

/// 按 id 删除连接（同时清理其终端标签与 active 指针）。
#[frb(sync)]
pub fn delete_connection(id: String) -> Result<(), String> {
    connections::remove(&id).map_err(|e| e.to_string())?;
    Ok(())
}
