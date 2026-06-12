# 用 Rust 打造命令行 SSH 工具 R-Shell：连接管理、远程执行、SFTP 传输与 MCP 一体化实战

> 关键词：Rust 命令行工具、SSH 客户端、CLI、SFTP、russh、MCP、known_hosts、终端运维
>
> 项目开源地址：[https://github.com/MageGojo/r-shell-cli](https://github.com/MageGojo/r-shell-cli)

## 摘要

R-Shell 是一个用 **Rust** 编写的命令行 **SSH 工作台**，把日常服务器运维里最高频的几件事——**保存 SSH 连接、执行远程命令、打开交互式终端、SFTP 上传下载、查看远程系统资源**——收敛到一个名为 `r-shell` 的可执行文件里，并内置一个本地 **MCP（Model Context Protocol）服务器**，方便 AI 工具直接管理你的连接。本文从工程视角完整介绍它的能力、命令用法、架构设计与安全加固（主机密钥校验、凭据文件权限、MCP 访问控制），并给出可直接复制的实战示例。

读完本文你将了解：

- R-Shell 提供哪些命令，分别解决什么问题；
- 如何用一条命令完成远程执行、文件传输、系统监控；
- 它在 SSH 安全上做了哪些工程实践（这部分对运维很关键）；
- 如何把它接入支持 MCP 的 AI 客户端。

---

## 目录

- [一、R-Shell 是什么](#一r-shell-是什么)
- [二、为什么用 Rust 写一个命令行 SSH 工具](#二为什么用-rust-写一个命令行-ssh-工具)
- [三、整体架构](#三整体架构)
- [四、安装与构建](#四安装与构建)
- [五、连接到远程主机的两种方式](#五连接到远程主机的两种方式)
- [六、命令详解与实战示例](#六命令详解与实战示例)
  - [6.1 connections：管理保存的连接](#61-connections管理保存的连接)
  - [6.2 exec：执行远程命令](#62-exec执行远程命令)
  - [6.3 shell：交互式终端](#63-shell交互式终端)
  - [6.4 ls：列出远程目录](#64-ls列出远程目录)
  - [6.5 upload / download：SFTP 文件传输](#65-upload--downloadsftp-文件传输)
  - [6.6 stats：远程系统资源快照](#66-stats远程系统资源快照)
  - [6.7 mcp：启动本地 MCP 服务器](#67-mcp启动本地-mcp-服务器)
- [七、安全设计：一个 SSH 工具应该做对的事](#七安全设计一个-ssh-工具应该做对的事)
- [八、把 R-Shell 接入 AI 工具（MCP）](#八把-r-shell-接入-ai-工具mcp)
- [九、常见问题 FAQ](#九常见问题-faq)
- [十、总结](#十总结)

---

## 一、R-Shell 是什么

R-Shell 是一个**单文件、可脚本化的命令行 SSH 工作台**。它不是一个图形界面应用，而是一个像 `git`、`docker` 那样用子命令组织功能的 CLI 工具。

它的核心能力可以用一张表概括：

| 能力 | 对应命令 |
| --- | --- |
| 管理保存的 SSH 连接（增删改查） | `connections list/add/update/remove` |
| 密码与私钥两种认证 | 所有连接类命令 |
| 执行单条远程命令 | `exec` |
| 打开交互式 PTY 终端 | `shell` |
| 列出远程目录 | `ls` |
| SFTP 上传 / 下载文件 | `upload` / `download` |
| 远程系统资源快照（CPU/内存/磁盘/网络） | `stats` |
| 本地 MCP 服务器（供 AI 工具调用） | `mcp` |

设计理念很简单：**一个子命令只做一件事，输出尽量是可被 `grep`/`jq` 处理的纯文本**，需要结构化时再提供 `--json`。

---

## 二、为什么用 Rust 写一个命令行 SSH 工具

选择 Rust 来实现这样一个工具，主要基于三点工程考量：

1. **单二进制、零运行时依赖**。`cargo build --release` 产出一个静态性较好的可执行文件，拷到服务器上即可运行，不用装 Node、Python 或一堆动态库。
2. **内存安全 + 异步生态成熟**。SSH/SFTP 是典型的 I/O 密集场景，配合 `tokio` 异步运行时和 `russh` 这一纯 Rust 的 SSH 实现，可以在不写 `unsafe` 的前提下获得不错的性能与稳定性。
3. **类型系统帮你把错误处理写对**。用 `anyhow::Result` 贯穿所有可能失败的操作，连接超时、认证失败、命令非零退出都能转换为清晰的错误信息，而不是莫名其妙的 panic。

依赖栈一览（节选）：

```toml
[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
crossterm = "0.28"
russh = "0.43"
russh-keys = "0.43"
russh-sftp = "2"
tokio = { version = "1", features = ["full"] }
rmcp = { version = "1.7", features = ["server", "transport-streamable-http-server"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- `clap`：命令行参数解析（derive 宏，声明式定义子命令）；
- `russh` / `russh-keys` / `russh-sftp`：SSH 协议、密钥处理、SFTP 子系统；
- `crossterm`：交互式终端的 raw mode 与无回显密码输入；
- `rmcp`：MCP 服务器实现（Streamable HTTP）。

---

## 三、整体架构

R-Shell 的代码分层非常清晰：**最上层是 CLI（基于 clap）**，每个子命令映射到一个处理函数；中间是**与界面无关的核心模块**；底层是**持久化与 MCP 服务**。

![R-Shell 架构图](https://raw.githubusercontent.com/MageGojo/r-shell-cli/main/docs/images/rshell-cli-architecture.png)

各模块职责：

| 模块 | 文件 | 职责 |
| --- | --- | --- |
| CLI 入口 | `cli/src/main.rs` | 参数解析、命令分发、输出格式化 |
| 连接管理器 | `cli/src/native_backend.rs` | 维护连接池，封装执行/传输/目录等操作 |
| SSH 实现 | `cli/src/ssh.rs` | SSH 握手、命令执行、PTY、SFTP |
| 系统监控 | `cli/src/monitor.rs` | 解析远程 `/proc`、`df`，计算 CPU/网络速率 |
| 数据模型 | `cli/src/model.rs` | 连接与工作区的数据结构 |
| 本地持久化 | `cli/src/storage.rs` | 读写 `workspace.json` |
| MCP 服务器 | `cli/src/mcp.rs` | 本地 Streamable HTTP MCP 服务 |

这种分层的好处是：**业务逻辑完全不依赖任何界面框架**，既能被 CLI 复用，也能被 MCP 服务复用，未来要换成别的入口也很容易。

---

## 四、安装与构建

环境要求：安装 **Rust 与 Cargo** 即可。

```bash
# 克隆仓库
git clone https://github.com/MageGojo/r-shell-cli.git
cd r-shell-cli

# 构建 release 二进制
cargo build --release --manifest-path cli/Cargo.toml

# 产物路径：
#   cli/target/release/r-shell
```

把 `cli/target/release/r-shell` 拷贝到 `PATH` 目录（如 `/usr/local/bin`）后，就能直接使用 `r-shell` 命令。下文示例默认它已在 `PATH` 中。

查看帮助：

```bash
r-shell --help
r-shell <command> --help   # 查看某个子命令的全部参数
```

---

## 五、连接到远程主机的两种方式

几乎所有需要联网的命令都接受一个**目标（target）**，有两种指定方式：

**方式一：引用已保存的连接**（推荐日常使用）

```bash
r-shell exec -c prod -- whoami        # 按名称引用
r-shell exec --connection ssh-123 -- whoami   # 按 id 引用
```

**方式二：临时（ad-hoc）指定主机**

```bash
r-shell exec --host 203.0.113.10 --user deploy --port 22 -- whoami
```

如果某个连接需要密码但你没有提供，R-Shell 会在连接时**安全地提示输入**（输入不回显）。

常用目标参数：

| 参数 | 简写 | 说明 | 默认值 |
| --- | --- | --- | --- |
| `--connection <id\|name>` | `-c` | 使用已保存的连接 | — |
| `--host <host>` | | 临时主机（IP 或域名） | — |
| `--user <user>` | `-u` | 临时用户名 | — |
| `--port <port>` | `-p` | 端口 | `22` |
| `--key-path <path>` | | 私钥路径 | — |
| `--passphrase <pp>` | | 私钥口令 | — |
| `--insecure` | | 跳过主机密钥校验（危险） | `false` |

---

## 六、命令详解与实战示例

### 6.1 connections：管理保存的连接

保存的连接以 JSON 形式存储在本地工作区文件中。

```bash
# 列表（表格 / JSON）
r-shell connections list
r-shell connections list --json

# 新增（密码认证）
r-shell connections add \
  --name prod \
  --host 203.0.113.10 \
  --username deploy \
  --port 22 \
  --auth password \
  --folder Work \
  --description "生产 Web 服务器"

# 新增（公钥认证）
r-shell connections add \
  --name prod \
  --host 203.0.113.10 \
  --username deploy \
  --auth publickey \
  --key-path ~/.ssh/id_ed25519

# 更新指定字段
r-shell connections update <connection_id> --port 2222 --folder Staging

# 删除
r-shell connections remove <connection_id>
```

`connections list --json` 的输出是脱敏的——**只会告诉你某连接有没有密码（`has_password`），绝不会回显明文凭据**：

```json
[
  {
    "connection_id": "ssh-1781247286839",
    "name": "prod",
    "host": "203.0.113.10",
    "username": "deploy",
    "port": 22,
    "auth_method": "publickey",
    "folder": "Work",
    "has_password": false,
    "has_private_key_path": true,
    "status": "Disconnected"
  }
]
```

### 6.2 exec：执行远程命令

在远程主机上执行一条命令并打印输出，`--` 后面的内容会原样发送到远端。

```bash
r-shell exec -c prod -- uname -a
r-shell exec -c prod -- "ls -la /var/www && df -h"
r-shell exec --host 203.0.113.10 --user deploy -- systemctl status nginx
```

### 6.3 shell：交互式终端

打开一个完整的交互式 PTY 终端，支持 `vim`、`htop`、`less` 等需要伪终端的程序。底层用 crossterm 的 raw mode 把本地按键透传到远端。

```bash
r-shell shell -c prod
```

按 **`Ctrl-]`** 可以强制退出本地的 shell 循环。

### 6.4 ls：列出远程目录

```bash
r-shell ls -c prod /var/log
r-shell ls -c prod /var/log --json
r-shell ls -c prod              # 默认列当前/家目录
```

输出列为：类型（`DIR`/`FILE`/`LNK`）、权限、大小、修改时间、名称。注意 `ls` 与 `stats` 面向 Linux 主机（依赖 GNU `ls` 与 `/proc`）。

### 6.5 upload / download：SFTP 文件传输

基于 SFTP 子系统的单文件传输。

```bash
# 本地 -> 远程
r-shell upload -c prod ./app.tar.gz /tmp/app.tar.gz

# 远程 -> 本地
r-shell download -c prod /tmp/remote.log ./local.log
```

传输完成后会打印传输字节数，例如：

```text
Uploaded ./app.tar.gz -> /tmp/app.tar.gz (1.0 KB)
```

### 6.6 stats：远程系统资源快照

连续采样两次，计算出 CPU 使用率、负载、内存、交换分区、磁盘占用与网络吞吐。

```bash
r-shell stats -c prod
```

示例输出：

```text
OS:      Linux 6.1.0
Uptime:  12d 4h 31m
CPU:     7.4%  (8 cores, load 0.42)
Memory:  61.2%  (4.9/7.8 GB)
Disk:    40.0%  (3.8/9.5 GB)
Network: down 1.5 KB/s  up 320 B/s
```

它的实现思路是：用一条命令一次性把 `/proc/stat`、`/proc/meminfo`、`/proc/net/dev`、`df` 等信息抓回来，再在本地解析。CPU 利用率和网络速率属于"速率量"，需要两次采样做差值才能算出来。

### 6.7 mcp：启动本地 MCP 服务器

启动一个本地的 MCP（Model Context Protocol）Streamable HTTP 服务器，让支持 MCP 的 AI 工具可以管理你保存的 SSH 连接。仅绑定到 localhost。

```bash
r-shell mcp
# R-Shell MCP server listening on http://127.0.0.1:9123/mcp
# Press Ctrl-C to stop.
```

---

## 七、安全设计：一个 SSH 工具应该做对的事

SSH 工具最容易被忽视、却最关键的就是安全。R-Shell 在这几方面做了明确的工程实践：

### 7.1 主机密钥校验（防中间人攻击）

R-Shell 会按照标准的 `~/.ssh/known_hosts` 做**首次信任（TOFU，Trust-On-First-Use）**校验：

- **首次连接**某主机：把它的主机密钥记录进 `known_hosts`，并提示 `permanently added`；
- **后续连接**：密钥必须与记录一致；
- **密钥不匹配**：直接**拒绝连接**——这通常意味着可能的中间人攻击。

```text
[ssh] REMOTE HOST IDENTIFICATION HAS CHANGED for 203.0.113.10:22!
[ssh] The host key does not match the one in known_hosts ...
```

如果确认是合法的密钥变更（比如服务器重装），删除 `~/.ssh/known_hosts` 里对应的那一行再重连即可。明知风险的测试场景可以用 `--insecure` 跳过校验。

### 7.2 凭据文件权限收紧

存放连接的 `workspace.json` 及其目录在写入时会被设置为**仅属主可访问**（Unix 下 `0600` / `0700`），避免同机器上的其他用户读到你的连接配置。

### 7.3 MCP 端点访问控制

MCP 服务只绑定 `127.0.0.1`，并且：

- 请求的 `Host` 头必须是回环地址（`127.0.0.1` / `localhost` / `[::1]`），以此**抵御 DNS rebinding**（恶意域名解析到本地的攻击）；
- 如果带了 `Origin` 头，必须是回环来源；**跨站来源和 `null` 来源一律拒绝**（返回 `403`）；
- MCP 的列表接口对凭据脱敏，**不返回任何明文密码或私钥**。

这些细节看似琐碎，但正是区分"能用"和"可放心用"的地方。

---

## 八、把 R-Shell 接入 AI 工具（MCP）

MCP 服务器暴露的工具如下（凭据均脱敏）：

| 工具 | 说明 |
| --- | --- |
| `r_shell_ssh_connections_list` | 列出保存的连接（脱敏） |
| `r_shell_ssh_connection_create` | 创建保存的连接 |
| `r_shell_ssh_connection_update` | 更新保存的连接 |
| `r_shell_ssh_connection_delete` | 删除保存的连接 |
| `r_shell_ssh_tabs_list` | 列出打开的标签（CLI 下恒为空） |

先用 `r-shell mcp` 启动服务，再把支持 MCP 的客户端指向 `http://127.0.0.1:9123/mcp`。一个典型的 MCP 客户端配置示例：

```json
{
  "mcpServers": {
    "r-shell": {
      "url": "http://127.0.0.1:9123/mcp"
    }
  }
}
```

这样你就能让 AI 助手帮你"列出我所有的 SSH 连接""新建一个连接到测试机"等等，而所有敏感凭据都不会通过 MCP 泄露出去。

---

## 九、常见问题 FAQ

**Q1：`ls` 或 `stats` 在某些主机上报错怎么办？**
A：这两个命令面向 Linux 服务器，依赖 GNU `ls --time-style=long-iso` 和 `/proc` 文件系统。在 macOS/BSD 等主机上可能不可用，但 `exec` 可以执行任意命令，不受此限制。

**Q2：连接保存在哪里？**
A：保存在本地数据目录的 `r-shell/workspace.json`。macOS 在 `~/Library/Application Support/`，Linux 在 `~/.local/share/`，Windows 在 `%LOCALAPPDATA%`。

**Q3：密码会明文存储吗？**
A：如果你用 `--password` 保存密码，它会写入 `workspace.json`，但该文件已被设置为仅属主可读（0600）。更推荐使用公钥认证，或不保存密码、连接时再交互输入。

**Q4：第一次连接提示 `permanently added` 是正常的吗？**
A：正常。这是 TOFU 首次信任机制在记录主机密钥，和 OpenSSH 第一次连接新主机时的行为一致。

---

## 十、总结

R-Shell 把"保存连接、远程执行、交互式终端、SFTP 传输、系统监控、MCP 服务"这些运维高频操作收敛进一个用 **Rust** 写的命令行工具里，具备以下特点：

- **单二进制、可脚本化**，子命令职责清晰，输出对管道友好；
- **核心逻辑与界面解耦**，CLI 与 MCP 服务共享同一套后端；
- **安全上做了正确的事**：主机密钥 TOFU 校验、凭据文件权限收紧、MCP 双重访问控制（Host + Origin）与凭据脱敏。

如果你日常需要在多台服务器之间跑命令、传文件、看负载，又想要一个轻量、安全、可被脚本和 AI 工具调用的 SSH 工具，欢迎试用并 Star：

👉 项目地址：[https://github.com/MageGojo/r-shell-cli](https://github.com/MageGojo/r-shell-cli)

> 如果这篇文章对你有帮助，欢迎点赞、收藏、评论交流。后续会继续分享 Rust 命令行工具与运维自动化相关的实战内容。
