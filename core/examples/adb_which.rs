//! 诊断:打印 Conch / R-Shell 实际会使用哪一个 adb,并尝试 `adb version`。
//!
//! 用于排查「开发能连、生产连不上」——在不同启动环境下跑它,看解析到的 adb 路径:
//!   cargo run -p r-shell-core --example adb_which
//! 也可把本二进制拷进 `Conch.app/Contents/MacOS/` 内运行,验证「内嵌优先」是否命中。

fn main() {
    let path = r_shell_core::adb_bin::adb_program();
    println!("resolved adb = {}", path.display());
    match std::process::Command::new(path).arg("version").output() {
        Ok(o) => {
            print!("{}", String::from_utf8_lossy(&o.stdout));
            if !o.status.success() {
                eprintln!(
                    "(exit {:?}) {}",
                    o.status.code(),
                    String::from_utf8_lossy(&o.stderr)
                );
            }
        }
        Err(e) => eprintln!("spawn adb failed: {e}"),
    }
}
