# iOS SSH · MCP `ssh_session_open` 卡死修复

日期：2026-08-06

## 现象

- GUI 里 iPhone 终端能连（`mobile@192.168.0.103`），监控也正常
- Cursor MCP `ssh_session_open(connection=iPhone, elevate=sudo, reconnect=true)` 挂几十秒～两分钟
- 交互里 `su -` 报 `su: failed to create session`

## 根因（只碰 iOS / 会话生命周期，不动其它平台协议）

1. **读锁死锁**：`execute_command_raw` 整段 `channel.wait()` 期间持有 `Backend` 的 `RwLock` 读锁；上一轮挂住的 exec/监控不释放时，`reconnect` → `insert_backend` 要写锁 → MCP 永久卡住（看起来像「Conch 卡死」）。
2. **exec/auth 无超时**：TCP connect 有 12s，但鉴权与 `exec` 通道无上限；iOS/USB 抖动时会一直等。
3. **`su` 非交互挂死**：越狱 iOS 上 `su` 常从 `/dev/tty` 要密码、忽略 stdin pipe；MCP 非 PTY exec 会无限等。`su -` 登录会话在 Dopamine/rootless 上还会直接 `failed to create session`。iOS 提权应走 `sudo -S`。

## 修复

| 文件 | 改动 |
|---|---|
| `core/src/ssh.rs` | 鉴权 15s / exec 45s 超时 |
| `core/src/native_backend.rs` | reconnect 先从 map 摘掉旧会话；disconnect 写锁 2s 超时，避免死等；iOS probe 8s |
| `core/src/ios_ssh.rs` | `platform:ios` + `elevate:su` → 改走 `sudo -S`（fail-fast，不再 pipe+su 挂死） |

## 验收

- `cargo test -p r-shell-core ios_ssh`
- MCP：`ssh_session_open` ≤15s 返回；`ssh_exec id` 在 elevate:sudo 下为 root
- 安装：`cargo build -p r-shell --release` → 覆盖 `~/.local/bin/r-shell`，重启 Cursor MCP
