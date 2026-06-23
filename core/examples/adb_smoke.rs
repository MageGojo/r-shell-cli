//! 真机冒烟:用 `NativeConnectionManager` 的公开 API(GUI bridge 调的同一套)对一台
//! 已授权的 ADB 设备跑一遍 终端 / 文件 / 监控,验证 Stage 8 的 ADB 通道在真机可用。
//!
//! 用法:`cargo run -p r-shell-core --example adb_smoke -- <host:port>`
//! 例:  `cargo run -p r-shell-core --example adb_smoke -- 192.168.0.101:45593`
//!
//! 设备需已授权(`adb devices` 显示 `device`)。本 example 不写仓库代码、可随时删。

use std::time::{Duration, Instant};

use r_shell_core::monitor::{self, SystemStats};
use r_shell_core::native_backend::NativeConnectionManager;

const CID: &str = "adb-smoke";

#[tokio::main]
async fn main() {
    let serial = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.101:45593".to_string());

    println!("== R-Shell ADB 真机冒烟 ==  serial = {serial}\n");

    let mgr = NativeConnectionManager::new();

    // ── 连接(走 AdbClient::connect:adb connect + 校验 device 状态 + harden) ──
    match mgr.create_adb_connection(CID.to_string(), serial.clone()).await {
        Ok(()) => println!("[连接] OK —— 设备已授权,已执行强制开发者模式(harden)"),
        Err(e) => {
            eprintln!("[连接] 失败: {e:#}");
            std::process::exit(1);
        }
    }

    // ── 命令执行(execute_command,监控/采集也走它) ──
    section("命令执行 execute_command");
    run_cmd(&mgr, "getprop ro.product.model").await;
    run_cmd(&mgr, "getprop ro.build.version.release").await;
    run_cmd(&mgr, "id").await;
    run_cmd(&mgr, "echo 管道测试 | tr a-z A-Z; whoami").await; // 验证 sh -c 解析管道/分号

    // ── 文件:列目录(sftp_list = ADB 实际路径:ls -la + parse_android_ls) ──
    section("文件 sftp_list /sdcard");
    match mgr.sftp_list(CID, "/sdcard").await {
        Ok(entries) => {
            println!("  共 {} 项(前 12):", entries.len());
            for e in entries.iter().take(12) {
                let kind = if e.is_dir {
                    "DIR "
                } else if e.is_symlink {
                    "LNK "
                } else {
                    "FILE"
                };
                println!("    {kind} {:>10}  {}", e.size, e.name);
            }
        }
        Err(e) => eprintln!("  sftp_list 失败: {e:#}"),
    }

    // realpath
    section("文件 sftp_realpath");
    match mgr.sftp_realpath(CID, ".").await {
        Ok(p) => println!("  realpath(\".\") = {p}"),
        Err(e) => eprintln!("  realpath 失败: {e:#}"),
    }

    // ── 文件:写内存→读回(write_file_from_bytes / read_file_to_memory) ──
    section("文件 写入→读回(内存)");
    let remote = "/sdcard/rshell_adb_smoke.txt";
    let payload = format!(
        "R-Shell ADB smoke @ {}\n中文行 OK\n",
        chrono_like_now()
    );
    match mgr
        .write_file_from_bytes(CID, remote, payload.as_bytes())
        .await
    {
        Ok(n) => println!("  写入 {remote}: {n} 字节"),
        Err(e) => eprintln!("  写入失败: {e:#}"),
    }
    match mgr.read_file_to_memory(CID, remote).await {
        Ok(bytes) => {
            let got = String::from_utf8_lossy(&bytes);
            let ok = got == payload;
            println!("  读回 {} 字节, 与写入一致: {}", bytes.len(), ok);
            if !ok {
                println!("    读回内容:\n{got}");
            }
        }
        Err(e) => eprintln!("  读回失败: {e:#}"),
    }

    // ── 文件:上传/下载 往返(upload_file / download_file) ──
    section("文件 上传→下载 往返");
    let tmp_local_up = std::env::temp_dir().join("rshell_adb_up.bin");
    let tmp_local_down = std::env::temp_dir().join("rshell_adb_down.bin");
    let blob: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();
    if let Err(e) = std::fs::write(&tmp_local_up, &blob) {
        eprintln!("  本地写测试文件失败: {e:#}");
    } else {
        let remote_bin = "/sdcard/rshell_adb_smoke.bin";
        match mgr
            .upload_file(CID, tmp_local_up.to_str().unwrap(), remote_bin)
            .await
        {
            Ok(n) => println!("  upload {} 字节 → {remote_bin}", n),
            Err(e) => eprintln!("  upload 失败: {e:#}"),
        }
        let _ = std::fs::remove_file(&tmp_local_down);
        match mgr
            .download_file(CID, remote_bin, tmp_local_down.to_str().unwrap())
            .await
        {
            Ok(n) => {
                let back = std::fs::read(&tmp_local_down).unwrap_or_default();
                println!(
                    "  download {} 字节, 与上传一致: {}",
                    n,
                    back == blob
                );
            }
            Err(e) => eprintln!("  download 失败: {e:#}"),
        }
        // 清理设备上的测试文件
        run_cmd(&mgr, "rm -f /sdcard/rshell_adb_smoke.bin /sdcard/rshell_adb_smoke.txt").await;
    }

    // ── 监控:fetch_system_snapshot ×2 → SystemStats(CPU%/网速需两帧) ──
    section("监控 fetch_system_snapshot");
    match (
        mgr.fetch_system_snapshot(CID).await,
        {
            let started = Instant::now();
            tokio::time::sleep(Duration::from_millis(900)).await;
            let second = mgr.fetch_system_snapshot(CID).await;
            second.map(|s| (s, started.elapsed().as_secs_f64()))
        },
    ) {
        (Ok(first), Ok((second, elapsed))) => {
            let stats = SystemStats::from_samples(Some(&first), &second, elapsed);
            print_stats(&stats);
        }
        (a, b) => {
            if let Err(e) = a {
                eprintln!("  第一帧失败: {e:#}");
            }
            if let Err(e) = b {
                eprintln!("  第二帧失败: {e:#}");
            }
        }
    }

    // ── 终端:open_pty → 发命令 → 读输出(交互式 PTY) ──
    section("终端 open_pty(交互式)");
    match mgr.open_pty(CID, 80, 24).await {
        Ok(pty_id) => {
            println!("  PTY 已开: {pty_id}");
            // 等提示符,喂一条命令
            tokio::time::sleep(Duration::from_millis(400)).await;
            let _ = mgr
                .write_pty(&pty_id, b"echo PTY_MARK_$((6*7))\n".to_vec())
                .await;
            // 收 ~2s 输出
            let mut acc = String::new();
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                match mgr.read_pty(&pty_id).await {
                    Ok(Some(data)) => acc.push_str(&String::from_utf8_lossy(&data)),
                    Ok(None) => {}
                    Err(_) => break,
                }
                if acc.contains("PTY_MARK_42") {
                    break;
                }
            }
            let hit = acc.contains("PTY_MARK_42");
            println!("  发送 `echo PTY_MARK_$((6*7))`,捕获到 `PTY_MARK_42`: {hit}");
            let preview: String = acc.replace('\r', "").lines().take(6).collect::<Vec<_>>().join(" | ");
            println!("  PTY 输出预览: {preview}");
            let _ = mgr.close_pty(&pty_id).await;
        }
        Err(e) => eprintln!("  open_pty 失败: {e:#}"),
    }

    // ── 收尾 ──
    let _ = mgr.close_connection(CID).await;
    let _ = std::fs::remove_file(&tmp_local_up);
    let _ = std::fs::remove_file(&tmp_local_down);
    section("完成");
    println!("ADB 真机冒烟跑完。");
}

async fn run_cmd(mgr: &NativeConnectionManager, cmd: &str) {
    match mgr.execute_command(CID, cmd).await {
        Ok(out) => println!("  $ {cmd}\n    {}", out.trim().replace('\n', "\n    ")),
        Err(e) => eprintln!("  $ {cmd}\n    [失败] {e:#}"),
    }
}

fn section(title: &str) {
    println!("\n── {title} ──");
}

fn print_stats(s: &SystemStats) {
    println!(
        "  OS:      {}",
        if s.os.is_empty() { "unknown" } else { &s.os }
    );
    println!("  Uptime:  {}", monitor::format_uptime(s.uptime_secs));
    println!(
        "  CPU:     {:.1}%  ({} cores, load {:.2})",
        s.cpu_percent, s.cpu_cores, s.load1
    );
    println!(
        "  Memory:  {:.1}%  ({})",
        s.mem_percent,
        monitor::format_kb_pair(s.mem_used_kb, s.mem_total_kb)
    );
    println!(
        "  Disk:    {:.1}%  ({})  [主盘]",
        s.disk_percent,
        monitor::format_kb_pair(s.disk_used_kb, s.disk_total_kb)
    );
    for d in &s.disks {
        println!(
            "    └ {:<18} {:>5.1}%  ({})",
            d.mount,
            d.percent(),
            monitor::format_kb_pair(d.used_kb, d.total_kb)
        );
    }
    println!(
        "  Network: down {}  up {}",
        monitor::format_rate(s.net_rx_per_sec),
        monitor::format_rate(s.net_tx_per_sec)
    );
}

/// 极简时间戳(避免引第三方 time 依赖):用 UNIX 秒。
fn chrono_like_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
