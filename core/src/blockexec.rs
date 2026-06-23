//! 命令块(Block Terminal · Warp 式)执行辅助 —— 见 docs/gui/11-命令块终端原型.md。
//!
//! 命令块每次只跑**一条命令**并把它的完整输出(stdout+stderr 合并)、退出码、以及
//! 命令结束后的工作目录捕获成「一块」。难点在于:每条命令各自起一个 exec 通道
//! (SSH)/ 子进程(ADB / 本机),通道之间**不共享 shell 状态**,于是 `cd /tmp`
//! 之后下一条命令并不在 `/tmp`。
//!
//! 这里用一小段 **POSIX shell 包装**解决:在用户命令之后,打印一行带「退出码 + pwd」
//! 的哨兵:
//!
//! ```sh
//! { cd <cwd> 2>/dev/null; <command>; } 2>&1; __rsrc=$?; printf '\x1e%s\x1e%s\x1e' "$__rsrc" "$(pwd)"
//! ```
//!
//! - `{ …; } 2>&1` 把组内 stderr 并进 stdout(SSH `execute_command` 只捕获 stdout,
//!   不合并就看不到报错)。
//! - 末尾 `printf` 恒退出 0,因此 SSH/ADB 看到整条命令「成功」并原样回传全部输出
//!   (真实退出码从哨兵解析)。
//! - `$(pwd)` 在组之后取值,会反映用户命令里的 `cd`,于是调用方能把新 cwd 带进下一块。
//!
//! Windows 远端默认 shell 为 PowerShell,不认这套 POSIX 语法。为此另有
//! [`wrap_powershell`]:用等价的 PowerShell 片段(`Set-Location` + `& {…} 2>&1 |
//! Out-String` + `$LASTEXITCODE`/`$?` + `[Console]::Out.Write` 打同一套哨兵),
//! 调用方(bridge)按「先试 POSIX,未命中哨兵再试 PowerShell」自动探测并缓存
//! 该连接的 shell 种类。两种包装都解析不到哨兵时,[`parse_outcome`] 优雅降级
//! (原样返回输出、`had_marker = false`)。

use anyhow::Result;

/// 哨兵分隔符:RS(Record Separator,0x1E),正常命令输出里几乎不会出现。
pub const MARKER: char = '\u{1e}';

/// 一条命令块执行的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockOutcome {
    /// 合并后的命令输出(stdout+stderr),已剥离哨兵尾巴。
    pub body: String,
    /// 用户命令的退出码;未命中哨兵(非 POSIX 远端)时为 `-1`。
    pub exit_code: i32,
    /// 命令结束后的工作目录(供下一块沿用);未命中哨兵时回退为传入的 cwd。
    pub cwd: String,
    /// 是否成功解析到哨兵(即退出码 / cwd 是否可信)。
    pub had_marker: bool,
}

/// 单引号转义,供拼进 shell 命令字符串。
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// PowerShell 单引号字符串转义(内部 `'` 写成 `''`)。
fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// 把用户命令包装成带「cd 进入工作目录 + 哨兵(退出码 / pwd)」的 POSIX shell 片段。
///
/// `cwd` 为空则不 `cd`(首次执行时 `$(pwd)` 会回报 shell 默认目录,作为初始 cwd)。
/// `command` 为空白则用 `:`(no-op)兜底,避免 `{ ; }` 语法错误。
pub fn wrap_posix(cwd: &str, command: &str) -> String {
    let command = if command.trim().is_empty() {
        ":"
    } else {
        command
    };
    let cd = if cwd.trim().is_empty() {
        String::new()
    } else {
        format!("cd {} 2>/dev/null; ", shell_quote(cwd))
    };
    // 注意:format! 里 `{{`/`}}` 是字面花括号;`\\036` 让最终字符串含「反斜杠+036」,
    // 交给远端 printf 解释成八进制 036(= 0x1E)。
    format!(
        "{{ {cd}{command}; }} 2>&1; __rsrc=$?; printf '\\036%s\\036%s\\036' \"$__rsrc\" \"$(pwd)\""
    )
}

/// 把用户命令包装成等价的 **PowerShell** 片段(Windows 远端默认 shell),打出与
/// [`wrap_posix`] 完全相同的哨兵 `\x1e<rc>\x1e<pwd>\x1e`,故 [`parse_outcome`] 通用。
///
/// 要点(对齐 `monitor::windows_stats_command` 的可用写法):**只用单引号**(规避
/// Windows-OpenSSH exec 的双引号 mangling)、强制 `OutputEncoding=UTF8`(防中文乱码)、
/// `& {…} 2>&1 | Out-String` 合并 stderr 并渲染对象输出、退出码取 `$LASTEXITCODE`
/// (原生程序)否则按 `$?` 折成 0/1(cmdlet)、`(Get-Location).Path` 反映 `cd`、
/// 末尾 `exit 0` 让 SSH 看到「成功」(真实退出码在哨兵里)。
pub fn wrap_powershell(cwd: &str, command: &str) -> String {
    let inner = if command.trim().is_empty() {
        "$null"
    } else {
        command
    };
    let cd = if cwd.trim().is_empty() {
        String::new()
    } else {
        format!("try{{Set-Location -LiteralPath {}}}catch{{}}; ", ps_quote(cwd))
    };
    let mut s = String::new();
    s.push_str("[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; ");
    s.push_str("$ErrorActionPreference='Continue'; ");
    s.push_str(&cd);
    // Out-String 会把每行按 -Width 用空格补齐;逐行 TrimEnd 去掉尾随空白再以 LF 拼接,
    // 否则表格输出会膨胀(每行被补到几千字符)。
    s.push_str("$o=(& {");
    s.push_str(inner);
    s.push_str("} 2>&1 | Out-String -Stream -Width 512 | ForEach-Object {$_.TrimEnd()}) -join [char]10; ");
    s.push_str("$ok=$?; $c=$LASTEXITCODE; if($null -eq $c){if($ok){$c=0}else{$c=1}}; ");
    s.push_str(
        "[Console]::Out.Write($o+[char]30+[string]$c+[char]30+(Get-Location).Path+[char]30); ",
    );
    s.push_str("[Console]::Out.Flush(); exit 0");
    s
}

/// 解析 [`wrap_posix`] 跑出来的原始输出,拆出 body / 退出码 / 新 cwd。
///
/// 期望尾部形如 `…body…\x1e<rc>\x1e<pwd>\x1e`。找不到合法哨兵则原样返回
/// (`had_marker = false`、`exit_code = -1`、`cwd = fallback_cwd`)。
pub fn parse_outcome(raw: &str, fallback_cwd: &str) -> BlockOutcome {
    let parts: Vec<&str> = raw.split(MARKER).collect();
    // 至少要有 [body, rc, pwd, trailing] 四段(body 可能为空串)。
    if parts.len() >= 4 {
        let n = parts.len();
        let trailing = parts[n - 1];
        let pwd = parts[n - 2].trim();
        let rc = parts[n - 3].trim();
        // 哨兵完整性:trailing 应为空白,rc 应能解析成整数。
        if trailing.trim().is_empty() {
            if let Ok(code) = rc.parse::<i32>() {
                // body = 末尾三段之前的全部(若 body 自身含 MARKER 则原样拼回)。
                let body = parts[..n - 3].join(&MARKER.to_string());
                let cwd = if pwd.is_empty() {
                    fallback_cwd.to_string()
                } else {
                    pwd.to_string()
                };
                return BlockOutcome {
                    body,
                    exit_code: code,
                    cwd,
                    had_marker: true,
                };
            }
        }
    }
    BlockOutcome {
        body: raw.to_string(),
        exit_code: -1,
        cwd: fallback_cwd.to_string(),
        had_marker: false,
    }
}

/// 在**本机**(运行 GUI 的这台电脑)跑一条命令块。
///
/// 用 `$SHELL -c <wrapped>`(缺省 `/bin/sh`)执行 [`wrap_posix`] 包装后的片段,
/// 故同样支持 `cd` 跨块持久 + 退出码 + stderr 合并。仅 POSIX(macOS / Linux)。
pub async fn run_local(cwd: &str, command: &str) -> Result<BlockOutcome> {
    let wrapped = wrap_posix(cwd, command);
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "/bin/sh".to_string());

    let output = tokio::process::Command::new(&shell)
        .arg("-c")
        .arg(&wrapped)
        .output()
        .await?;

    // 包装内 `2>&1` 已把 stderr 并入 stdout;stderr 理应为空,保险起见仍追加。
    let mut raw = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.stderr.is_empty() {
        raw.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    Ok(parse_outcome(&raw, cwd))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_includes_cd_command_and_marker_printf() {
        let w = wrap_posix("/tmp/a b", "ls -la");
        assert!(w.contains("cd '/tmp/a b' 2>/dev/null;"));
        assert!(w.contains("ls -la"));
        assert!(w.contains("2>&1"));
        assert!(w.contains("__rsrc=$?"));
        assert!(w.contains(r"printf '\036%s\036%s\036'"));
        assert!(w.contains("$(pwd)"));
    }

    #[test]
    fn wrap_without_cwd_skips_cd() {
        let w = wrap_posix("", "pwd");
        assert!(!w.contains("cd "));
        assert!(w.contains("pwd"));
    }

    #[test]
    fn wrap_blank_command_is_noop() {
        let w = wrap_posix("/x", "   ");
        // 命令位被替换为 `:`,不会产生 `{ ; }`。
        assert!(w.contains("{ cd '/x' 2>/dev/null; :; }"));
    }

    #[test]
    fn wrap_escapes_single_quotes_in_cwd() {
        let w = wrap_posix("/o'ut", "echo hi");
        assert!(w.contains("cd '/o'\\''ut' 2>/dev/null;"));
    }

    #[test]
    fn powershell_wrap_has_setloc_command_and_marker() {
        let w = wrap_powershell("C:\\Users\\me", "Get-ChildItem");
        assert!(w.contains("Set-Location -LiteralPath 'C:\\Users\\me'"));
        assert!(w.contains("& {Get-ChildItem} 2>&1 | Out-String -Stream"));
        assert!(w.contains("[char]30")); // 哨兵分隔符
        assert!(w.contains("(Get-Location).Path"));
        assert!(w.contains("exit 0"));
        assert!(w.contains("OutputEncoding=[System.Text.Encoding]::UTF8"));
    }

    #[test]
    fn powershell_wrap_without_cwd_skips_setloc() {
        let w = wrap_powershell("", "pwd");
        assert!(!w.contains("Set-Location"));
        assert!(w.contains("& {pwd}"));
    }

    #[test]
    fn powershell_wrap_escapes_single_quotes_in_cwd() {
        let w = wrap_powershell("C:\\o'ut", "ls");
        assert!(w.contains("Set-Location -LiteralPath 'C:\\o''ut'"));
    }

    #[test]
    fn powershell_wrap_blank_command_uses_null() {
        let w = wrap_powershell("C:\\", "   ");
        assert!(w.contains("& {$null}"));
    }

    #[test]
    fn parse_success_marker() {
        let raw = format!("hello\nworld\n{m}0{m}/tmp{m}", m = MARKER);
        let out = parse_outcome(&raw, "/start");
        assert!(out.had_marker);
        assert_eq!(out.body, "hello\nworld\n");
        assert_eq!(out.exit_code, 0);
        assert_eq!(out.cwd, "/tmp");
    }

    #[test]
    fn parse_nonzero_exit() {
        let raw = format!("boom\n{m}127{m}/home/u{m}", m = MARKER);
        let out = parse_outcome(&raw, "/start");
        assert!(out.had_marker);
        assert_eq!(out.exit_code, 127);
        assert_eq!(out.cwd, "/home/u");
        assert_eq!(out.body, "boom\n");
    }

    #[test]
    fn parse_no_marker_degrades_gracefully() {
        let raw = "PowerShell style output, no sentinel";
        let out = parse_outcome(raw, "/keep");
        assert!(!out.had_marker);
        assert_eq!(out.exit_code, -1);
        assert_eq!(out.cwd, "/keep");
        assert_eq!(out.body, raw);
    }

    #[test]
    fn parse_empty_body_with_marker() {
        let raw = format!("{m}0{m}/{m}", m = MARKER);
        let out = parse_outcome(&raw, "/start");
        assert!(out.had_marker);
        assert_eq!(out.body, "");
        assert_eq!(out.exit_code, 0);
        assert_eq!(out.cwd, "/");
    }

    #[tokio::test]
    async fn run_local_echo_captures_output_and_exit() {
        let out = run_local("/", "echo block-term-ok").await.unwrap();
        assert!(out.had_marker, "本机 sh 应命中哨兵");
        assert!(out.body.contains("block-term-ok"));
        assert_eq!(out.exit_code, 0);
        // cd / 后 pwd 应为根。
        assert_eq!(out.cwd, "/");
    }

    #[tokio::test]
    async fn run_local_nonzero_exit_is_reported() {
        let out = run_local("/", "false").await.unwrap();
        assert!(out.had_marker);
        assert_ne!(out.exit_code, 0);
    }

    #[tokio::test]
    async fn run_local_cd_persists_into_cwd() {
        // 用户命令里的 cd 应反映到结果 cwd(供下一块沿用)。
        let out = run_local("/", "cd /usr && echo moved").await.unwrap();
        assert!(out.body.contains("moved"));
        assert_eq!(out.exit_code, 0);
        assert_eq!(out.cwd, "/usr");
    }
}
