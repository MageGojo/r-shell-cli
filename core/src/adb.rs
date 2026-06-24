//! ADB 后端 —— 通过本地 `adb` 可执行文件管理安卓设备的 shell 与文件。
//!
//! 与走 russh 长连接的 [`crate::ssh::SshClient`] 不同,ADB 的会话状态由本机的
//! `adb` daemon(`adb connect host:port` 后保活)维护;本类型几乎是无状态的——每个
//! 操作都 spawn 一个 `adb` 子进程:
//!
//! - 命令执行(含监控采集):`adb -s <serial> shell <cmd>`(安卓是 Linux,`/proc`+`df`
//!   等监控命令可直接复用)。
//! - 交互式终端:`adb -s <serial> shell -t -t`,接管子进程 stdin/stdout 构造一个
//!   后端无关的 [`PtySession`](crate::pty::PtySession)。
//! - 文件:`adb push` / `adb pull`(收发)、`adb exec-out cat`(读内存)、`ls -la`(列目录)。
//!
//! `serial` 即 `host:port`(如 `192.168.0.101:5555`)。
//!
//! ## 强制开启开发者模式(harden)
//! 连接成功后 best-effort 执行 `settings put global development_settings_enabled 1`
//! 与 `adb_enabled 1`(adb shell 的 uid=2000 默认持有 WRITE_SECURE_SETTINGS,无需
//! root),用于对抗「开发者模式 / USB 调试容易被系统关掉」。失败不阻断连接。

use std::process::Stdio;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;

use crate::adb_bin::adb_program;
use crate::pty::PtySession;
use crate::ssh::SftpEntry;

/// 构造一个指向「已解析的 adb 可执行文件」的命令(内嵌优先,见 [`crate::adb_bin`])。
///
/// 全模块统一用它替代裸名调用 adb,使「生产环境找不到 adb」一处修复、全量生效。
fn adb_cmd() -> Command {
    Command::new(adb_program())
}

/// 一个安卓设备的 ADB 客户端(`serial = host:port`)。
pub struct AdbClient {
    serial: String,
    connected: bool,
}

impl AdbClient {
    pub fn new(serial: impl Into<String>) -> Self {
        Self {
            serial: serial.into(),
            connected: false,
        }
    }

    /// 确保 `adb` daemon 已连上设备且处于 `device`(已授权)状态,随后 best-effort
    /// 强制开启开发者模式。设备处于 `unauthorized` / `offline` 时返回清晰错误。
    pub async fn connect(&mut self) -> Result<()> {
        // 让本机 adb daemon 主动连一次(已连则幂等;网络 adb 必需)。
        let _ = adb_cmd()
            .args(["connect", &self.serial])
            .output()
            .await;

        let state = self.device_state().await?;
        if state != "device" {
            return Err(match state.as_str() {
                "unauthorized" => anyhow!(
                    "ADB 设备 {} 未授权:请在手机上勾选「一律允许这台计算机调试」并点允许",
                    self.serial
                ),
                "offline" => anyhow!(
                    "ADB 设备 {} 离线:请检查 USB 调试 / 网络 adb(5555)是否开启",
                    self.serial
                ),
                "" => anyhow!(
                    "ADB 设备 {} 未连接:确认手机已开启网络 adb 且与本机同一局域网",
                    self.serial
                ),
                other => anyhow!("ADB 设备 {} 状态异常: {}", self.serial, other),
            });
        }

        self.connected = true;
        // 强制开启开发者模式(best-effort,失败不阻断)。
        self.harden().await;
        Ok(())
    }

    /// 查询 `adb -s <serial> get-state`,返回去空白后的状态字符串
    /// (`device` / `unauthorized` / `offline` / `""`)。
    async fn device_state(&self) -> Result<String> {
        let out = adb_cmd()
            .args(["-s", &self.serial, "get-state"])
            .output()
            .await
            .context("无法执行 adb get-state(adb 是否已安装并在 PATH 中?)")?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            // get-state 失败时,stderr 常含 "device unauthorized" / "device offline"。
            let err = String::from_utf8_lossy(&out.stderr).to_lowercase();
            if err.contains("unauthorized") {
                Ok("unauthorized".to_string())
            } else if err.contains("offline") {
                Ok("offline".to_string())
            } else {
                Ok(String::new())
            }
        }
    }

    /// 强制开启开发者模式 / adb / 充电常亮(best-effort,吞错误)。
    async fn harden(&self) {
        for cmd in [
            "settings put global development_settings_enabled 1",
            "settings put global adb_enabled 1",
            "settings put global stay_on_while_plugged_in 7",
        ] {
            let _ = self.execute_command(cmd).await;
        }
    }

    /// 在设备上执行一条命令并返回合并后的输出(`adb -s <serial> shell <cmd>`)。
    ///
    /// `cmd` 作为**单个参数**传给 adb,由设备端 `sh -c` 解析,因此可包含管道 / 分号 /
    /// 重定向(监控命令依赖这一点)。
    pub async fn execute_command(&self, command: &str) -> Result<String> {
        let out = adb_cmd()
            .args(["-s", &self.serial, "shell", command])
            .output()
            .await
            .context("无法执行 adb shell")?;

        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        if out.status.success() {
            return Ok(stdout);
        }

        // 非零退出:若有 stdout 仍当作可用输出(很多 toybox 命令部分失败但有结果);
        // 否则把 stderr 作为错误抛出。
        if !stdout.trim().is_empty() {
            return Ok(stdout);
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(anyhow!(
            "adb 命令失败: {}",
            if stderr.trim().is_empty() {
                format!("exit {:?}", out.status.code())
            } else {
                stderr.trim().to_string()
            }
        ))
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        // 不主动 `adb disconnect`:同一台 adb daemon 可能被其它会话 / 标签共用。
        // 仅标记本逻辑连接已关闭。
        self.connected = false;
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// 打开一个交互式 shell,接管子进程 stdin/stdout 构造 [`PtySession`]。
    ///
    /// 用 `-t -t` 强制分配 PTY(本机无 tty 时也分配),使 `vi`/`top` 等全屏程序可用。
    /// 动态 resize 暂不支持(adb CLI 无运行时改窗口尺寸的接口),`resize_tx` 收到的
    /// 请求被静默丢弃。
    pub async fn create_pty_session(&self, _cols: u32, _rows: u32) -> Result<PtySession> {
        let mut child = adb_cmd()
            .args(["-s", &self.serial, "shell", "-t", "-t"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("无法启动 adb shell(交互式)")?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("adb shell 缺少 stdin"))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("adb shell 缺少 stdout"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("adb shell 缺少 stderr"))?;

        let (input_tx, mut input_rx) = mpsc::channel::<Vec<u8>>(1000);
        let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>(128);
        let (resize_tx, mut resize_rx) = mpsc::channel::<(u32, u32)>(16);
        let cancel = CancellationToken::new();

        // 输入:前端字节 → 子进程 stdin(写后立即 flush 保证交互响应)。
        tokio::spawn(async move {
            while let Some(data) = input_rx.recv().await {
                if stdin.write_all(&data).await.is_err() {
                    break;
                }
                let _ = stdin.flush().await;
            }
        });

        // stderr 也并入输出流(adb 把部分 shell 输出写到 stderr)。
        let err_tx = output_tx.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                match stderr.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if err_tx.send(buf[..n].to_vec()).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        // 输出 + 撤销:读 stdout 推给前端;cancel 触发或 EOF 即 kill 子进程。
        let cancel_reader = cancel.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                tokio::select! {
                    _ = cancel_reader.cancelled() => {
                        let _ = child.kill().await;
                        break;
                    }
                    read = stdout.read(&mut buf) => {
                        match read {
                            Ok(0) | Err(_) => {
                                let _ = child.kill().await;
                                break;
                            }
                            Ok(n) => {
                                if output_tx.send(buf[..n].to_vec()).await.is_err() {
                                    let _ = child.kill().await;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        });

        // resize 暂不支持:消费请求避免 sender 阻塞。
        tokio::spawn(async move { while resize_rx.recv().await.is_some() {} });

        Ok(PtySession {
            input_tx,
            output_rx: Arc::new(Mutex::new(output_rx)),
            resize_tx,
            cancel,
        })
    }

    /// 列目录(`ls -la`),解析为结构化 [`SftpEntry`](与 SSH 的 SFTP 列表同类型)。
    ///
    /// 强制给路径补一个**尾斜杠**:安卓的 `/sdcard` 等常用目录其实是「指向目录的软
    /// 链接」,`ls -la <软链接>` 只会列出软链接自身这一行;加尾斜杠后 `ls` 才会进入其
    /// 目标目录列出内容(对普通目录无副作用)。`-L` 也能解引用,但会把目录内的子软链接
    /// 一并解析成目标类型、破坏 `is_symlink` 判定,故改用尾斜杠。
    pub async fn list_dir(&self, path: &str) -> Result<Vec<SftpEntry>> {
        let dir = format!("{}/", path.trim_end_matches('/'));
        let cmd = format!("ls -la {}", shell_quote(&dir));
        let out = self.execute_command(&cmd).await?;
        Ok(parse_android_ls(&out))
    }

    /// 解析一个路径为绝对路径;`"."` / 空 → 默认到 `/sdcard`(安卓最常用的存储根)。
    pub async fn realpath(&self, path: &str) -> Result<String> {
        if path == "." || path.trim().is_empty() {
            return Ok("/sdcard".to_string());
        }
        match self
            .execute_command(&format!("realpath {}", shell_quote(path)))
            .await
        {
            Ok(out) if !out.trim().is_empty() => Ok(out.trim().to_string()),
            _ => Ok(path.to_string()),
        }
    }

    /// 递归删除远端路径(`rm -rf`,文件 / 目录 / 软链接通吃)。
    pub async fn delete(&self, path: &str) -> Result<()> {
        self.execute_command(&format!("rm -rf {}", shell_quote(path)))
            .await
            .map(|_| ())
    }

    /// 重命名 / 移动远端路径(`mv`)。
    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        self.execute_command(&format!("mv {} {}", shell_quote(from), shell_quote(to)))
            .await
            .map(|_| ())
    }

    /// 远端文件大小(字节);失败返回 0。
    async fn remote_size(&self, remote: &str) -> u64 {
        self.execute_command(&format!("stat -c %s {}", shell_quote(remote)))
            .await
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0)
    }

    /// 上传本地文件到设备(`adb push`),带进度回调(整体两点:0 → total)。
    pub async fn push<F>(&self, local: &str, remote: &str, mut on_progress: F) -> Result<u64>
    where
        F: FnMut(u64, u64) + Send,
    {
        let total = tokio::fs::metadata(local)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        on_progress(0, total);

        let out = adb_cmd()
            .args(["-s", &self.serial, "push", local, remote])
            .output()
            .await
            .context("无法执行 adb push")?;
        if !out.status.success() {
            return Err(anyhow!(
                "adb push 失败: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        on_progress(total, total);
        Ok(total)
    }

    /// 从设备下载文件到本地(`adb pull`),带进度回调(整体两点:0 → total)。
    pub async fn pull<F>(&self, remote: &str, local: &str, mut on_progress: F) -> Result<u64>
    where
        F: FnMut(u64, u64) + Send,
    {
        let total = self.remote_size(remote).await;
        on_progress(0, total);

        let out = adb_cmd()
            .args(["-s", &self.serial, "pull", remote, local])
            .output()
            .await
            .context("无法执行 adb pull")?;
        if !out.status.success() {
            return Err(anyhow!(
                "adb pull 失败: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        let got = tokio::fs::metadata(local)
            .await
            .map(|m| m.len())
            .unwrap_or(total);
        on_progress(got, got.max(total));
        Ok(got)
    }

    /// 读取远端文件全部内容到内存(`adb exec-out cat`,二进制安全)。
    pub async fn read_file(&self, remote: &str) -> Result<Vec<u8>> {
        let out = adb_cmd()
            .args(["-s", &self.serial, "exec-out", "cat", remote])
            .output()
            .await
            .context("无法执行 adb exec-out cat")?;
        if !out.status.success() {
            return Err(anyhow!(
                "读取远端文件失败: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(out.stdout)
    }

    /// 把字节写入远端文件:先落本地临时文件,再 `adb push`(二进制安全)。
    pub async fn write_file(&self, data: &[u8], remote: &str) -> Result<u64> {
        let tmp = std::env::temp_dir().join(format!(
            "rshell-adb-{}.tmp",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        tokio::fs::write(&tmp, data)
            .await
            .context("无法写入临时文件")?;
        let local = tmp.to_string_lossy().to_string();
        let result = self.push(&local, remote, |_, _| {}).await;
        let _ = tokio::fs::remove_file(&tmp).await;
        result
    }
}

/// 与设备配对(安卓 11+ 无线调试「使用配对码配对设备」):`adb pair <host:port> <code>`。
///
/// `host_port` 是手机「无线调试 → 使用配对码配对设备」弹窗里显示的**配对地址**
/// (端口是随机的,**不是**连接用的 5555);`code` 是同一弹窗里的 6 位配对码。
/// 配对只需做一次,成功后该设备会信任本机密钥,后续用 [`connect_device`] 连 5555 即可。
pub async fn pair(host_port: &str, code: &str) -> Result<String> {
    let out = adb_cmd()
        .args(["pair", host_port, code])
        .output()
        .await
        .context("无法执行 adb pair(adb 是否已安装并在 PATH 中?)")?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let trimmed = combined.trim();
    if out.status.success() && trimmed.to_lowercase().contains("successfully paired") {
        Ok(trimmed.to_string())
    } else if trimmed.is_empty() {
        Err(anyhow!(
            "adb pair 无输出:确认配对地址 / 配对码正确,且手机与本机在同一局域网"
        ))
    } else {
        Err(anyhow!("配对失败: {}", trimmed))
    }
}

/// 主动连接网络 adb 设备:`adb connect <host:port>`(`host_port` 用连接端口,通常 5555)。
pub async fn connect_device(host_port: &str) -> Result<String> {
    let out = adb_cmd()
        .args(["connect", host_port])
        .output()
        .await
        .context("无法执行 adb connect(adb 是否已安装并在 PATH 中?)")?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let trimmed = combined.trim();
    let lower = trimmed.to_lowercase();
    if lower.contains("connected to") || lower.contains("already connected") {
        Ok(trimmed.to_string())
    } else if trimmed.is_empty() {
        Err(anyhow!("adb connect 无输出:确认设备地址与网络 adb(5555)已开启"))
    } else {
        Err(anyhow!("连接失败: {}", trimmed))
    }
}

/// 单引号转义,供拼进 `adb shell` 的命令字符串(等价 SSH 后端的 shell_quote)。
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// 解析安卓 toybox `ls -la` 的输出为 [`SftpEntry`] 列表。
///
/// 典型行:`-rw-rw-rw- 1 root root 123 2024-06-01 12:00 file.txt`。
/// 软链接行尾为 `name -> target`,这里只取 `name`。修改时间转 Unix 秒较繁琐且
/// 依赖设备 locale,这里置 0(列表主要给「类型 / 大小 / 名称」)。
fn parse_android_ls(output: &str) -> Vec<SftpEntry> {
    let mut entries = Vec::new();

    for line in output.lines() {
        let line = line.trim_end();
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("total ") {
            continue;
        }

        let fields: Vec<&str> = trimmed.split_whitespace().collect();
        // 至少:perms links owner group size date time name => 8 段
        if fields.len() < 8 {
            continue;
        }
        let permissions = fields[0];
        let first = permissions.chars().next().unwrap_or('-');
        let is_dir = first == 'd';
        let is_symlink = first == 'l';

        // 大小:第 5 段(索引 4);设备 / 字符设备可能是 "a, b" 形式,解析失败按 0。
        let size = fields[4].parse::<u64>().unwrap_or(0);

        // 名称:第 8 段(索引 7)起到行尾;软链接截断 " -> "。
        let name_part = fields[7..].join(" ");
        let name = name_part
            .split(" -> ")
            .next()
            .unwrap_or(&name_part)
            .to_string();
        if name == "." || name == ".." || name.is_empty() {
            continue;
        }

        entries.push(SftpEntry {
            name,
            is_dir,
            is_symlink,
            size,
            permissions: permissions.to_string(),
            modified_unix: 0,
        });
    }

    entries.sort_by(|a, b| {
        let rank = |e: &SftpEntry| if e.is_dir { 0 } else { 1 };
        rank(a)
            .cmp(&rank(b))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_android_ls() {
        let output = "\
total 0
drwxrwx--x  4 root  sdcard_rw 4096 2024-06-01 12:00 Download
-rw-rw-rw-  1 root  sdcard_rw  123 2024-06-01 12:01 hello world.txt
lrwxrwxrwx  1 root  root         7 2024-06-01 12:02 latest -> release
";
        let entries = parse_android_ls(output);
        assert_eq!(entries.len(), 3);
        // 目录排前。
        assert_eq!(entries[0].name, "Download");
        assert!(entries[0].is_dir);
        // 软链接只取名字。
        let link = entries.iter().find(|e| e.is_symlink).unwrap();
        assert_eq!(link.name, "latest");
        // 含空格的文件名完整保留。
        assert!(entries.iter().any(|e| e.name == "hello world.txt" && e.size == 123));
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("/sdcard/a b"), "'/sdcard/a b'");
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }
}
