# Stage 4：SFTP 双栏文件管理 + 拖拽上传 + 传输队列

> 状态：实施中 | 上游：[架构设计 02](02-架构设计.md) §4/§6、[进度.md](进度.md)
> 决策延续：核心能力下沉 `r-shell-core`（CLI/GUI 同源），bridge 薄封装，凭据只在 Rust 侧用。

## 1. 目标

在主窗口「文件」视图实现 FinalShell 式的 **SFTP 双栏文件管理**：

- **左：远程**（当前连接的 SFTP 文件系统）；**右：本地**（运行 GUI 机器的本地文件系统）。
- 列：名称 / 大小 / 权限 / 修改时间；目录在前、按名排序;可进入目录、返回上级、刷新。
- 传输：选中后用中间 ←/→ 按钮上传 / 下载；**支持把文件从系统拖入远程栏直接上传**。
- **传输队列**：每个任务显示方向 / 文件名 / 进度条 / 已传 / 总量 / 状态（进行中 / 完成 / 失败），可清除已完成。

平台：macOS 先跑通（与既有阶段一致），Windows/Linux 复用同代码。

## 2. 为什么用 SFTP `read_dir` 而非 `ls -la`

core 既有的 `list_directory` 走 `ls -la`（仅 Linux）。Stage 3.5 的真机是 **Windows**，`ls` 不可用。
本阶段新增**基于 SFTP 子系统 `read_dir` 的列目录**：

- 跨平台正确（Linux 与 Windows OpenSSH SFTP 都支持），返回结构化元数据（类型 / 大小 / 权限 / mtime），无需解析 `ls` 文本。
- 旧 `list_directory` / `upload_file` / `download_file` **原样保留**给 CLI（零回归）。

## 3. 分层设计

### 3.1 core（`core/src/ssh.rs` + `native_backend.rs`）

`ssh.rs` 新增（`SshClient` 上，复用已建立的会话）：

```rust
pub struct SftpEntry { name, is_dir, is_symlink, size: u64, permissions: String /*rwxr-xr-x*/, modified_unix: i64 }

async fn open_sftp(&self) -> Result<SftpSession>            // 私有：开 sftp 子系统
pub async fn sftp_read_dir(&self, path) -> Result<Vec<SftpEntry>>
pub async fn sftp_realpath(&self, path) -> Result<String>   // canonicalize（解析 . / ..，取 home）
pub async fn upload_file_progress<F: FnMut(u64,u64)+Send>(&self, local, remote, cb) -> Result<u64>
pub async fn download_file_progress<F: FnMut(u64,u64)+Send>(&self, remote, local, cb) -> Result<u64>
```

进度用**回调闭包** `FnMut(transferred, total)`，core 不依赖 frb（关注点分离）；分块 32KB。

`native_backend.rs` 新增（`NativeConnectionManager` 上，按 connection_id 查活跃会话）：

```rust
pub use crate::ssh::SftpEntry;
async fn client(&self, id) -> Result<Arc<RwLock<SshClient>>>  // 私有取连接（新代码用）
pub async fn sftp_list(&self, id, path) -> Result<Vec<SftpEntry>>
pub async fn sftp_realpath(&self, id, path) -> Result<String>
pub async fn sftp_upload<F>(&self, id, local, remote, cb) -> Result<u64>
pub async fn sftp_download<F>(&self, id, remote, local, cb) -> Result<u64>
```

### 3.2 bridge（`desktop/rust/src/api/`）

- `mod.rs`：把进程级 `manager()` 上提为 `pub(crate)`，新增 `pub(crate) async fn ensure_session(id)`（幂等开会话，供终端 / SFTP 共用同一个管理器——**关键**：SFTP 必须复用终端已开的会话）。`terminal::session_open` 改为调用它。
- `sftp.rs`（新）：

```rust
pub struct SftpEntryDto { name, kind: String /*dir|file|symlink*/, size: i64, permissions: String, modified_unix: i64 }
pub struct TransferProgress { transferred: i64, total: i64 }

pub async fn sftp_list(id, path) -> Result<Vec<SftpEntryDto>, String>
pub async fn sftp_home(id) -> Result<String, String>           // realpath(".")
pub async fn sftp_realpath(id, path) -> Result<String, String>
pub async fn sftp_upload(id, local, remote, sink: StreamSink<TransferProgress>) -> Result<(), String>
pub async fn sftp_download(id, remote, local, sink: StreamSink<TransferProgress>) -> Result<(), String>
```

`size` / `modified_unix` 用 **i64**（干净映射 Dart `int`）；时间 / 体积**在 Dart 侧格式化**。
进度**在 bridge 闭包里节流**（≥128KB 或完成才推一帧），避免大文件刷爆 stream。
上传 / 下载经 frb 转为 Dart `Stream<TransferProgress>`（与 `pty_subscribe` 同范式）：进度走 onData，失败走 onError，完成走 onDone。

### 3.3 Flutter（`desktop/lib/features/files/`，组件化）

- `sftp_repository.dart`：封装 frb sftp 调用（UI 不直接碰 frb）。
- `local_fs.dart`：`dart:io` 列本地目录 / 取 home（本地栏与连接无关）。
- `transfer_queue.dart`：`TransferQueue`（ChangeNotifier）+ `TransferItem`（方向 / 进度 / 状态）；订阅 stream 更新。
- `files_controller.dart`：`FilesController`（ChangeNotifier）持有远程路径 / 条目 / 本地路径 / 条目 / 选中态 + 转发上传下载到队列；连接切换即载入远程 home。
- `files_panel.dart`：双栏 UI（远程栏含 `DropTarget` 拖拽上传）+ 中间传输按钮 + 底部传输队列。

依赖：新增 `desktop_drop`（桌面文件拖入）。本地选取直接在本地栏浏览，无需额外 picker。

### 3.4 接入 `app_scaffold`

- 中心区按 `_navIndex` 切换：`1=终端`→`TerminalPanel`，`2=文件`→`FilesPanel`；其余暂仍终端。
- 连接树「⋯」菜单加「打开文件」→ 选中该连接 + 切到文件视图 + `FilesController.setConnection`。
- `FilesController` 与 `TerminalTabsController` 一样由 `_AppScaffoldState` 持有（跨视图切换不丢状态）。
- 状态栏在文件视图显示传输中数量 / 远程路径。

状态管理：延续轻量 `ChangeNotifier`（Riverpod 仍按既有技术债推迟）。

## 4. 关键决策

- **列目录改用 SFTP `read_dir`**：跨平台（含 Windows），结构化元数据；旧 `ls` 接口保留给 CLI（零回归）。
- **进度用回调闭包 + bridge 节流**：core 不依赖 frb；stream 不被大文件刷爆。
- **会话单例复用**：bridge 进程级 `manager()` 上提共享 + `ensure_session`，SFTP 与终端复用同一活跃 SSH 会话。
- **数值字段用 i64、格式化在 Dart**：避免 u64→Dart 映射歧义；体积 / 时间显示交给 UI。
- **本地栏用 `dart:io`**：与连接解耦，直接浏览选取；拖拽用 `desktop_drop`。

## 5. 验收 DoD

- `cargo build --workspace` 零警告、`cargo test` 全绿（含新增 SFTP 解析 / 映射单测）。
- `flutter analyze` 0 告警、`flutter build macos` ✓ Built。
- GUI：选连接→文件视图→看到远程 home 列表；进入 / 返回目录；本地栏浏览；←/→ 与拖拽触发传输并在队列看到进度与完成。
- 凭据不出现在 Dart 侧（沿用脱敏）。
