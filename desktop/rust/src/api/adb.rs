//! GUI ADB 设备配对 / 连接 API —— 复用 r-shell-core 的 ADB 能力。
//!
//! 用于安卓 11+「无线调试」**首次连接**:先用配对码 [`adb_pair`] 让设备信任本机
//! 密钥,再 [`adb_connect`] 连上连接端口(通常 5555)。两者都直接调用本机 `adb`
//! 子进程,无需先在连接树里保存连接;成功后即可正常添加 / 使用 ADB 连接。

/// 用配对码与设备配对(`adb pair <host:port> <code>`)。
///
/// `host_port` 是手机「无线调试 → 使用配对码配对设备」里显示的**配对地址**
/// (端口随机,不是 5555);`code` 是同一界面的 6 位配对码。返回 adb 的成功提示。
pub async fn adb_pair(host_port: String, code: String) -> Result<String, String> {
    r_shell_core::adb::pair(host_port.trim(), code.trim())
        .await
        .map_err(|e| e.to_string())
}

/// 连接网络 adb 设备(`adb connect <host:port>`,通常端口 5555)。返回 adb 的提示。
pub async fn adb_connect(host_port: String) -> Result<String, String> {
    r_shell_core::adb::connect_device(host_port.trim())
        .await
        .map_err(|e| e.to_string())
}
