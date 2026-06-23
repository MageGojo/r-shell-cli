//! GUI 命令块终端 API —— 见 docs/gui/11-命令块终端原型.md。
//!
//! 命令块每次跑一条命令并把完整输出 / 退出码 / 结束后的工作目录捕获成「一块」。
//! 执行 = `r_shell_core::blockexec` 的 POSIX sentinel 包装 + 解析:
//!
//! - **远程**([`block_run_remote`]):`ensure_session` 复用终端 / 文件 / 监控共享的
//!   活跃会话,再走 `NativeConnectionManager::execute_command`(SSH/ADB 透明分派),
//!   **不新增执行通道**。
//! - **本机**([`block_run_local`]):`core::blockexec::run_local`(`$SHELL -c`),
//!   让用户无需任何服务器即可在本机试用。
//!
//! 一次性「请求-响应」用 `Result<T, String>` 即可(非长流,无 frb unawaited 隐患;
//! 与 `sftp_delete/rename` 同范式)。凭据只在 Rust 侧使用,绝不回传 Dart。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use r_shell_core::blockexec::{self, BlockOutcome};

use super::{ensure_session, manager};

/// 某连接远端的 shell 种类(决定用哪套哨兵包装)。
#[derive(Clone, Copy, PartialEq)]
enum ShellKind {
    /// POSIX shell(Linux / macOS / Android),用 `wrap_posix`。
    Posix,
    /// Windows PowerShell,用 `wrap_powershell`。
    PowerShell,
}

/// 按 connection_id 缓存已探明的 shell 种类,避免每条命令都重复「先试 POSIX」。
fn shell_kinds() -> &'static Mutex<HashMap<String, ShellKind>> {
    static CACHE: OnceLock<Mutex<HashMap<String, ShellKind>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 用指定 shell 包装并在连接上跑一条命令,解析出结果(含是否命中哨兵)。
async fn run_wrapped(
    kind: ShellKind,
    connection_id: &str,
    command: &str,
    cwd: &str,
) -> Result<BlockOutcome, String> {
    let wrapped = match kind {
        ShellKind::Posix => blockexec::wrap_posix(cwd, command),
        ShellKind::PowerShell => blockexec::wrap_powershell(cwd, command),
    };
    let raw = manager()
        .execute_command(connection_id, &wrapped)
        .await
        .map_err(|e| e.to_string())?;
    Ok(blockexec::parse_outcome(&raw, cwd))
}

/// 一条命令块的执行结果(frb 友好的 DTO,与 core [`BlockOutcome`] 一一映射)。
pub struct BlockResultDto {
    /// 合并后的输出(stdout+stderr),已剥离哨兵尾巴。
    pub body: String,
    /// 用户命令退出码;未命中哨兵(非 POSIX 远端)时为 `-1`。
    pub exit_code: i32,
    /// 命令结束后的工作目录(供下一块沿用)。
    pub cwd: String,
    /// 退出码 / cwd 是否可信(是否解析到哨兵)。
    pub had_marker: bool,
}

impl From<BlockOutcome> for BlockResultDto {
    fn from(o: BlockOutcome) -> Self {
        Self {
            body: o.body,
            exit_code: o.exit_code,
            cwd: o.cwd,
            had_marker: o.had_marker,
        }
    }
}

/// 在某已保存连接(SSH / ADB)上执行一条命令块。
///
/// `cwd` 为上一块结束时的工作目录(首次传空串,由远端默认目录回填)。
///
/// **跨 shell**:首次对某连接执行时「先试 POSIX(Linux/macOS/Android),未命中哨兵
/// 再试 PowerShell(Windows)」,把命中的 shell 种类缓存下来,后续直接用——这样
/// Windows 主机的命令块也能拿到退出码与跨块 `cd`(此前 POSIX 包装在 PowerShell 上
/// 整条报错 `Some(1)`)。两种都没命中哨兵时优雅降级(原样输出)。
pub async fn block_run_remote(
    connection_id: String,
    command: String,
    cwd: String,
) -> Result<BlockResultDto, String> {
    ensure_session(&connection_id).await?;

    // 已探明该连接的 shell 种类 → 直接用,不再多跑一次探测。
    let cached = shell_kinds().lock().unwrap().get(&connection_id).copied();
    if let Some(kind) = cached {
        return run_wrapped(kind, &connection_id, &command, &cwd)
            .await
            .map(Into::into);
    }

    // 首次:先试 POSIX,命中哨兵即认定并缓存。
    let posix = run_wrapped(ShellKind::Posix, &connection_id, &command, &cwd).await;
    if let Ok(ref oc) = posix {
        if oc.had_marker {
            shell_kinds()
                .lock()
                .unwrap()
                .insert(connection_id.clone(), ShellKind::Posix);
            return Ok(oc.clone().into());
        }
    }

    // 再试 PowerShell(Windows)。
    let ps = run_wrapped(ShellKind::PowerShell, &connection_id, &command, &cwd).await;
    if let Ok(oc) = ps {
        if oc.had_marker {
            shell_kinds()
                .lock()
                .unwrap()
                .insert(connection_id.clone(), ShellKind::PowerShell);
        }
        // 命中→可信结果;未命中→PowerShell 也只能降级原样输出。
        return Ok(oc.into());
    }

    // 两种都失败(连命令通道都没跑通):返回 POSIX 那次的错误或其降级输出。
    posix.map(Into::into)
}

/// 在本机(运行 GUI 的电脑)执行一条命令块。
pub async fn block_run_local(command: String, cwd: String) -> Result<BlockResultDto, String> {
    blockexec::run_local(&cwd, &command)
        .await
        .map(Into::into)
        .map_err(|e| e.to_string())
}
