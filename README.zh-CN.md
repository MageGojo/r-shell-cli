# Conch

Conch 是一个用 Rust 写的 SSH 客户端。一个 `r-shell` 二进制就能管理已保存的连接、
执行远程命令、打开交互式 shell、用 SFTP 传文件,以及打印一份系统状态快照。

它还内置一个可选的 MCP 服务器,让 AI 助手(Cursor、Claude 以及其他 MCP 客户端)
通过一条持久的 SSH 会话来操作服务器,而不必每一步都重新起一个 `ssh` 进程。

另外还有一个桌面应用 —— 基于同一套 Rust 核心、用 Flutter 做界面 —— 带标签页终端、
文件浏览器和实时监控。

English docs: [README.md](README.md)

<p align="center">
  <a href="https://github.com/MageGojo/conch/releases/latest"><img alt="最新版本" src="https://img.shields.io/github/v/release/MageGojo/conch?label=download&sort=semver"></a>
  <a href="https://github.com/MageGojo/conch/actions/workflows/release.yml"><img alt="发布构建" src="https://img.shields.io/github/actions/workflow/status/MageGojo/conch/release.yml?label=release%20build"></a>
  <img alt="平台" src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-blue">
  <img alt="语言" src="https://img.shields.io/badge/built%20with-Rust-orange?logo=rust">
  <a href="#许可证"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-green"></a>
</p>

## 截图

<p align="center">
  <img src="docs/gui/screenshots/conch-connections.png" alt="Conch 连接主页" width="900"><br>
  <em>连接主页 —— 已保存的主机按分组排列,并内置一张「本机监控」卡片。</em>
</p>

<p align="center">
  <img src="docs/gui/screenshots/conch-command-blocks.png" alt="Conch 命令块终端" width="900"><br>
  <em>命令块终端 —— 每条命令与它的输出各成一块,带退出码、耗时,工作目录在块之间持续保持。</em>
</p>

<p align="center">
  <img src="docs/gui/screenshots/conch-monitor.png" alt="Conch 实时监控" width="900"><br>
  <em>实时监控 —— CPU、内存、磁盘与网络,实时走势图。</em>
</p>

## 目录

- [截图](#截图)
- [功能](#功能)
- [安装](#安装)
- [快速上手](#快速上手)
- [命令](#命令)
- [MCP 服务器](#mcp-服务器)
- [认证](#认证)
- [配置](#配置)
- [安全](#安全)
- [开发](#开发)
- [项目结构](#项目结构)
- [许可证](#许可证)

## 功能

| 命令 | 说明 |
| --- | --- |
| `connections` | 增删改查已保存的主机 |
| `exec` | 在远程主机上执行单条命令 |
| `shell` | 打开交互式 PTY shell |
| `ls` | 列出远程目录 |
| `upload` / `download` | 通过 SFTP 传文件 |
| `stats` | 一次性的 CPU / 内存 / 磁盘 / 网络快照 |
| `mcp` | 启动本地 MCP 服务器 |

同一个二进制在 macOS、Linux、Windows 上都能跑。`exec`、`shell`、`upload`、
`download` 适用于任意 POSIX 主机;`ls` 和 `stats` 面向 Linux 主机(依赖 GNU `ls`
和 `/proc`)。

## 安装

### 桌面应用

预编译的桌面版在
[releases 页面](https://github.com/MageGojo/conch/releases/latest)。运行时已经打包好,
不用再装别的东西。

| 平台 | 文件 |
| --- | --- |
| Windows x64(安装器) | `Conch-windows-x64-setup.exe` |
| Windows x64(免安装) | `Conch-windows-x64.zip` |
| macOS | `Conch-macos.zip` |

目前还没做代码签名。macOS 首次打开时右键 `Conch.app` → **打开**;Windows 上如果
SmartScreen 拦截,点 **更多信息 → 仍要运行**。

### 命令行版(CLI)

| 平台 | 文件 |
| --- | --- |
| macOS(Apple Silicon) | `r-shell-macos-apple-silicon.dmg` |
| macOS(Intel) | `r-shell-macos-intel.dmg` |
| Windows x64 | `r-shell-windows-x64-installer.exe` |

macOS 上挂载 DMG,把二进制拷到 `PATH` 目录:

```bash
sudo cp /Volumes/Conch/r-shell /usr/local/bin/r-shell
sudo chmod +x /usr/local/bin/r-shell
xattr -dr com.apple.quarantine /usr/local/bin/r-shell   # 清除 Gatekeeper 隔离标记
r-shell --version
```

Windows 上运行安装器(它会把 `r-shell` 加进 `PATH`),然后开一个新终端。

### 源码编译

需要 Rust 和 Cargo([rustup.rs](https://rustup.rs))。不需要 OpenSSL 或 libssh ——
Conch 用的是纯 Rust 的 `russh`。

```bash
git clone https://github.com/MageGojo/conch.git
cd conch
cargo build --release --manifest-path cli/Cargo.toml
sudo install -m 0755 cli/target/release/r-shell /usr/local/bin/r-shell
```

不想安装,也可以直接用 Cargo 运行:

```bash
cargo run --manifest-path cli/Cargo.toml -- <命令> [选项]
```

## 快速上手

```bash
# 保存一个连接
r-shell connections add --name prod --host 203.0.113.10 --username deploy \
  --auth publickey --key-path ~/.ssh/id_ed25519

# 使用它
r-shell connections list
r-shell exec -c prod -- uptime
r-shell shell -c prod
r-shell upload   -c prod ./app.tar.gz /tmp/app.tar.gz
r-shell download -c prod /tmp/app.tar.gz ./app-copy.tar.gz
```

每个需要连主机的命令都接受一个目标:要么是已保存的连接(`-c <id|名称>`),要么直接
内联给出连接信息。如果需要密码但没提供,会提示你输入(不回显)。

通用目标参数(`exec`、`shell`、`ls`、`upload`、`download`、`stats` 都支持):

| 参数 | 别名 | 说明 | 默认值 |
| --- | --- | --- | --- |
| `--connection <id\|名称>` | `-c` | 使用已保存的连接 | — |
| `--host <主机>` | | 临时主机(IP 或主机名) | — |
| `--user <用户>` | `-u` | 临时 SSH 用户名 | — |
| `--port <端口>` | `-p` | 临时 SSH 端口 | `22` |
| `--password <密码>` | | 临时密码(更推荐用提示输入) | — |
| `--key-path <路径>` | | 私钥路径 | — |
| `--passphrase <口令>` | | 加密密钥的口令 | — |
| `--insecure` | | 跳过主机密钥校验 | `false` |

## 命令

任何时候都可以用 `r-shell --help` 或 `r-shell <命令> --help` 查看完整选项。

### connections

已保存的连接存放在本地 `workspace.json`(见 [配置](#配置))。

```bash
r-shell connections list [--json]

r-shell connections add \
  --name prod --host 203.0.113.10 --username deploy --port 22 \
  --auth publickey --key-path ~/.ssh/id_ed25519 \
  --folder Work --description "生产环境 Web 服务器"

r-shell connections update <id> --port 2222 --folder Staging
r-shell connections remove <id>
```

必填:`--name`、`--host`、`--username`。`--auth` 取 `password` 或 `publickey`
(默认 `password`);`--port` 默认 `22`;`--folder` 默认 `All Connections`。
`update` 接受一个连接 id,再加上要修改的字段。

### exec

执行单条命令并打印输出。`--` 之后的内容原样发给主机。

```bash
r-shell exec -c prod -- uname -a
r-shell exec -c prod -- "ls -la /var/www && df -h"
r-shell exec --host 203.0.113.10 --user deploy -- systemctl status nginx
```

### shell

raw 模式下的完整交互式 PTY(支持 vim、htop、less 等)。会话卡住时按 `Ctrl-]`
可强制退出本地循环。

```bash
r-shell shell -c prod
```

### ls

```bash
r-shell ls -c prod /var/log
r-shell ls -c prod /var/log --json
r-shell ls -c prod              # 默认 home 目录
```

输出列:类型(`DIR`/`FILE`/`LNK`)、权限、大小、修改时间、名称。(Linux 主机)

### upload / download

通过 SFTP 做单文件传输。

```bash
r-shell upload   -c prod ./local.tar.gz /tmp/remote.tar.gz
r-shell download -c prod /tmp/remote.log ./local.log
```

### stats

采两次样,打印一份快照(Linux 主机,依赖 `/proc`)。

```bash
r-shell stats -c prod
```

```text
OS:      Linux 6.1.0
Uptime:  12d 4h 31m
CPU:     7.4%  (8 cores, load 0.42)
Memory:  61.2%  (4.9/7.8 GB)
Disk:    40.0%  (3.8/9.5 GB)
Network: down 1.5 KB/s  up 320 B/s
```

### mcp

启动本地 MCP 服务器(见下文)。

```bash
r-shell mcp
# Conch MCP server listening on http://127.0.0.1:9123/mcp
```

## MCP 服务器

`r-shell mcp` 会启动一个本地的 Model Context Protocol 服务器,走 Streamable HTTP,
只绑定 `127.0.0.1`。把 MCP 客户端指向 `http://127.0.0.1:9123/mcp`:

```json
{
  "mcpServers": {
    "r-shell": {
      "url": "http://127.0.0.1:9123/mcp"
    }
  }
}
```

Cursor 写到 `~/.cursor/mcp.json`;Claude Desktop 等客户端把同一个 URL 作为
Streamable HTTP 服务器添加,然后重启。

服务器会在多次调用之间保持一条 SSH 会话常驻,这样助手就能反复执行命令、读写文件,
而不用每次重连。会话只存在内存里,生命周期与 `r-shell mcp` 的运行时间一致。

会话相关工具:

| 工具 | 说明 |
| --- | --- |
| `ssh_session_open` | 打开或复用会话,返回 `session_id` |
| `ssh_exec` | 在会话上执行命令 |
| `ssh_read_file` | 读取远程文件(UTF-8,二进制用 base64) |
| `ssh_write_file` | 创建或覆盖远程文件 |
| `ssh_list_dir` | 列出远程目录 |
| `ssh_sessions_list` / `ssh_session_close` | 列出或关闭活动会话 |

连接管理工具(操作 `workspace.json`):

| 工具 | 说明 |
| --- | --- |
| `r_shell_ssh_connections_list` | 列出已保存连接(已去除凭据) |
| `r_shell_ssh_connection_create` / `_update` / `_delete` | 管理已保存连接 |

凭据绝不返回 —— 列表调用只暴露 `has_password` 这类布尔值。端点要求 `Host` 头是回环地址
(若带 `Origin`,也必须是回环);跨域请求和 DNS 重绑定请求一律返回 `403`。

## 认证

- **密码** —— `--auth password` 配 `--password`,或省略它,在连接时提示输入。
- **公钥** —— `--auth publickey` 配 `--key-path`(加密密钥再加 `--passphrase`)。
  `~/` 开头的路径会被展开。

主机密钥按 `~/.ssh/known_hosts` 校验,采用首次信任(TOFU):首次连接记录密钥,后续连接
必须一致。不一致会中止连接(可能是中间人攻击),需要先删掉 `known_hosts` 里对应那行。
`--insecure` 会完全跳过这项检查 —— 只建议用于临时或本地测试主机。

## 配置

已保存的连接以 JSON 形式存放:

| 系统 | 路径 |
| --- | --- |
| macOS | `~/Library/Application Support/r-shell/workspace.json` |
| Linux | `~/.local/share/r-shell/workspace.json` |
| Windows | `%LOCALAPPDATA%\r-shell\workspace.json` |

在 Unix 上,这个文件及其目录以仅属主权限创建(`0600` / `0700`)。

## 安全

- 主机密钥按 `known_hosts` 校验(首次信任);密钥变更会中止连接,除非传 `--insecure`。
- 密码、私钥、口令绝不被打印,也不会由 MCP 调用返回 —— 列表响应只暴露
  `has_password` / `has_private_key_path`。
- 密码提示不回显。
- `workspace.json` 在 Unix 上仅属主可读写。
- MCP 服务器只绑定 localhost,拒绝非回环的 `Host`/`Origin`(防 DNS 重绑定与跨域访问)。

## 开发

根目录的 `package.json` 是对 Cargo 的薄封装:

```bash
pnpm dev      # cargo run -- --help
pnpm check    # cargo check
pnpm test     # cargo test
pnpm build    # cargo build
pnpm fmt      # cargo fmt
```

也可以直接用 Cargo,加上 `--manifest-path cli/Cargo.toml`。版本号通过
`pnpm version:patch|minor|major` 升级,会同步更新 `package.json`、`cli/Cargo.toml`、
`cli/Cargo.lock` 和 `CHANGELOG.md`。

## 项目结构

```text
conch/
├── cli/         # r-shell:CLI + MCP 服务器(Rust)
├── core/        # 共享的 SSH / MCP 核心库
├── desktop/     # 桌面应用(Flutter 界面 + Rust 核心)
├── packaging/   # macOS .dmg 与 Windows 安装器脚本
├── scripts/     # 版本号升级脚本
└── .github/     # CI:测试与发布构建
```

## 许可证

MIT,见 [LICENSE](LICENSE)。由 [极数本源 ApiZero](https://apizero.cn/) 团队维护,
欢迎在 [GitHub](https://github.com/MageGojo/conch) 提 issue 和 PR。
