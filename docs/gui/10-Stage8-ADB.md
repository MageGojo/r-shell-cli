# Stage 8：ADB 通道（用 R-Shell 管安卓）

> 状态：**真机端到端已验证通过（2026-06-23，Redmi 220233L2C / Android 11 / armeabi-v7a）**——终端 / 文件 / 监控全绿；发现并修复 `/sdcard` 软链接目录列空的 bug。
> 关联：[架构设计](02-架构设计.md) · [进度.md](进度.md)
> 决策来源：用户「做手机的支持，连接安卓 192.168.0.101:5555」「方案 B：给 r-shell-cli 新增一个 ADB 通道（core 加 adb backend、model 加 protocol=ADB、GUI 适配），直接走 5555 管理安卓的 shell/文件。由 rust 实现，flutter 做 UI」。

## 1. 目标

在不破坏现有 SSH 能力的前提下，给 R-Shell 增加一类连接 **协议 = ADB**，复用既有的「多标签终端 / SFTP 双栏 / 监控仪表盘」三大界面，直接通过 `adb`（5555 网络 ADB）管理安卓设备的 shell 与文件。CLI / GUI 同源（core 为单一事实来源）。

## 2. 关键架构决策

### 2.1 后端抽象：`Backend` enum（而非 trait object）
`NativeConnectionManager` 原本只存 `Arc<RwLock<SshClient>>`。新增 `core/src/native_backend.rs` 内的私有枚举：

```rust
enum Backend { Ssh(SshClient), Adb(AdbClient) }
```

manager 的注册表改为 `HashMap<String, Arc<RwLock<Backend>>>`，所有操作（exec / PTY / SFTP / 监控）在 `Backend` 上做一次 `match` 分派转发。

- **为什么用 enum 不用 `dyn Trait`**：SFTP 收发是泛型回调 `FnMut(u64,u64)`，trait object 无法直接容纳泛型方法；enum 转发改动集中、类型清晰、零动态分发开销。
- **收益**：bridge 与 Flutter 几乎无需感知后端类型——终端 `pty_*`、SFTP `sftp_*`、监控 `stats_stream` 全部走 manager 既有方法，对 ADB 透明。

### 2.2 后端无关的 `PtySession`（新模块 `core/src/pty.rs`）
原 `PtySession` 定义在 `ssh.rs` 且带 russh 专属字段 `channel_id: ChannelId`（实际无人读取）。抽到独立 `pty` 模块并去掉 `channel_id`，只保留四个传输无关的通道：`input_tx / output_rx / resize_tx / cancel`。SSH（russh channel）与 ADB（`adb shell` 子进程）都构造同一个 `PtySession`，多标签终端逻辑两端通用。

### 2.3 ADB 是「子进程后端」（`core/src/adb.rs` → `AdbClient`）
与 SSH 的 russh 长连接不同，ADB 的会话状态由本机 `adb` daemon 维护（`adb connect host:port` 后保活），`AdbClient` 几乎无状态，每个操作 spawn 一个 `adb` 子进程：

| 能力 | 实现 |
| --- | --- |
| 命令执行 / 监控采集 | `adb -s <serial> shell <cmd>`（`cmd` 作为单参数，设备端 `sh -c` 解析，可含管道/分号/重定向） |
| 交互式终端 | `adb -s <serial> shell -t -t`（强制分配 PTY），接管子进程 stdin/stdout 构造 `PtySession` |
| 列目录 | `ls -la` → `parse_android_ls` 解析为 `SftpEntry`（类型/大小/名称；软链接取名） |
| 上传 / 下载 | `adb push` / `adb pull`（带「0 → total」两点进度） |
| 读文件到内存 | `adb exec-out cat <remote>`（二进制安全） |
| 写文件 | 落临时文件后 `adb push`（二进制安全） |
| realpath | `realpath`；`"."`/空 → 默认 `/sdcard`（安卓最常用存储根） |

`serial = host:port`（如 `192.168.0.101:5555`）。

### 2.4 监控几乎零成本复用 Linux 路径
安卓是 Linux 内核，core 的 `stats_command()` 全是 `/proc/stat`、`/proc/meminfo`、`/proc/loadavg`、`/proc/uptime`、`/proc/net/dev`、`df -kP /`、`nproc`、`uname -sr`——这些在安卓 toybox（root 后更全）均可用。监控走 `execute_command` 透明分派，故 ADB 设备的 CPU / 内存 / 磁盘 / 网速 / uptime **直接可用**，无需新增采集器。GUI 监控间隔默认 **1000ms（每秒刷新）**（`MonitorController.intervalMs = 1000`）。

### 2.5 连接后强制开启开发者模式（harden）
针对用户「开发者模式容易掉」：`AdbClient::connect` 成功后 best-effort（失败不阻断）执行：

```
settings put global development_settings_enabled 1
settings put global adb_enabled 1
settings put global stay_on_while_plugged_in 7
```

adb shell 的 uid=2000 默认持有 `WRITE_SECURE_SETTINGS`，**无需 root** 即可生效。更强的持久化（把本机公钥写进 `/data/misc/adb/adb_keys`、`setprop persist.adb.tcp.port 5555`）需 root，待真机授权后用 `adb root` 单独固化（不放进每次连接流程，避免 `adb root` 触发断连）。

### 2.6 协议字段（model / connections）
- `model.rs::normalized()`：原来**强制过滤所有非 SSH 连接**并把 `protocol` 锁成 `"SSH"`；改为保留 `SSH` 与 `ADB`，其余规范化大写。
- `connections.rs`：`NewConnection` / `ConnectionPatch` 增加 `protocol` 字段；`build_connection` 对 ADB **放宽校验**（只需 name + host + port，无用户名 / 认证）。CLI 仍传 `protocol: "SSH"`（零回归）。
- bridge `ConnectionDto` / `ConnectionInput` 增加 `protocol`；`ensure_session` 按协议分派：ADB → `create_adb_connection(serial)`，SSH → `build_ssh_config` + `create_connection`。

## 3. 改动清单

**core**
- 新增 `pty.rs`（后端无关 `PtySession`）、`adb.rs`（`AdbClient` + `parse_android_ls`，含 2 个单测）。
- `ssh.rs`：`PtySession` 移出、去 `channel_id`。
- `native_backend.rs`：`Backend` enum + 转发；`create_adb_connection` / `insert_backend`；注册表改 `Backend`。
- `model.rs`：`normalized` 放行 ADB。
- `connections.rs`：`protocol` 字段（New/Patch）+ ADB 校验放宽。
- `lib.rs`：注册 `adb` / `pty` 模块。
- `cli/main.rs`：`NewConnection` 补 `protocol: "SSH"`。

**bridge（desktop/rust）**
- `api/connections.rs`：DTO / Input 加 `protocol`。
- `api/mod.rs`：`ensure_session` 按协议分派。
- frb codegen 重新生成绑定。

**Flutter（desktop/lib）**
- `connection_edit_dialog.dart`：常规页签加「协议」选择器（SSH / ADB）；ADB 时第二页签变「ADB」只显示设备地址 + 端口 + 说明卡，端口默认 5555（22↔5555 智能切换）；`_save` 按协议分流。
- `connection_tree.dart`：ADB 显示安卓绿色角标 + `adb · host:port` 副标题。
- 监控间隔确认为每秒（无需改）。

## 4. 已知限制 / TODO
- ADB 交互终端**暂不支持动态 resize**（adb CLI 无运行时改窗口尺寸接口），`resize_tx` 请求被丢弃。
- 文件传输进度为「0 → total」两点（未解析 `adb push/pull` 的百分比输出）。
- `ls -la` 列表的 `modified_unix` 置 0（转 Unix 秒依赖设备 locale，暂不解析）。
- 持久化授权（写 `adb_keys`）/ `persist.adb.tcp.port` 需 root，本机为零售 MIUI（`adb root` 返回 `adbd cannot run as root in production builds`）→ **走不通**；非 root 持久化见 §6。

### 4.1 已修复（真机暴露）
- **`/sdcard` 等「软链接指向的目录」列空**：`/sdcard` 实为 `→ /storage/self/primary` 的软链接，`ls -la /sdcard` 只会列出软链接自身那一行（真机实测 `sftp_list` 只返回 1 项 `LNK /sdcard`）。修复：`AdbClient::list_dir` 给路径强制补**尾斜杠**（`ls -la /sdcard/`）强制进入目标目录列内容；不用 `-L`（`-L` 会把目录内的子软链接也解引用成目标类型、破坏 `is_symlink` 判定）。修复后 `/sdcard` 正确列出 128 项。

## 5. 真机验证结果（2026-06-23 ✅）

设备：Redmi `220233L2C` · Android 11 · `armeabi-v7a` · `uid=2000(shell)`。验证方式：`core/examples/adb_smoke.rs` 用 `NativeConnectionManager` 的公开 API（**GUI bridge 调的同一套**）对真机端到端跑一遍。

| # | 能力 | API | 结果 |
| --- | --- | --- | --- |
| 1 | 授权 | `adb pair`（无线调试配对码）→ `device` | ✅ 5555 与无线调试口均 `device` |
| 2 | 连接 + harden | `create_adb_connection` | ✅ 连接成功，强制开发者模式已执行 |
| 3 | 命令执行 | `execute_command` | ✅ `getprop` / `id` / 管道·分号·中文均正常 |
| 4 | 列目录 | `sftp_list("/sdcard")` | ✅ 128 项（修复软链接 bug 后） |
| 5 | realpath | `sftp_realpath(".")` | ✅ `/sdcard` |
| 6 | 读写文件 | `write_file_from_bytes` / `read_file_to_memory` | ✅ 写 44B 读回一致（含中文） |
| 7 | 上传下载 | `upload_file` / `download_file` | ✅ 4096B 往返字节一致 |
| 8 | 监控 | `fetch_system_snapshot` ×2 → `SystemStats` | ✅ OS/Uptime/CPU%/内存/磁盘/网速全有 |
| 9 | 交互终端 | `open_pty` / `write_pty` / `read_pty` | ✅ 捕获 `PTY_MARK_42`，提示符 `dandelion:/ $` |

验收：`cargo build` 零警告 · `cargo test -p r-shell-core` **37 通过** · `examples/adb_smoke` 真机全绿。
workspace 已加 ADB 连接：`安卓手机 (Redmi 220233L2C)` = `192.168.0.101:5555`（protocol=ADB）。

> 复跑：`cargo run -p r-shell-core --example adb_smoke -- 192.168.0.101:5555`

## 6. 免 root 持久化（本机零售 MIUI，`adb root` 不可用）

`adb root` 被生产版拒绝，故无法写 `/data/misc/adb/adb_keys` 或 `persist.adb.tcp.port`。非 root 现实：

- **密钥信任已持久**：无线调试配对后，本机被设备信任，**重启后 USB / 已配对的无线调试免再授权**。
- **网络 adb 监听不持久**：`adb tcpip 5555` 的监听态**不随重启保留**（无 root 无法设 persist 属性）。重启后恢复网络 adb 的步骤：
  1. 手机重新打开「无线调试」（MIUI 重启后常自动关）。
  2. 本机 `adb mdns services` 会自动发现设备（已配对，免再配对）。
  3. 一行命令回到稳定 5555：`adb -s <mdns或当前口> tcpip 5555 && adb connect 192.168.0.101:5555`。
- **彻底固化需 root**：若设备 root，可 `adb root` 后写 `adb_keys` + `setprop persist.adb.tcp.port 5555`，重启即自动网络 adb（待设备 root 后再做）。
- harden（`development_settings_enabled` / `adb_enabled` / `stay_on_while_plugged_in`）每次连接 best-effort 执行，无需 root，缓解「开发者模式被关」。

## 7. 内嵌 adb · 零环境交付（2026-06-24 ✅）

> 决策来源：用户「开发能连安卓、生产环节连不上 → 内嵌会更稳，直接内嵌，且内嵌了要能直接用、不依赖其它环节」。

### 7.1 根因
ADB 后端一律 `Command::new("adb")`，只靠系统 `PATH` 找 adb。开发时从终端 / `cargo run` 启动，继承 shell 完整 PATH（含 Homebrew、SDK platform-tools），能找到；**生产时 GUI 从 Finder/Dock 启动（macOS）只拿到 launchd 最小 PATH `/usr/bin:/bin:/usr/sbin:/sbin`、不读 `.zshrc`**，看不到 Homebrew/SDK 里的 adb；交付给别人的干净机更是压根没装 → `Command::new("adb")` 直接 `ENOENT`，表现为「连不上安卓」。

### 7.2 解法：内嵌优先的 adb 解析器（`core/src/adb_bin.rs`）
新增 `adb_bin::adb_program()`（进程级解析一次后缓存），`adb.rs` 全部 `Command::new("adb")` 改走它。解析顺序：
1. 显式覆盖 `CONCH_ADB` / `R_SHELL_ADB` / `ADB_PATH`；
2. **随应用内嵌**（相对 `current_exe`）：macOS `Contents/Resources/adb/adb`、Windows/Linux 可执行同级 `adb/adb[.exe]`；命中时 best-effort 补 `+x` / 去 `com.apple.quarantine`；
3. `$ANDROID_HOME` / `$ANDROID_SDK_ROOT` 下 `platform-tools/adb`；
4. `PATH` 查找；
5. 各平台常见安装目录；全落空才回退裸名 `adb`（保旧行为，CLI 零回归）。

### 7.3 打包内嵌（把 adb 一起带上）
- **macOS** `scripts/build_mac.sh`（新）：`flutter build macos` → 拷 adb 进 `Conch.app/Contents/Resources/adb/adb` → ad-hoc 重签（adb + 整包 `--deep`，加文件会破坏原签名封印）→ `ditto` 压 `dist/Conch-macos.zip`。内嵌的 adb 是 **universal（x86_64+arm64）且仅依赖系统库**（`otool -L` 仅 `/usr/lib`+`/System`）→ Intel/Apple Silicon 干净机双双即用。
- **Windows** `scripts/build_win.ps1`（扩展）：把 `adb.exe` + `AdbWinApi.dll` + `AdbWinUsbApi.dll` 拷进 `Release\adb\`（adb.exe 运行依赖那两个 dll）；zip 与 NSIS（`File /r`）整目录打包自动带上。
- **adb 来源** `scripts/fetch_adb.sh`（新）：缓存 `vendor/adb/<plat>/` 命中即用，否则下载 Google 官方 platform-tools（darwin 兜底拷本机 adb）；`vendor/adb/` 已入 `.gitignore`。

### 7.4 验证（本机 macOS，2026-06-24）
- `cargo test -p r-shell-core` **57 通过**（含 `adb_bin` 3 新测：mac `.app` Resources 落点 / exe 同级 / PATH 查找）。
- 把诊断器 `core/examples/adb_which.rs`（打印解析到的 adb 路径 + `adb version`）拷进 `Conch.app/Contents/MacOS/` 内运行：解析结果 = **内嵌副本** `…/Conch.app/Contents/Resources/adb/adb`（而非 Homebrew 那个），`adb version` 正常。
- `build_mac.sh` 自检：内嵌 adb 跑通 + 「deps OK（仅 /usr/lib 与 /System 系统库）」。

### 7.5 注意
- 内嵌 adb 首次命令会拉起本机 adb server（5037）；若机器另有不同版本 adb server，会自动「kill 后重启」（adb 正常行为，无需人工）。默认不改端口以最大化兼容。
- adb 二进制「内嵌即可用」；**安卓侧仍需手机开启无线调试 / USB 调试并授权**（这是设备侧前置，非本机环境问题）。
