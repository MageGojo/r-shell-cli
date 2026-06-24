//! adb 可执行文件定位 —— 解决「开发能连安卓、生产环节连不上」。
//!
//! 根因:代码以前一律 `Command::new("adb")`,只靠系统 `PATH` 找 adb。
//! - 开发时从终端 / `cargo run` 启动,继承了 shell 的完整 PATH(含 Homebrew、
//!   Android SDK platform-tools),adb 能找到。
//! - 生产时 GUI 多半从 Finder/Dock(macOS)启动,拿到的是 launchd 最小 PATH
//!   (`/usr/bin:/bin:/usr/sbin:/sbin`),**不读 `.zshrc`**,看不到 Homebrew/SDK 里的
//!   adb;交付给别人的干净机器更是压根没装 → `Command::new("adb")` 直接 ENOENT。
//!
//! 解法(本模块):优先使用**随应用内嵌**的 adb(零环境交付),再按一串已知位置兜底;
//! 全部找不到才回退裸名 `adb`(保持「机器已把 adb 放进 PATH」的旧行为)。所有 adb
//! 调用统一走 [`adb_program`],一处定位、全量生效。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// adb 可执行文件名(Windows 带 `.exe`)。
#[cfg(windows)]
const ADB_FILE: &str = "adb.exe";
#[cfg(not(windows))]
const ADB_FILE: &str = "adb";

/// 返回应当传给 `Command::new(...)` 的 adb 路径(进程级解析一次后缓存)。
///
/// 解析顺序见 [`resolve`]。任何已知位置都找不到时回退裸名 `adb`。
pub fn adb_program() -> &'static Path {
    static RESOLVED: OnceLock<PathBuf> = OnceLock::new();
    RESOLVED
        .get_or_init(|| resolve().unwrap_or_else(|| PathBuf::from(ADB_FILE)))
        .as_path()
}

/// 按优先级解析 adb 路径:
/// 1. 显式覆盖环境变量 `CONCH_ADB` / `R_SHELL_ADB` / `ADB_PATH`;
/// 2. **随应用内嵌**的 adb(相对可执行文件,见 [`bundled_under`])——命中时顺手修可执行位 / 去隔离;
/// 3. Android SDK:`$ANDROID_HOME` / `$ANDROID_SDK_ROOT` 下的 `platform-tools/adb`;
/// 4. `PATH` 查找(GUI 从终端启动时仍有效);
/// 5. 各平台常见安装目录。
fn resolve() -> Option<PathBuf> {
    // 1) 显式覆盖(最高优先,便于排障与定制)。
    for key in ["CONCH_ADB", "R_SHELL_ADB", "ADB_PATH"] {
        if let Some(v) = std::env::var_os(key) {
            let p = PathBuf::from(v);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    // 2) 随应用内嵌(零环境交付的主路径)。
    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = bundled_under(&exe).into_iter().find(|p| p.is_file()) {
            prepare_bundled(&p);
            return Some(p);
        }
    }

    // 3) Android SDK 环境变量。
    for key in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Some(sdk) = std::env::var_os(key) {
            let p = Path::new(&sdk).join("platform-tools").join(ADB_FILE);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    // 4) PATH 查找。
    if let Some(p) = which_in_path(ADB_FILE) {
        return Some(p);
    }

    // 5) 常见安装目录兜底。
    common_dirs().into_iter().find(|p| p.is_file())
}

/// 计算「相对可执行文件」的内嵌 adb 候选路径(纯函数,便于测试)。
///
/// - 通用:可执行文件同级的 `adb/<adb>`(Windows Release 目录 / Linux bundle 用)。
/// - macOS `.app`:`Contents/MacOS/<exe>` → `Contents/Resources/adb/<adb>`。
fn bundled_under(exe: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Some(dir) = exe.parent() else {
        return out;
    };
    out.push(dir.join("adb").join(ADB_FILE));
    #[cfg(target_os = "macos")]
    if let Some(contents) = dir.parent() {
        // dir = Contents/MacOS → contents = Contents
        out.push(contents.join("Resources").join("adb").join(ADB_FILE));
    }
    out
}

/// 在 `$PATH` 各目录里找 `name`。
fn which_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|cand| cand.is_file())
}

/// 各平台 adb 常见安装目录(SDK / 包管理器默认落点)。
fn common_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut v = Vec::new();

    #[cfg(target_os = "macos")]
    {
        v.push(PathBuf::from("/opt/homebrew/bin/adb"));
        v.push(PathBuf::from("/usr/local/bin/adb"));
        v.push(PathBuf::from(
            "/opt/homebrew/share/android-commandlinetools/platform-tools/adb",
        ));
        if let Some(h) = &home {
            v.push(h.join("Library/Android/sdk/platform-tools/adb"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        v.push(PathBuf::from("/usr/local/bin/adb"));
        v.push(PathBuf::from("/usr/bin/adb"));
        if let Some(h) = &home {
            v.push(h.join("Android/Sdk/platform-tools/adb"));
        }
    }

    #[cfg(windows)]
    {
        let _ = &home;
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            v.push(Path::new(&local).join("Android\\Sdk\\platform-tools\\adb.exe"));
        }
        if let Some(pf) = std::env::var_os("ProgramFiles") {
            v.push(Path::new(&pf).join("Android\\platform-tools\\adb.exe"));
        }
    }

    v
}

/// 对**内嵌副本**做 best-effort 自愈:补可执行位、去 macOS 下载隔离属性。
///
/// 失败一律忽略(例如 App 安装到 `/Applications` 后 Resources 只读)——内嵌副本在打包
/// 阶段已 `chmod +x` 并 ad-hoc 签名,这里只是多一层保险。
#[cfg(unix)]
fn prepare_bundled(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(p) {
        if meta.permissions().mode() & 0o111 == 0 {
            let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755));
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("/usr/bin/xattr")
            .args(["-d", "com.apple.quarantine"])
            .arg(p)
            .output();
    }
}

#[cfg(windows)]
fn prepare_bundled(_p: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_includes_exe_sibling_adb_dir() {
        let exe = if cfg!(windows) {
            PathBuf::from("C:\\app\\Release\\Conch.exe")
        } else {
            PathBuf::from("/opt/app/conch")
        };
        let cands = bundled_under(&exe);
        // 第一候选永远是 exe 同级的 adb/<adb>。
        assert_eq!(cands[0], exe.parent().unwrap().join("adb").join(ADB_FILE));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn bundled_includes_macos_app_resources() {
        let exe = PathBuf::from("/Applications/Conch.app/Contents/MacOS/Conch");
        let cands = bundled_under(&exe);
        assert!(cands.contains(&PathBuf::from(
            "/Applications/Conch.app/Contents/Resources/adb/adb"
        )));
    }

    #[test]
    fn which_finds_nothing_for_bogus_name() {
        // 一个几乎不可能存在于 PATH 的名字。
        assert!(which_in_path("definitely-not-a-real-binary-xyzzy-9999").is_none());
    }
}
