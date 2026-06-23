//! GUI MCP 面板 API —— 在桌面进程内启停 r-shell-core 的本地 MCP 服务。
//!
//! core 的 [`start_mcp_server`](r_shell_core::mcp::start_mcp_server) 是一个进程内
//! axum Streamable-HTTP 服务(绑定 `127.0.0.1:9123/mcp`),原本由 CLI `r-shell mcp`
//! 阻塞运行。本模块把它 `tokio::spawn` 到 frb 的运行时,用一个 **bridge 私有的静态
//! 状态**(`JoinHandle` + 起始时刻)记录运行情况:
//!
//! - 启动:若未在运行则 spawn;spawn 后短暂等待,若任务秒退说明绑定失败(端口被占),报错。
//! - 停止:`JoinHandle::abort()` 释放监听端口(drop 掉 axum serve 的 future)。
//! - 运行状态 / 运行时长由本模块自管,**不依赖 core 的 `MCP_SERVER_RUNNING`**(abort 不会回写它)。
//!
//! 工具目录 [`mcp_tools`] 是镜像 core `RShellMcpServer` 上 `#[tool]` 的静态清单(name +
//! 中文说明 + 分类),供面板展示;不依赖运行时反射。

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use r_shell_core::mcp;
use tokio::task::JoinHandle;

/// bridge 私有的 MCP 运行状态。
struct McpState {
    handle: Option<JoinHandle<()>>,
    started_at: Option<Instant>,
}

fn state() -> &'static Mutex<McpState> {
    static STATE: OnceLock<Mutex<McpState>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(McpState {
            handle: None,
            started_at: None,
        })
    })
}

/// 该 handle 当前是否仍在运行(spawn 的任务未结束)。
fn is_alive(handle: &Option<JoinHandle<()>>) -> bool {
    handle.as_ref().map(|h| !h.is_finished()).unwrap_or(false)
}

/// MCP 服务状态快照(面板状态卡用)。
pub struct McpStatusDto {
    pub running: bool,
    pub host: String,
    pub port: u16,
    pub endpoint: String,
    /// 运行时长(秒);未运行为 0。
    pub uptime_secs: u64,
    /// 暴露的工具数量。
    pub tool_count: u32,
}

/// 一个 MCP 工具的展示信息。
pub struct McpToolDto {
    pub name: String,
    pub summary: String,
    /// 分类(决定面板图标):`session` / `exec` / `file` / `dir` / `connection`。
    pub category: String,
}

/// MCP 端点常量(`http://127.0.0.1:9123/mcp`)。
#[flutter_rust_bridge::frb(sync)]
pub fn mcp_endpoint() -> String {
    mcp::MCP_ENDPOINT.to_string()
}

/// MCP 监听端口(9123)。
#[flutter_rust_bridge::frb(sync)]
pub fn mcp_port() -> u16 {
    mcp::MCP_PORT
}

/// 读取当前 MCP 服务状态(同步,面板可随时轮询)。
#[flutter_rust_bridge::frb(sync)]
pub fn mcp_status() -> McpStatusDto {
    let st = state().lock().expect("mcp state lock");
    let running = is_alive(&st.handle);
    let uptime_secs = if running {
        st.started_at.map(|t| t.elapsed().as_secs()).unwrap_or(0)
    } else {
        0
    };
    McpStatusDto {
        running,
        host: "127.0.0.1".to_string(),
        port: mcp::MCP_PORT,
        endpoint: mcp::MCP_ENDPOINT.to_string(),
        uptime_secs,
        tool_count: mcp_tools().len() as u32,
    }
}

/// 启动本机 MCP 服务(幂等:已运行则直接返回成功)。
///
/// spawn 后等待 250ms;若任务已结束说明 `start_mcp_server` 绑定失败(多为端口被占),返回错误。
pub async fn mcp_start() -> Result<(), String> {
    {
        let st = state().lock().map_err(|_| "mcp state lock poisoned".to_string())?;
        if is_alive(&st.handle) {
            return Ok(());
        }
    }

    let bridge = mcp::McpBridge::new();
    let handle = tokio::spawn(async move {
        if let Err(error) = mcp::start_mcp_server(bridge).await {
            eprintln!("[mcp] server exited: {error:#}");
        }
    });

    // 给端口绑定留点时间;若任务很快结束,基本是绑定失败(端口被占用)。
    tokio::time::sleep(Duration::from_millis(250)).await;
    if handle.is_finished() {
        return Err(format!(
            "MCP 服务启动失败:端口 {} 可能已被占用",
            mcp::MCP_PORT
        ));
    }

    let mut st = state().lock().map_err(|_| "mcp state lock poisoned".to_string())?;
    st.handle = Some(handle);
    st.started_at = Some(Instant::now());
    Ok(())
}

/// 停止本机 MCP 服务(幂等:未运行则无操作)。abort spawn 的任务以释放监听端口。
pub async fn mcp_stop() -> Result<(), String> {
    let handle = {
        let mut st = state().lock().map_err(|_| "mcp state lock poisoned".to_string())?;
        st.started_at = None;
        st.handle.take()
    };
    if let Some(handle) = handle {
        handle.abort();
    }
    Ok(())
}

/// 列出 MCP 暴露的工具(静态清单,镜像 core `RShellMcpServer`)。
#[flutter_rust_bridge::frb(sync)]
pub fn mcp_tools() -> Vec<McpToolDto> {
    fn tool(name: &str, summary: &str, category: &str) -> McpToolDto {
        McpToolDto {
            name: name.to_string(),
            summary: summary.to_string(),
            category: category.to_string(),
        }
    }
    vec![
        tool("ssh_session_open", "打开 / 复用持久 SSH 会话", "session"),
        tool("ssh_exec", "在已开会话上执行命令", "exec"),
        tool("ssh_read_file", "读取远程文件", "file"),
        tool("ssh_write_file", "写入 / 覆盖远程文件", "file"),
        tool("ssh_list_dir", "列出远程目录", "dir"),
        tool("ssh_sessions_list", "列出当前活跃会话", "session"),
        tool("ssh_session_close", "关闭持久 SSH 会话", "session"),
        tool("r_shell_ssh_connections_list", "列出已保存连接(脱敏)", "connection"),
        tool("r_shell_ssh_connection_create", "创建并保存 SSH 连接", "connection"),
        tool("r_shell_ssh_connection_update", "更新已保存连接", "connection"),
        tool("r_shell_ssh_connection_delete", "删除已保存连接", "connection"),
        tool("r_shell_ssh_tabs_list", "列出已打开的终端标签", "session"),
    ]
}
