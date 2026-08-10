# Stage 10 · iOS 越狱 SSH + MCP Cursor 兼容 + 开机自启

> 日期:2026-07-31  
> 目标:让 Conch 可靠连越狱 iOS(OpenSSH/dropbear · root/mobile · 按需提权)、修好 Cursor 连不上本机 MCP、设置里一键开机自启(App + MCP)。

## 1. 背景

越狱 iOS 的 SSH 与普通 Linux 主机有几处硬差异:

| 点 | 现象 | 处理 |
| --- | --- | --- |
| PATH | dropbear/OpenSSH 登录 shell 常只有 `/usr/bin:/bin`,越狱工具在 `/var/jb/...` | 远程命令前注入 jailbreak PATH |
| 用户 | 常用 `root` / `mobile`(默认口令多为 `alpine`,可改) | 连接预设 + UI 快捷选用户 |
| 提权 | `mobile` 写系统路径 / 装包会 Permission denied,需 `su`/`sudo` 到 root | 连接级 `elevate=su\|sudo`,用已存密码非交互提权 |
| 监控 | 无完整 Linux `/proc` | 已实现 Darwin 回退:`sysctl`/`vm_stat`/`df`(Python argv,避开 rootless 无 `/bin/sh`) |

USB 真机(`iproxy`)可转端口,但本轮探测时设备侧 **22/22222 无 SSH banner**(守护进程未起或未装 OpenSSH)。实现仍以「标准 TCP SSH」为准,局域网 / USB 转发均可。

## 2. MCP · Cursor 连不上

根因排查(按时间线):

1. **Accept**:Streamable HTTP 要求 `Accept` 同时含 `application/json` + `text/event-stream`;Cursor 常只发其一 → 已在 core 加 `normalize_mcp_accept` 中间件。
2. **Origin**:本机守卫拒绝非 loopback Origin。Cursor/VS Code 会发 `Origin: vscode-file://vscode-app` / `cursor://...` → **403**,表现为「连不上 MCP」。放行桌面 IDE 专用 scheme,仍要求 `Host` 为 loopback(防 DNS rebinding)。
3. **Private Network Access(2026-08)**:Cursor Shared MCP 走 Chromium 网络栈。从 `vscode-file://` / `cursor://` Origin 访问 `http://127.0.0.1:9123` 会先发 `OPTIONS` 预检并带 `Access-Control-Request-Private-Network: true`。服务端原先对 OPTIONS 回 **405**,Chromium 直接 `net::ERR_FAILED`。修复:中间件 `cursor_cors_pna`。
4. **stdio 兜底(2026-08)**:即便补了 PNA,部分 macOS / Cursor 版本对 loopback 仍持续 `net::ERR_FAILED`(本机网络权限等)。推荐 Cursor `mcp.json` 改用子进程 stdio,彻底绕开 Chromium HTTP:

```json
{
  "mcpServers": {
    "conch": {
      "command": "/Users/shcodegojo/.local/bin/r-shell",
      "args": ["mcp", "--stdio"]
    }
  }
}
```

`r-shell mcp --stdio` 与 HTTP 模式共用同一套工具实现。

## 3. 连接模型(标签约定,零 frb 大改)

不新增 frb 字段,复用已有 `tags`:

| tag | 含义 |
| --- | --- |
| `platform:ios` | 按 iOS 越狱配置包装命令(注入 PATH) |
| `elevate:su` | 非 iOS:经 `su root -c` 提权; **iOS 上改走 `sudo -S`**(Dopamine `su -` 常 `failed to create session`,且非交互 `su` 会挂死) |
| `elevate:sudo` | 经 `sudo -S` 提权(iOS 推荐) |

口令:提权复用该连接已保存的 `password`(越狱机 root/mobile 同密是常态)。GUI「iOS 越狱」预设会写入上述 tag,并默认用户 `root`、端口 `22`。

## 4. 开机自启

设置页新增:

- **登录时启动 Conch** → macOS Login Item(`launch_at_startup`)
- 打开时**自动勾选「随应用启动 MCP」**,保证开机后 MCP 也起来

## 5. 验收

- [ ] `cargo test -p r-shell-core`:iOS wrap / Origin 相关单测全绿
- [ ] 仅 `Accept: application/json` 与 `Origin: vscode-file://vscode-app` 的 initialize → 200
- [ ] GUI:连接弹窗可选 iOS 预设;设置可见开机自启开关
- [ ] 有可达越狱机时:root 直连 / mobile+`elevate:su` 能跑需 root 的命令
