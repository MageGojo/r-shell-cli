# 连接实时刷新 + MCP 安卓/iOS OpenSSH 预设

> 日期:2026-07-31  
> 目标:MCP/CLI 改 `workspace.json` 后 GUI 立刻看见;Agent 用 MCP 一键建越狱 iOS / 安卓 dropbear 连接,勿再手改 JSON。

## 1. GUI 实时刷新

`AppScaffold` 每 **2s** 轮询 `list_connections()`。连接签名(`id|name|host|port|user|protocol|tags`)变化才 `setState`,并 `pruneMissing` 清掉已删连接的命令块标签。

约定:**增删改连接一律走 MCP / GUI / CLI**,不要直接写 `workspace.json`(容易和运行中的 GUI 脱节;历史事故已有)。

## 2. MCP `r_shell_ssh_connection_create` 设备预设

| 参数 | 含义 |
| --- | --- |
| `platform` | `ios` / `iphone` → 打 `platform:ios`、默认 `root`/`alpine`/22、文件夹 `iOS`;`android` → 打 `platform:android`(OpenSSH/dropbear)、默认端口常 8022 若仍为 22 且未改 |
| `elevate` | `su` / `sudo` → 写入 `elevate:…` 标签(mobile→root) |
| `protocol` | `SSH`(默认)或 `ADB` |

`name` / `username` / `auth_method` 可省略:由预设兜底。Agent 应优先调此工具,勿直接改磁盘上的 workspace。

### 示例

```json
// 越狱 iPhone(只填主机即可)
{ "host": "192.168.0.103", "platform": "ios", "password": "123456", "elevate": "sudo" }

// 安卓 dropbear OpenSSH
{ "name": "抖音设备", "host": "192.168.0.107", "port": 8022, "username": "root",
  "platform": "android", "auth_method": "publickey",
  "private_key_path": "/path/to/id_ed25519" }
```

## 3. 会话侧

`ssh_session_open` 已支持 `platform=ios` / `elevate=su|sudo`;连上后会自动嗅探 Darwin/`/var/jb` 并注入 PATH。
