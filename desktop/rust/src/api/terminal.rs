//! GUI 终端 / 会话 API。
//!
//! 在一个进程级的 [`NativeConnectionManager`] 上提供:会话开关 + 多标签 PTY
//! (每个终端标签一个 `pty_id`)。PTY 输出字节经 [`StreamSink`] 单向推给 Dart,
//! 由 `xterm.dart` 渲染;键盘输入 / resize / 关闭通过 `pty_id` 回调。
//!
//! 凭据(密码 / 私钥 / passphrase)从本地 `workspace.json` 读取,**仅在 Rust 侧
//! 用于建立连接,绝不回传 Dart**。

use super::manager;
use crate::frb_generated::StreamSink;

/// 确保某个已保存连接有一个活跃 SSH 会话(幂等:已连接则复用,供同主机多标签共用)。
pub async fn session_open(connection_id: String) -> Result<(), String> {
    super::ensure_session(&connection_id).await
}

/// 关闭某连接的 SSH 会话(同时清理其名下所有 PTY)。
pub async fn session_close(connection_id: String) -> Result<(), String> {
    manager()
        .close_connection(&connection_id)
        .await
        .map_err(|e| e.to_string())
}

/// 该连接当前是否有活跃会话。
pub async fn session_is_open(connection_id: String) -> bool {
    manager().has_connection(&connection_id).await
}

/// 在已打开的会话上新开一个 PTY,返回唯一 `pty_id`(用于后续 write / resize / close)。
pub async fn pty_open(connection_id: String, cols: u16, rows: u16) -> Result<String, String> {
    manager()
        .open_pty(&connection_id, cols as u32, rows as u32)
        .await
        .map_err(|e| e.to_string())
}

/// 订阅某 PTY 的输出字节流(Dart 侧得到 `Stream<Uint8List>`)。PTY 关闭即流自然结束。
pub async fn pty_subscribe(pty_id: String, sink: StreamSink<Vec<u8>>) -> Result<(), String> {
    let mgr = manager();
    loop {
        match mgr.read_pty(&pty_id).await {
            Ok(Some(bytes)) => {
                if sink.add(bytes).is_err() {
                    break; // Dart 端已取消订阅
                }
            }
            Ok(None) => {} // 150ms 无数据,继续轮询(顺带感知关闭)
            Err(_) => break, // PTY 已关闭 / 不存在
        }
    }
    Ok(())
}

/// 向 PTY 写入输入字节(键盘)。
pub async fn pty_write(pty_id: String, data: Vec<u8>) -> Result<(), String> {
    manager()
        .write_pty(&pty_id, data)
        .await
        .map_err(|e| e.to_string())
}

/// 调整 PTY 窗口尺寸(列 / 行)。
pub async fn pty_resize(pty_id: String, cols: u16, rows: u16) -> Result<(), String> {
    manager()
        .resize_pty_by_id(&pty_id, cols as u32, rows as u32)
        .await
        .map_err(|e| e.to_string())
}

/// 关闭 PTY(终端标签关闭时调用)。
pub async fn pty_close(pty_id: String) -> Result<(), String> {
    manager()
        .close_pty(&pty_id)
        .await
        .map_err(|e| e.to_string())
}
