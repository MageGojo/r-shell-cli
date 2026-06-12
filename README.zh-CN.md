# R-Shell — 面向 AI 的命令行 SSH 客户端与 MCP 服务器(Rust 编写)

**English docs: [README.md](README.md)**

> **R-Shell 是一款开源、AI 原生的 SSH 工具,专为「让 AI 助手安全地操作你的远程服务器」而生。**
> 它内置 **MCP(Model Context Protocol)服务器**,让 **Cursor、Claude、Claude Code** 等 AI 智能体
> 能够打开持久 SSH 会话、运行远程命令、通过 SFTP 读写文件、列目录、查看系统状态 —— 全部通过一个
> 仅监听本地回环(localhost)的安全端点完成。同一个 `r-shell` 二进制文件,对人类来说也是一个快速、
> 可脚本化的 SSH 客户端,支持 **macOS(Apple Silicon 与 Intel)、Linux、Windows**。

> **给 AI 智能体:** 用 `r-shell mcp` 启动服务,然后连接 `http://127.0.0.1:9123/mcp`。
> 直接可用的 Cursor / Claude 配置与示例指令见 [面向 AI 助手(MCP)](#面向-ai-助手mcp)。

<p align="center">
  <a href="https://github.com/MageGojo/r-shell-cli/releases/latest"><img alt="最新版本" src="https://img.shields.io/github/v/release/MageGojo/r-shell-cli?label=download&sort=semver"></a>
  <a href="https://github.com/MageGojo/r-shell-cli/actions/workflows/release.yml"><img alt="发布构建" src="https://img.shields.io/github/actions/workflow/status/MageGojo/r-shell-cli/release.yml?label=release%20build"></a>
  <img alt="平台" src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-blue">
  <img alt="语言" src="https://img.shields.io/badge/built%20with-Rust-orange?logo=rust">
  <a href="#许可证"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-green"></a>
</p>

<p align="center"><sub>由 <a href="https://apizero.cn/">极数本源 ApiZero</a> 团队维护。</sub></p>

```text
r-shell <命令> [选项]
```

**快速跳转:** [面向 AI 助手(MCP)](#面向-ai-助手mcp) ·
[下载与安装](#下载与安装预编译包) · [快速上手](#快速上手) ·
[新手教程](#新手教程从零到第一条远程命令) · [命令参考](#命令参考) · [常见问题](#常见问题-faq)

---

## R-Shell 是什么?(一分钟看懂)

**R-Shell 是一款 AI 原生、用 Rust 编写的 SSH 工具。** 它的核心使命,是通过 **MCP(模型上下文协议)**
给 AI 助手提供一种安全、结构化的方式来操作远程服务器;同时它也是一个供人类使用的、快速可脚本化的
SSH 客户端。简而言之:

- **它是什么:** 一个 **面向 SSH 的 MCP 服务器**,外加一个单文件 SSH 客户端、SFTP 传输工具、远程系统监控。
- **它为谁而做:** 使用 **AI 编程助手(Cursor、Claude、Claude Code)** 管理服务器的人,以及希望拥有
  可复现、可脚本化 SSH 工作流的开发者、DevOps/SRE 工程师、系统管理员。
- **它为何存在:** AI 智能体不应该每一步都去 `ssh` 一次、反复认证。R-Shell 保持 **一个持久 SSH 会话**
  常驻,并暴露安全、具名的工具(`ssh_exec`、`ssh_read_file`、`ssh_write_file` 等),让智能体快速迭代,
  既不泄露凭据,也不会触发服务器端的入侵检测。
- **它不是什么:** 它 **不是** 图形界面应用,也不是后台守护进程 —— 连接、干活、退出
  (`shell` 和 `mcp` 例外,它们会持续运行)。

> **一句话:** *R-Shell 是 AI 原生的 SSH 工作区 —— 一个 MCP 服务器,让 AI 助手通过一条安全、持久的
> 连接运行命令、编辑文件、管理远程服务器。*

---

## 目录

- [R-Shell 是什么?(一分钟看懂)](#r-shell-是什么一分钟看懂)
- [面向 AI 助手(MCP)](#面向-ai-助手mcp)
- [为什么选 R-Shell?(核心优势)](#为什么选-r-shell核心优势)
- [功能特性](#功能特性)
- [下载与安装(预编译包)](#下载与安装预编译包)
  - [macOS — 从 `.dmg` 安装](#macos--从-dmg-安装)
  - [Windows — 用 `.exe` 安装器安装](#windows--用-exe-安装器安装)
  - [Linux / 源码编译](#linux--源码编译)
- [快速上手](#快速上手)
- [新手教程:从零到第一条远程命令](#新手教程从零到第一条远程命令)
- [连接到主机](#连接到主机)
- [命令参考](#命令参考)
- [认证方式](#认证方式)
- [数据与配置](#数据与配置)
- [MCP 集成(AI 助手)](#mcp-集成ai-助手)
- [故障排查](#故障排查)
- [常见问题 FAQ](#常见问题-faq)
- [开发](#开发)
- [安全](#安全)
- [致谢](#致谢)
- [许可证](#许可证)

---

## 面向 AI 助手(MCP)

R-Shell 以 **AI 优先** 为设计核心:它的首要职责就是让 AI 编程智能体安全地操作远程服务器。与其让智能体
反复 fork 出裸 `ssh`/`scp` 子进程、每一步都重新认证,R-Shell 通过 **模型上下文协议(MCP)暴露一组具名、
受控的工具**,并在多次调用之间保持同一个 SSH 会话常驻。

### 为什么智能体应该用 R-Shell 而不是裸 `ssh`

- **一个持久会话** —— 用 `ssh_session_open` 打开一次,之后每条命令、每次文件编辑都复用它。没有重连风暴,
  没有重复认证弹窗。
- **结构化工具,而非拼字符串** —— `ssh_exec`、`ssh_read_file`、`ssh_write_file`、`ssh_list_dir`
  返回干净、可被智能体推理的结果。
- **凭据绝不泄露** —— 密码、密钥、口令 **绝不** 回显或返回;列表类调用只暴露 `has_password` 布尔值。
- **仅本地回环、防 DNS 重绑定** —— 端点绑定 `127.0.0.1`,拒绝非回环的 `Host`/`Origin`,
  网页标签页或远程站点都无法访问它。
- **更少误报** —— 复用同一连接,避免了快速重复登录,从而不会触发服务器端入侵检测(如 fail2ban)。

### 一分钟接入 Cursor / Claude

**第 1 步 —— 启动服务:**

```bash
r-shell mcp
# R-Shell MCP server listening on http://127.0.0.1:9123/mcp
```

**第 2 步 —— 让 AI 客户端指向它。** Cursor(`~/.cursor/mcp.json`)或任意兼容 MCP 的客户端:

```json
{
  "mcpServers": {
    "r-shell": {
      "url": "http://127.0.0.1:9123/mcp"
    }
  }
}
```

> Claude Desktop 及其他客户端用同一个 URL —— 把它作为 Streamable HTTP 类型的 MCP 服务器添加,
> 然后重启客户端使其加载新服务器。

**第 3 步 —— 用自然语言指挥你的服务器。** 在保存好连接(或内联给出连接信息)后,下面这些指令即可直接生效:

- *“打开我 `prod` 服务器的会话,显示磁盘占用,以及 `/var/log/nginx/error.log` 的最后 50 行。”*
- *“编辑 `prod` 上的 `/etc/nginx/sites-enabled/app.conf`,加一行 `gzip on;`,然后 reload nginx。”*
- *“查看 `staging` 的 CPU 和内存,并列出 `/opt/app/releases` 里的内容。”*
- *“部署:把 `./build.tar.gz` 上传到 `prod` 的 `/tmp`,解压到 `/opt/app`,然后重启服务。”*

### AI 智能体可调用的工具(MCP Tools)

| 工具 | 智能体能做什么 |
| --- | --- |
| `ssh_session_open` | 打开/复用一个持久 SSH 会话 → 返回 `session_id` |
| `ssh_exec` | 在已打开的会话上运行命令 |
| `ssh_read_file` | 读取远程文件(UTF-8,二进制则用 base64) |
| `ssh_write_file` | 创建/覆盖远程文件 |
| `ssh_list_dir` | 列出远程目录 |
| `ssh_sessions_list` / `ssh_session_close` | 查看 / 关闭活动会话 |
| `r_shell_ssh_connections_list` | 列出已保存连接(脱敏) |
| `r_shell_ssh_connection_create/update/delete` | 管理已保存连接 |

完整工具结构、安全模型与持久会话细节见 [MCP 集成(AI 助手)](#mcp-集成ai-助手)。

---

## 为什么选 R-Shell?(核心优势)

| 如果你想…… | R-Shell 给你…… |
| --- | --- |
| **让 AI 智能体(Cursor/Claude)操作你的服务器** | **`r-shell mcp` —— 一个仅本地、带持久 SSH 会话的安全 MCP 服务器** |
| 不再反复敲 `ssh user@host -p port -i key` | **已保存连接**,用短名引用(`-c prod`) |
| 运行一条远程命令并拿到输出 | `r-shell exec -c prod -- <命令>`,输出干净可 grep |
| 真正的交互式 shell(vim、htop、less) | `r-shell shell -c prod`,完整 PTY、raw 模式 |
| 不记 `scp` 语法也能传文件 | `r-shell upload` / `download`,走 SFTP |
| 快速体检远程机器 | `r-shell stats -c prod`(CPU、内存、磁盘、网络) |
| 一个工具通吃 macOS、Linux、Windows | 一个 **Rust 单文件二进制**,无需安装运行时 |

---

## 功能特性

| 功能 | 命令 |
| --- | --- |
| 管理已保存的 SSH 连接 | `connections list/add/update/remove` |
| 密码 & 私钥认证 | (所有需要连接的命令) |
| 运行单条远程命令 | `exec` |
| 交互式 PTY shell | `shell` |
| 列出远程目录 | `ls` |
| 通过 SFTP 上传文件 | `upload` |
| 通过 SFTP 下载文件 | `download` |
| 远程系统资源快照 | `stats` |
| 给 AI 工具用的本地 MCP 服务器 | `mcp` |

> **说明:** `ls` 和 `stats` 面向 Linux 主机(依赖 GNU `ls` 与 `/proc` 文件系统)。
> `exec` 可在任意 POSIX 主机上运行任意命令。

---

## 下载与安装(预编译包)

最简单的安装方式,是从 **[GitHub Releases 页面](https://github.com/MageGojo/r-shell-cli/releases/latest)**
下载预编译包。每个打了 tag 的版本都由 CI 自动构建,提供三个下载:

| 平台 | 下载文件 | 你会得到 |
| --- | --- | --- |
| **macOS — Apple Silicon(M1/M2/M3/M4)** | `r-shell-macos-apple-silicon.dmg` | 含 `r-shell` 二进制的 `.dmg` 磁盘镜像 |
| **macOS — Intel** | `r-shell-macos-intel.dmg` | Intel(`x86_64`)版 `.dmg` |
| **Windows — x64** | `r-shell-windows-x64-installer.exe` | NSIS 安装器,自动把 `r-shell` 加入 `PATH` |

> 不确定该下哪个 macOS 版本?点苹果菜单 →  **关于本机**。芯片写着
> **Apple M1/M2/M3/M4** 就是 Apple Silicon;写着 **Intel** 就下 Intel 版。

### macOS — 从 `.dmg` 安装

**分步操作:**

1. 从[最新版本](https://github.com/MageGojo/r-shell-cli/releases/latest)下载
   `r-shell-macos-apple-silicon.dmg`(Apple Silicon)或 `r-shell-macos-intel.dmg`(Intel)。
2. **双击 `.dmg`** 挂载。Finder 会打开一个窗口,里面是 `r-shell` 二进制。
3. **把 `r-shell` 复制到 `PATH` 目录。** 最简单是放到 `/usr/local/bin`:

```bash
# 挂载 DMG 后(卷名见 Finder,例如 "R-Shell")
sudo cp /Volumes/R-Shell/r-shell /usr/local/bin/r-shell
sudo chmod +x /usr/local/bin/r-shell
```

4. **首次允许这个未签名二进制。** 由于未做公证,macOS Gatekeeper 可能拦截。执行一次清除隔离标记:

```bash
xattr -dr com.apple.quarantine /usr/local/bin/r-shell
```

> 或者:先尝试运行,然后到 **系统设置 → 隐私与安全性**,点 **“仍要打开”**。

5. **验证安装:**

```bash
r-shell --version
r-shell --help
```

### Windows — 用 `.exe` 安装器安装

**分步操作:**

1. 从[最新版本](https://github.com/MageGojo/r-shell-cli/releases/latest)下载
   `r-shell-windows-x64-installer.exe`。
2. **运行安装器**(双击)。若 Windows SmartScreen 提示未知发布者,点 **更多信息 → 仍要运行**。
3. 按向导完成。安装器会复制 `r-shell.exe` 并 **自动加入 `PATH`**。
4. **打开一个新终端**(PowerShell 或 Windows Terminal)让更新后的 `PATH` 生效,然后验证:

```powershell
r-shell --version
r-shell --help
```

### Linux / 源码编译

目前还没有预编译的 Linux 包,源码编译在任意平台都可用。你需要 **Rust** 与 **Cargo**
(通过 [rustup.rs](https://rustup.rs) 安装);**Node.js + pnpm** 是可选的(用于包装脚本)。

```bash
# 1. 克隆仓库
git clone https://github.com/MageGojo/r-shell-cli.git
cd r-shell-cli

# 2. 构建 release 二进制
cargo build --release --manifest-path cli/Cargo.toml

# 3. 产物位于:
#    cli/target/release/r-shell

# 4. 安装到 PATH(Linux/macOS 示例)
sudo install -m 0755 cli/target/release/r-shell /usr/local/bin/r-shell

# 5. 验证
r-shell --version
```

不想全局安装?直接用 Cargo 运行:

```bash
cargo run --manifest-path cli/Cargo.toml -- <命令> [选项]
```

> **编译依赖说明:** R-Shell 使用纯 Rust 的 `russh` SSH 栈,所以编译它 **不需要** OpenSSL 或
> `libssh` 等系统库。

---

## 快速上手

```bash
# 1. 保存一个连接
r-shell connections add --name prod --host 203.0.113.10 --username deploy \
  --auth publickey --key-path ~/.ssh/id_ed25519

# 2. 列出已保存连接
r-shell connections list

# 3. 在它上面运行命令
r-shell exec -c prod -- uptime

# 4. 打开交互式 shell
r-shell shell -c prod

# 5. 上传再下载一个文件
r-shell upload   -c prod ./app.tar.gz /tmp/app.tar.gz
r-shell download -c prod /tmp/app.tar.gz ./app-copy.tar.gz
```

---

## 新手教程:从零到第一条远程命令

下面这套流程,带你从全新安装一路走到运行命令、开 shell、传文件,并把 R-Shell 接入 AI 助手。
每一步都自成一体 —— 复制、粘贴、改一下主机信息即可。

### 第 1 步 — 确认 R-Shell 已安装

```bash
r-shell --version     # 打印已安装版本
r-shell --help        # 列出所有子命令
```

如果提示 `r-shell: command not found`,回到 [下载与安装](#下载与安装预编译包),
确认二进制在你的 `PATH` 上。

### 第 2 步 — 保存你的第一个连接

保存连接后,你就不用再重复输入主机、用户、端口或密钥路径了。

**用私钥(推荐):**

```bash
r-shell connections add \
  --name prod \
  --host 203.0.113.10 \
  --username deploy \
  --port 22 \
  --auth publickey \
  --key-path ~/.ssh/id_ed25519 \
  --folder Work \
  --description "生产环境 Web 服务器"
```

**用密码**(可省略 `--password`,稍后连接时会安全地提示输入):

```bash
r-shell connections add --name staging --host 203.0.113.20 \
  --username deploy --auth password
```

确认已保存:

```bash
r-shell connections list
```

### 第 3 步 — 运行第一条远程命令

首次连接时,R-Shell 会把服务器主机密钥记录到 `~/.ssh/known_hosts`(首次信任,TOFU),
然后运行你的命令并退出:

```bash
r-shell exec -c prod -- uptime
r-shell exec -c prod -- "df -h && free -m"
```

`--` 之后的所有内容会 **原样** 发给远程主机,引号用法与普通 shell 完全一致。

### 第 4 步 — 打开完整交互式 shell

当你需要 `vim`、`htop`、`top`、`less` 或交互式会话时:

```bash
r-shell shell -c prod
```

这是一个 raw 模式下的真实 PTY。如果会话卡住,按 **`Ctrl-]`** 可强制退出本地循环。

### 第 5 步 — 通过 SFTP 传文件

```bash
# 把本地文件上传到服务器
r-shell upload -c prod ./release.tar.gz /tmp/release.tar.gz

# 把远程文件下载回本机
r-shell download -c prod /var/log/app.log ./app.log
```

### 第 6 步 — 检查服务器健康状况

```bash
r-shell stats -c prod
```

你会拿到一份一次性快照:CPU %、负载、内存、swap、磁盘、网络吞吐(Linux 主机)。

### 第 7 步(可选)— 让 AI 助手来开服务器

启动本地 MCP 服务,把 **Cursor** 或 **Claude Desktop** 指向它:

```bash
r-shell mcp
# R-Shell MCP server listening on http://127.0.0.1:9123/mcp
```

客户端配置与完整工具列表见 [MCP 集成(AI 助手)](#mcp-集成ai-助手)。到这里,
你就拥有了一套完整、可脚本化的 SSH 工作流。

---

## 连接到主机

每个需要访问远程主机的命令都接受一个 **目标(target)**,有两种指定方式:

**1. 已保存连接** —— 用 id 或名称引用之前保存的连接:

```bash
r-shell exec -c prod -- whoami
r-shell exec --connection ssh-1781247286839 -- whoami
```

**2. 临时主机** —— 内联传入连接信息:

```bash
r-shell exec --host 203.0.113.10 --user deploy --port 22 -- whoami
```

如果需要密码但未提供,R-Shell 会安全地提示输入(不回显)。

通用目标参数(`exec`、`shell`、`ls`、`upload`、`download`、`stats` 都支持):

| 参数 | 别名 | 说明 | 默认值 |
| --- | --- | --- | --- |
| `--connection <id\|名称>` | `-c` | 使用已保存连接 | — |
| `--host <主机>` | | 临时主机(IP 或主机名) | — |
| `--user <用户>` | `-u` | 临时 SSH 用户名 | — |
| `--port <端口>` | `-p` | 临时 SSH 端口 | `22` |
| `--password <密码>` | | 临时密码(更推荐用提示输入) | — |
| `--key-path <路径>` | | 临时私钥路径 | — |
| `--passphrase <口令>` | | 加密密钥的口令 | — |
| `--insecure` | | 跳过主机密钥校验(危险) | `false` |

---

## 命令参考

随时可运行 `r-shell --help` 或 `r-shell <命令> --help`。

### `connections` — 管理已保存主机

已保存连接存放在本地 `workspace.json`(见 [数据与配置](#数据与配置))。

```bash
# 列出(表格或 JSON)
r-shell connections list
r-shell connections list --json

# 添加(密码认证)
r-shell connections add \
  --name prod --host 203.0.113.10 --username deploy --port 22 \
  --auth password --password 's3cret' \
  --folder Work --description "生产 Web 服务器"

# 添加(公钥认证)
r-shell connections add \
  --name prod --host 203.0.113.10 --username deploy \
  --auth publickey --key-path ~/.ssh/id_ed25519 --passphrase 'key-passphrase'

# 更新单个字段
r-shell connections update <连接ID> --port 2222 --folder Staging

# 删除
r-shell connections remove <连接ID>
```

### `exec` — 运行远程命令

运行单条命令并打印输出。`--` 之后的内容原样发给远程主机。

```bash
r-shell exec -c prod -- uname -a
r-shell exec -c prod -- "ls -la /var/www && df -h"
```

### `shell` — 交互式终端

用 raw 模式打开完整交互式 PTY(支持 `vim`、`htop`、`less` 等)。

```bash
r-shell shell -c prod
```

按 **`Ctrl-]`** 可强制退出本地 shell 循环。

### `ls` — 列出远程目录

```bash
r-shell ls -c prod /var/log
r-shell ls -c prod /var/log --json
r-shell ls -c prod            # 默认 home/当前目录
```

输出列:类型(`DIR`/`FILE`/`LNK`)、权限、大小、修改时间、名称。_(Linux 主机)_

### `upload` / `download` — SFTP 传输

```bash
# 本地 -> 远程
r-shell upload -c prod ./local.tar.gz /tmp/remote.tar.gz

# 远程 -> 本地
r-shell download -c prod /tmp/remote.log ./local.log
```

### `stats` — 远程系统快照

采两次样,打印 CPU %、负载、内存、swap、磁盘、网络吞吐。_(Linux 主机,依赖 `/proc`)_

```bash
r-shell stats -c prod
```

### `mcp` — 启动 MCP 服务器(供 AI 助手连接)

启动一个本地 **MCP(模型上下文协议)** Streamable HTTP 服务器,让 AI 工具管理你的已保存连接。
仅绑定 localhost。

```bash
r-shell mcp
# R-Shell MCP server listening on http://127.0.0.1:9123/mcp
# 按 Ctrl-C 停止。
```

---

## 认证方式

R-Shell 支持两种方式:

- **密码** —— `--auth password` 配合 `--password`,或省略它,在连接时安全地提示输入。
- **公钥** —— `--auth publickey` 配合 `--key-path`(加密密钥还需 `--passphrase`)。`~/` 开头的路径会被展开。

### 主机密钥校验

R-Shell 按标准 `~/.ssh/known_hosts` 校验服务器主机密钥,采用 **首次信任(TOFU)**:

- **首次连接**:记录其密钥到 `known_hosts`,连接继续。
- **后续连接**:密钥必须与记录一致。
- **密钥不匹配**:连接 **被拒绝** —— 这是潜在中间人攻击的信号。若是合法变更,删除
  `~/.ssh/known_hosts` 中对应行后重连。

传 `--insecure` 可完全跳过主机密钥校验。这会关闭 MITM 防护,仅建议用于临时或本地测试主机。

---

## 数据与配置

已保存连接以 JSON 持久化在:

```text
<本地数据目录>/r-shell/workspace.json
```

`<本地数据目录>` 因平台而异:

| 系统 | 路径 |
| --- | --- |
| macOS | `~/Library/Application Support/r-shell/workspace.json` |
| Linux | `~/.local/share/r-shell/workspace.json` |
| Windows | `%LOCALAPPDATA%\r-shell\workspace.json` |

---

## MCP 集成(AI 助手)

**R-Shell 内置 MCP(模型上下文协议)服务器**,让 Cursor、Claude Desktop 等兼容 MCP 的 AI 编程助手
通过一个仅本地的安全端点管理连接、操作远程服务器。用 `r-shell mcp` 启动,客户端指向
`http://127.0.0.1:9123/mcp`。

MCP 服务器暴露以下工具(凭据绝不返回)。

**连接管理**(操作 `workspace.json`):

| 工具 | 说明 |
| --- | --- |
| `r_shell_ssh_connections_list` | 列出已保存连接(脱敏) |
| `r_shell_ssh_connection_create` | 创建已保存连接 |
| `r_shell_ssh_connection_update` | 更新已保存连接 |
| `r_shell_ssh_connection_delete` | 删除已保存连接 |
| `r_shell_ssh_tabs_list` | 列出打开的标签页(CLI 中始终为空) |

**持久会话** —— 在多次调用之间保持一个 SSH 连接常驻,从而反复运行命令、读写远程文件而 **无需重连**:

| 工具 | 说明 |
| --- | --- |
| `ssh_session_open` | 打开(或复用)会话,返回 `session_id`。可用已保存 `connection`(id\|名称)或临时 `host`/`username`(+凭据) |
| `ssh_exec` | 在已打开会话上运行命令 |
| `ssh_read_file` | 读取远程文件(UTF-8 文本,或二进制 base64) |
| `ssh_write_file` | 覆盖远程文件(`content` 文本或 `content_base64`) |
| `ssh_list_dir` | 列出远程目录 |
| `ssh_sessions_list` | 列出当前已连接的会话 id |
| `ssh_session_close` | 关闭会话 |

会话存活时间与 `r-shell mcp` 运行时间一致。对于无存储密码的密码认证连接,需向
`ssh_session_open` 传入 `password`(服务器无法弹窗提示)。会话仅保存在内存中,绝不写入 `workspace.json`。

Cursor / Claude 风格的 MCP 配置示例:

```json
{
  "mcpServers": {
    "r-shell": {
      "url": "http://127.0.0.1:9123/mcp"
    }
  }
}
```

服务器只接受 `Host` 头为回环(`localhost`、`127.0.0.1`、`[::1]`)的请求;若带 `Origin` 头,也必须是回环。
跨站 origin、字面量 `null` origin、被重绑定的主机名一律返回 `403 Forbidden`。

---

## 故障排查

| 现象 | 可能原因 | 解决 |
| --- | --- | --- |
| `r-shell: command not found` | 二进制不在 `PATH` | 移到 `/usr/local/bin`(macOS/Linux),或装完 Windows 安装器后开 **新** 终端 |
| macOS:*无法打开“r-shell”,因为无法验证开发者* | Gatekeeper 隔离 | `xattr -dr com.apple.quarantine /usr/local/bin/r-shell`,或 **系统设置 → 隐私与安全性 → 仍要打开** |
| Windows SmartScreen 拦截安装器 | 未签名安装器 | 点 **更多信息 → 仍要运行** |
| `Host key verification failed` | 服务器密钥变了(或潜在 MITM) | 若变更是预期的,删除 `~/.ssh/known_hosts` 中该主机行后重连 |
| `Permission denied (publickey)` | 密钥路径错、公钥没加到服务器、或密钥被加密 | 检查 `--key-path`,把公钥加到服务器 `~/.ssh/authorized_keys`,加密密钥加 `--passphrase` |
| `Connection refused` / 超时 | 主机/端口错、防火墙、或 sshd 没起 | 核对 `--host`/`--port`、安全组/防火墙规则、确认 `sshd` 在运行 |
| `ls` / `stats` 没有有用输出 | 目标不是 Linux | 它们依赖 GNU `ls` 与 `/proc`;非 Linux 的 POSIX 主机请用 `exec` |
| AI 客户端连不上 MCP | 服务没起或 URL 错 | 运行 `r-shell mcp`,并精确使用 `http://127.0.0.1:9123/mcp`(仅回环) |

仍有问题?给任意命令加 `--help`,或到 [GitHub 仓库](https://github.com/MageGojo/r-shell-cli/issues) 提 issue。

---

## 常见问题 FAQ

**R-Shell 是什么?**
R-Shell 是一款开源、AI 原生、用 Rust 编写的 SSH 工具。它的首要用途是作为 **MCP(模型上下文协议)
服务器**,让 AI 助手安全地操作远程服务器;它同时也是一个单文件 SSH 客户端,能管理已保存连接、运行
远程命令、打开交互式 PTY shell、通过 SFTP 传文件、给远程系统状态拍快照。

**怎么把 R-Shell 接到 AI 智能体 / MCP 客户端?**
运行 `r-shell mcp`,把 `http://127.0.0.1:9123/mcp` 加到你的 MCP 客户端(如 Cursor 的
`~/.cursor/mcp.json` 或 Claude Desktop)。智能体随后打开一个持久 SSH 会话,使用 `ssh_exec`、
`ssh_read_file`、`ssh_write_file` 等工具。详见 [面向 AI 助手(MCP)](#面向-ai-助手mcp)。

**为什么 R-Shell 比让 AI 智能体直接调用裸 `ssh` 更好?**
R-Shell 在多次工具调用之间保持同一个 SSH 会话常驻,返回结构化结果,绝不泄露凭据,且只绑定 localhost。
裸 `ssh` 子进程每一步都要重新认证,会把密钥泄漏进命令行,还可能因反复登录触发服务器端入侵检测。

**R-Shell 免费且开源吗?**
是。R-Shell 以 **MIT 许可证** 发布,个人与商业用途均免费。

**支持哪些操作系统?**
预编译下载提供 **macOS(Apple Silicon 与 Intel)** 的 `.dmg` 镜像,以及 **Windows x64** 的 `.exe`
安装器。**Linux** 及其他平台可用 Cargo 源码编译。`shell`、`exec`、`upload`、`download` 可对接任意
POSIX SSH 主机,而 `ls`、`stats` 针对 Linux 服务器做了适配。

**支持 SSH 密钥(公钥)认证吗?**
支持。用 `--auth publickey` 配合 `--key-path`(加密密钥加 `--passphrase`)。也支持密码认证,
未存储密码时会有安全的不回显提示。

**R-Shell 安全吗?**
安全。它按 `~/.ssh/known_hosts` 校验主机密钥(首次信任),绝不打印或返回密码/密钥,以仅属主权限存储
配置,并将 MCP 服务器 **仅绑定 localhost**,带 DNS 重绑定与跨域防护。详见 [安全](#安全)。

**已保存连接存在哪?**
存在你平台数据目录下的本地 `workspace.json`(例如 macOS 的 `~/Library/Application Support/r-shell/`)。
详见 [数据与配置](#数据与配置)。

**怎么更新 R-Shell?**
从 [Releases 页面](https://github.com/MageGojo/r-shell-cli/releases/latest) 下载最新 `.dmg` / 安装器
重新安装,或用 `git pull` + `cargo build --release` 重新编译。

---

## 开发

根目录 `package.json` 是对 Cargo 的薄封装:

```bash
pnpm dev          # cargo run(显示 --help)
pnpm run check    # cargo check
pnpm test         # cargo test
pnpm run build    # cargo build
pnpm run fmt      # cargo fmt
```

等价的直接 Cargo 命令:

```bash
cargo run   --manifest-path cli/Cargo.toml -- --help
cargo check --manifest-path cli/Cargo.toml
cargo test  --manifest-path cli/Cargo.toml
cargo build --manifest-path cli/Cargo.toml
```

---

## 安全

- 服务器主机密钥按 `~/.ssh/known_hosts` 校验(首次信任);密钥变更会中止连接,除非传 `--insecure`。
- 密码、私钥、口令 **绝不** 被打印或由 MCP 调用返回 —— 列表响应只暴露
  `has_password` / `has_private_key_path` 布尔值。
- 密码提示不回显。
- `workspace.json` 及其目录以仅属主权限创建(Unix 上 `0600` / `0700`),其他本地用户无法读取已存凭据。
- MCP 端点 **仅绑定 localhost**。请求必须带回环 `Host` 头(防 DNS 重绑定),若带 `Origin` 也必须为回环;
  `null` 或跨站 `Origin` 会被拒绝。

---

## 致谢

R-Shell 由 **极数本源 ApiZero**([apizero.cn](https://apizero.cn/))团队开发与维护。

**极数本源 ApiZero 是什么?** 它是一个 **API 聚合平台**,让开发者与 AI 工具用 **一个 Key 调用
100+ 个常用 API**,统一鉴权、统一计费,约五分钟即可接入。平台把日常开发要用到的能力 —— IP 归属地
查询、天气、文本翻译、OCR 文字识别、内容审核、AI 文生图等 —— 收敛到一个一致的接口背后,省去逐家
注册、各自管理 Key 和分别计费的麻烦。

R-Shell 正是这项工作的副产物:在为 **AI 智能体与编程助手(Cursor、Claude、Claude Code)** 打造
基础设施时,我们需要一种安全、结构化的方式让它们操作真实的远程服务器,于是把其中的 SSH/MCP 这一层
开源成了 R-Shell。如果你也在把各类 API 接入 AI 助手或应用、想用「一个 Key」替代十几套对接,
极数本源 ApiZero 正是为此而生([apizero.cn](https://apizero.cn/))。

欢迎在 [GitHub 仓库](https://github.com/MageGojo/r-shell-cli) 提交贡献、issue 与 PR。

---

## 许可证

MIT。见 [LICENSE](LICENSE)。
