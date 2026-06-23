//! 验证命令块在 Windows(PowerShell)上的执行:对比 POSIX 包装(应失败)与
//! PowerShell 包装(修复)。走与 GUI bridge 同一套 `NativeConnectionManager`。
//!
//! 用法:`cargo run -p r-shell-core --example block_win_smoke [connection_id] [command]`
//! 默认连接 = Win-Dev(`ssh-1782117733340`),默认命令 = `ls`。

use r_shell_core::blockexec;
use r_shell_core::connections;
use r_shell_core::native_backend::NativeConnectionManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let id = args.next().unwrap_or_else(|| "ssh-1782117733340".to_string());
    let command = args.next().unwrap_or_else(|| "ls".to_string());

    let conn = connections::find(&id)?;
    println!("连接: {} ({}@{}:{})", conn.name, conn.username, conn.host, conn.port);

    let config = connections::build_ssh_config(&conn, false)?;
    let mgr = NativeConnectionManager::new();
    mgr.create_connection(id.clone(), config).await?;
    println!("SSH 已连接,命令 = {command:?}\n");

    println!("== ① POSIX 包装(命令块旧逻辑,PowerShell 上应整条失败)==");
    let wp = blockexec::wrap_posix("", &command);
    match mgr.execute_command(&id, &wp).await {
        Ok(raw) => {
            let oc = blockexec::parse_outcome(&raw, "");
            let body: String = oc.body.chars().take(300).collect();
            println!(
                "  执行返回 Ok;had_marker={} exit={} body(前300)={:?}",
                oc.had_marker, oc.exit_code, body
            );
        }
        Err(e) => println!("  执行返回 Err: {e}  ← 这正是你看到的「Command failed with code: Some(1)」"),
    }

    println!("\n== ② PowerShell 包装(本次修复)==");
    let wps = blockexec::wrap_powershell("", &command);
    match mgr.execute_command(&id, &wps).await {
        Ok(raw) => {
            let oc = blockexec::parse_outcome(&raw, "");
            let body: String = oc.body.trim_end().chars().take(1200).collect();
            println!(
                "  had_marker={} exit_code={} cwd={}",
                oc.had_marker, oc.exit_code, oc.cwd
            );
            println!("  ---- 输出(前1200字符) ----\n{}", body);
        }
        Err(e) => println!("  执行返回 Err: {e}"),
    }

    Ok(())
}
