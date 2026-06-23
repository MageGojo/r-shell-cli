//! 后端无关的交互式 PTY 会话句柄。
//!
//! [`PtySession`] 把「一个活跃的交互式 shell」抽象成四个通道,不绑定任何具体
//! 传输层:SSH(russh channel)与 ADB(`adb shell` 子进程)都构造同一个结构,于是
//! [`crate::native_backend::NativeConnectionManager`] 的多标签终端逻辑对两种后端
//! 完全透明。
//!
//! - `input_tx`  —— 键盘输入字节(前端 → shell)。
//! - `output_rx` —— shell 输出字节(shell → 前端),`Arc<Mutex<..>>` 以便多处轮询。
//! - `resize_tx` —— 窗口尺寸变更 `(cols, rows)`(ADB 后端目前忽略)。
//! - `cancel`    —— 撤销令牌;会话拆除时触发,读取任务据此尽快停止。

use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;

/// 一个活跃交互式 shell 的双向通道句柄(SSH / ADB 通用)。
pub struct PtySession {
    /// 键盘输入字节(前端 → shell)。
    pub input_tx: mpsc::Sender<Vec<u8>>,
    /// shell 输出字节(shell → 前端)。
    pub output_rx: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
    /// 窗口尺寸变更请求 `(cols, rows)`。
    pub resize_tx: mpsc::Sender<(u32, u32)>,
    /// 会话拆除时触发的撤销令牌。
    pub cancel: CancellationToken,
}
