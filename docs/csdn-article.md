# 用 Rust 打造命令行 SSH 工具 R-Shell：连接管理、远程执行、SFTP 传输与 MCP 一体化实战教程

> **关键词**：Rust 命令行工具、SSH 客户端、CLI 开发、SFTP、russh、tokio、clap、MCP、known_hosts 主机密钥校验、终端运维自动化
>
> **项目开源地址**：[https://github.com/MageGojo/r-shell-cli](https://github.com/MageGojo/r-shell-cli) ⭐ 欢迎 Star

![R-Shell 命令行 SSH 工具封面](https://raw.githubusercontent.com/MageGojo/r-shell-cli/main/docs/images/rshell-cli-cover.png)

## 摘要

**R-Shell** 是一个用 **Rust** 编写的命令行 **SSH 工作台**，它把日常服务器运维里最高频的几件事——**保存 SSH 连接、执行远程命令、打开交互式终端、SFTP 上传下载、查看远程系统资源**——全部收敛到一个名为 `r-shell` 的单一可执行文件里，并内置一个本地 **MCP（Model Context Protocol）服务器**，方便 AI 工具直接管理你的连接。

本文从工程实战视角，完整介绍它的能力清单、命令用法、模块架构、关键源码实现，以及在 SSH 安全上的工程加固（主机密钥校验、凭据文件权限、MCP 访问控制），并给出**可直接复制运行**的实战示例。

读完本文你将掌握：

- R-Shell 提供哪些命令，每个命令分别解决什么运维痛点；
- 如何用一条命令完成远程执行、文件传输、系统监控；
- 用 Rust 写 CLI 工具的关键技术（`clap` 子命令、`russh` SSH、`tokio` 异步、`crossterm` 终端）；
- 一个 SSH 工具在安全上"应该做对的事"（这部分对生产运维尤其重要）；
- 如何把它接入支持 MCP 的 AI 客户端，让 AI 帮你管理连接。

---

## 目录

- [一、R-Shell 是什么：一句话定位](#一r-shell-是什么一句话定位)
- [二、它和 ssh + scp 有什么区别](#二它和-ssh--scp-有什么区别)
- [三、为什么选择 Rust 来写 SSH 命令行工具](#三为什么选择-rust-来写-ssh-命令行工具)
- [四、整体架构与模块设计](#四整体架构与模块设计)
- [五、安装与构建](#五安装与构建)
- [六、连接到远程主机的两种方式](#六连接到远程主机的两种方式)
- [七、命令详解与实战示例](#七命令详解与实战示例)
  - [7.1 connections：管理保存的连接](#71-connections管理保存的连接)
  - [7.2 exec：执行远程命令](#72-exec执行远程命令)
  - [7.3 shell：交互式终端](#73-shell交互式终端)
  - [7.4 ls：列出远程目录](#74-ls列出远程目录)
  - [7.5 upload / download：SFTP 文件传输](#75-upload--downloadsftp-文件传输)
  - [7.6 stats：远程系统资源快照](#76-stats远程系统资源快照)
  - [7.7 mcp：启动本地 MCP 服务器](#77-mcp启动本地-mcp-服务器)
- [八、核心源码剖析](#八核心源码剖析)
  - [8.1 用 clap 声明式定义子命令](#81-用-clap-声明式定义子命令)
  - [8.2 一次连接、用完即走的执行模型](#82-一次连接用完即走的执行模型)
  - [8.3 主机密钥校验是怎么实现的](#83-主机密钥校验是怎么实现的)
- [九、安全设计：一个 SSH 工具应该做对的事](#九安全设计一个-ssh-工具应该做对的事)
- [十、把 R-Shell 接入 AI 工具（MCP）](#十把-r-shell-接入-ai-工具mcp)
- [十一、常见问题 FAQ](#十一常见问题-faq)
- [十二、总结与展望](#十二总结与展望)

---

## 一、R-Shell 是什么：一句话定位

R-Shell 是一个**单文件、可脚本化的命令行 SSH 工作台**。它不是图形界面应用，而是一个像 `git`、`docker`、`kubectl` 那样用「子命令」组织功能的 CLI 工具。

它的核心能力可以用一张表概括：

| 能力 | 对应命令 | 一句话说明 |
| --- | --- | --- |
| 管理保存的 SSH 连接（增删改查） | `connections list/add/update/remove` | 像通讯录一样管理服务器 |
| 密码与私钥两种认证 | 所有连接类命令 | 支持加密私钥与口令 |
| 执行单条远程命令 | `exec` | 一行命令拿到远端输出 |
| 打开交互式 PTY 终端 | `shell` | 支持 vim/htop/less |
| 列出远程目录 | `ls` | 表格或 JSON 输出 |
| SFTP 上传 / 下载文件 | `upload` / `download` | 不依赖远端 scp |
| 远程系统资源快照 | `stats` | CPU/内存/磁盘/网络一目了然 |
| 本地 MCP 服务器 | `mcp` | 供 AI 工具调用 |

设计理念很简单：**一个子命令只做一件事，输出尽量是可被 `grep`/`jq` 处理的纯文本**，需要结构化时再加 `--json`。这让它既适合人手敲，也适合写进自动化脚本和 CI。

---

## 二、它和 ssh + scp 有什么区别

很多人会问：系统自带 `ssh`、`scp`、`sftp` 已经够用了，为什么还要 R-Shell？下面这张对比表能说明它的定位：

| 场景 | 传统 `ssh` / `scp` | R-Shell |
| --- | --- | --- |
| 连接信息管理 | 手写 `~/.ssh/config` 或记 IP | `connections` 子命令统一增删改查，可 `--json` |
| 执行远程命令 | `ssh user@host "cmd"` | `r-shell exec -c prod -- cmd`，连接信息一次保存反复用 |
| 文件传输 | `scp` 需要单独记路径 | `upload` / `download` 复用同一套连接定义 |
| 看服务器负载 | 手动 `top`、`df`、`free` 分别敲 | `stats` 一条命令聚合输出 |
| 给 AI 工具用 | 不支持 | 内置 MCP 服务器，AI 可直接管理连接 |
| 安全默认值 | 取决于系统配置 | 内置 TOFU 主机密钥校验、凭据文件 0600 |

简单说：**R-Shell 不是要取代 `ssh`，而是把「连接管理 + 常用操作 + 安全默认 + AI 集成」打包成一个顺手、可脚本化的工具**。

---

## 三、为什么选择 Rust 来写 SSH 命令行工具

选择 Rust 实现这样一个工具，主要基于三点工程考量：

1. **单二进制、零运行时依赖**。`cargo build --release` 产出一个可执行文件，拷到服务器即可运行，不用装 Node、Python 或一堆动态库——这对运维工具尤其重要。
2. **内存安全 + 异步生态成熟**。SSH/SFTP 是典型的 I/O 密集场景，配合 `tokio` 异步运行时和 `russh` 这一纯 Rust 的 SSH 实现，可以在不写一行 `unsafe` 的前提下获得不错的性能与稳定性。
3. **类型系统帮你把错误处理写对**。用 `anyhow::Result` 贯穿所有可能失败的操作，连接超时、认证失败、命令非零退出都能转换为清晰的错误信息，而不是莫名其妙的 panic。

依赖栈一览（节选自 `Cargo.toml`）：

```toml
[dependencies]
anyhow = "1"                                    # 统一错误处理
clap = { version = "4", features = ["derive"] } # 命令行参数解析
crossterm = "0.28"                              # 终端 raw mode / 无回显输入
russh = "0.43"                                  # 纯 Rust SSH 协议实现
russh-keys = "0.43"                             # SSH 密钥处理 / known_hosts
russh-sftp = "2"                                # SFTP 子系统
tokio = { version = "1", features = ["full"] }  # 异步运行时
rmcp = { version = "1.7", features = ["server", "transport-streamable-http-server"] } # MCP 服务器
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

各库分工：

- `clap`：命令行参数解析（derive 宏，声明式定义子命令，自动生成 `--help`）；
- `russh` / `russh-keys` / `russh-sftp`：SSH 协议握手、密钥处理、SFTP 子系统；
- `crossterm`：交互式终端的 raw mode 与无回显密码输入；
- `tokio`：驱动所有异步 I/O；
- `rmcp`：MCP 服务器实现（Streamable HTTP 传输）。

---

## 四、整体架构与模块设计

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

这种分层的最大好处是：**业务逻辑完全不依赖任何界面框架**，既能被 CLI 复用，也能被 MCP 服务复用。事实上，R-Shell 最初是一个图形界面应用，正是因为核心逻辑与界面解耦，才能在重构成 CLI 时几乎零成本地复用全部后端模块。

---

## 五、安装与构建

环境要求：安装 **Rust 与 Cargo** 即可（推荐用 [rustup](https://rustup.rs/) 安装）。

```bash
# 1. 克隆仓库
git clone https://github.com/MageGojo/r-shell-cli.git
cd r-shell-cli

# 2. 构建 release 二进制
cargo build --release --manifest-path cli/Cargo.toml

# 3. 产物路径：
#   cli/target/release/r-shell
```

把 `cli/target/release/r-shell` 拷贝到 `PATH` 目录（如 `/usr/local/bin`）后，就能直接使用 `r-shell` 命令：

```bash
sudo cp cli/target/release/r-shell /usr/local/bin/
r-shell --version
```

下文示例默认它已在 `PATH` 中。查看帮助：

```bash
r-shell --help               # 总览所有子命令
r-shell <command> --help     # 查看某个子命令的全部参数
```

---

## 六、连接到远程主机的两种方式

几乎所有需要联网的命令都接受一个**目标（target）**，有两种指定方式：

**方式一：引用已保存的连接**（推荐日常使用）

```bash
r-shell exec -c prod -- whoami                # 按名称引用
r-shell exec --connection ssh-123 -- whoami   # 按 id 引用
```

**方式二：临时（ad-hoc）指定主机**

```bash
r-shell exec --host 203.0.113.10 --user deploy --port 22 -- whoami
```

如果某个连接需要密码但你没有提供，R-Shell 会在连接时**安全地提示输入**（输入不回显，不会留在 shell 历史里）。

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

## 七、命令详解与实战示例

### 7.1 connections：管理保存的连接

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

`connections list` 的表格输出示例：

```text
ID                    NAME                    HOST                    AUTH        FOLDER
ssh-1781247286839     prod                    deploy@203.0.113.10:22  publickey   Work
ssh-1781247300001     test                    root@192.168.1.20:22    password    Personal
```

`connections list --json` 的输出是**脱敏**的——只会告诉你某连接有没有密码（`has_password`），绝不会回显明文凭据：

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

### 7.2 exec：执行远程命令

在远程主机上执行一条命令并打印输出，`--` 后面的内容会原样发送到远端。

```bash
r-shell exec -c prod -- uname -a
r-shell exec -c prod -- "ls -la /var/www && df -h"
r-shell exec --host 203.0.113.10 --user deploy -- systemctl status nginx
```

配合管道，可以很自然地融入脚本：

```bash
# 把远端的磁盘使用率取出来做判断
USAGE=$(r-shell exec -c prod -- "df / | tail -1 | awk '{print \$5}'")
echo "根分区使用率：$USAGE"
```

### 7.3 shell：交互式终端

打开一个完整的交互式 PTY 终端，支持 `vim`、`htop`、`less` 等需要伪终端的程序。底层用 `crossterm` 的 raw mode 把本地按键透传到远端。

```bash
r-shell shell -c prod
```

按 **`Ctrl-]`** 可以强制退出本地的 shell 循环。

### 7.4 ls：列出远程目录

```bash
r-shell ls -c prod /var/log
r-shell ls -c prod /var/log --json
r-shell ls -c prod              # 默认列当前/家目录
```

输出列为：类型（`DIR`/`FILE`/`LNK`）、权限、大小、修改时间、名称。注意 `ls` 与 `stats` 面向 Linux 主机（依赖 GNU `ls` 与 `/proc`）。

### 7.5 upload / download：SFTP 文件传输

基于 SFTP 子系统的单文件传输，**不依赖远端是否安装 scp**。

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

### 7.6 stats：远程系统资源快照

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

它的实现思路是：用一条命令一次性把 `/proc/stat`、`/proc/meminfo`、`/proc/net/dev`、`df` 等信息抓回来，再在本地解析。CPU 利用率和网络速率属于「速率量」，需要两次采样做差值才能算出来。

### 7.7 mcp：启动本地 MCP 服务器

启动一个本地的 MCP（Model Context Protocol）Streamable HTTP 服务器，让支持 MCP 的 AI 工具可以管理你保存的 SSH 连接。仅绑定到 localhost。

```bash
r-shell mcp
# R-Shell MCP server listening on http://127.0.0.1:9123/mcp
# Press Ctrl-C to stop.
```

---

## 八、核心源码剖析

光看用法不够过瘾，下面挑三段最能体现工程实现的核心代码来讲。

### 8.1 用 clap 声明式定义子命令

借助 `clap` 的 derive 宏，整个命令树用结构体和枚举就能声明出来，`--help` 自动生成：

```rust
use clap::{Args, Parser, Subcommand};

/// R-Shell — a command-line SSH workspace.
#[derive(Parser, Debug)]
#[command(name = "r-shell", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// 管理保存的 SSH 连接
    #[command(subcommand)]
    Connections(ConnectionsCommand),
    /// 在远程主机上执行一条命令
    Exec(ExecArgs),
    /// 打开交互式 shell（PTY）
    Shell(TargetArgs),
    /// 列出远程目录
    Ls(LsArgs),
    /// SFTP 上传
    Upload(UploadArgs),
    /// SFTP 下载
    Download(DownloadArgs),
    /// 远程系统资源快照
    Stats(TargetArgs),
    /// 运行本地 MCP 服务器
    Mcp,
}
```

主函数只需把解析结果分发到对应处理函数：

```rust
fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Connections(cmd) => run_connections(cmd),
        Command::Exec(args) => block_on(cmd_exec(args)),
        Command::Shell(target) => block_on(cmd_shell(target)),
        Command::Ls(args) => block_on(cmd_ls(args)),
        Command::Upload(args) => block_on(cmd_upload(args)),
        Command::Download(args) => block_on(cmd_download(args)),
        Command::Stats(target) => block_on(cmd_stats(target)),
        Command::Mcp => block_on(cmd_mcp()),
    }
}
```

### 8.2 一次连接、用完即走的执行模型

对于 `exec` / `ls` / `upload` 等一次性命令，R-Shell 采用「连接 → 执行 → 断开」的模型，用一个高阶函数把这套生命周期封装起来，避免每个命令都重复写连接和清理逻辑：

```rust
/// 连接到目标主机，把活跃的连接管理器交给 `body` 执行，结束后自动断开。
async fn with_connection<F, Fut, T>(target: &TargetArgs, body: F) -> Result<T>
where
    F: FnOnce(Arc<NativeConnectionManager>) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let config = resolve_target(target)?;
    let manager = Arc::new(NativeConnectionManager::new());
    manager
        .create_connection(CONNECTION_ID.to_string(), config)
        .await
        .context("failed to establish SSH connection")?;

    let result = body(manager.clone()).await;
    let _ = manager.close_connection(CONNECTION_ID).await; // 无论成功失败都断开
    result
}
```

于是每个命令的实现都非常简洁，比如 `exec`：

```rust
async fn cmd_exec(args: ExecArgs) -> Result<()> {
    let command = args.command.join(" ");
    with_connection(&args.target, |manager| async move {
        let output = manager.execute_command(CONNECTION_ID, &command).await?;
        print!("{output}");
        Ok(())
    })
    .await
}
```

### 8.3 主机密钥校验是怎么实现的

这是整个工具安全性的核心。`russh` 在握手阶段会回调 `check_server_key`，我们在这里实现 TOFU 逻辑：

```rust
async fn check_server_key(
    &mut self,
    server_public_key: &key::PublicKey,
) -> Result<bool, Self::Error> {
    if self.insecure {
        return Ok(true); // --insecure：明知风险，跳过校验
    }

    match russh_keys::check_known_hosts(&self.host, self.port, server_public_key) {
        Ok(true) => Ok(true),               // 已记录且匹配 → 接受
        Ok(false) => {                      // 首次见到该主机 → 记录并接受（TOFU）
            let _ = russh_keys::learn_known_hosts(&self.host, self.port, server_public_key);
            eprintln!("[ssh] permanently added '{}:{}' to known hosts.", self.host, self.port);
            Ok(true)
        }
        Err(russh_keys::Error::KeyChanged { line }) => {  // 密钥变了 → 拒绝（可能是 MITM）
            eprintln!("[ssh] REMOTE HOST IDENTIFICATION HAS CHANGED (line {line})! Refused.");
            Ok(false)
        }
        Err(_) => Ok(false),                // 读取 known_hosts 失败 → 安全起见拒绝
    }
}
```

这套逻辑和 OpenSSH 的行为完全对齐：首次信任、之后校验、变更即拒。

---

## 九、安全设计：一个 SSH 工具应该做对的事

SSH 工具最容易被忽视、却最关键的就是安全。R-Shell 在这几方面做了明确的工程实践：

### 9.1 主机密钥校验（防中间人攻击）

R-Shell 会按照标准的 `~/.ssh/known_hosts` 做 **TOFU（Trust-On-First-Use，首次信任）** 校验：

- **首次连接**某主机：把它的主机密钥记录进 `known_hosts`，并提示 `permanently added`；
- **后续连接**：密钥必须与记录一致；
- **密钥不匹配**：直接**拒绝连接**——这通常意味着可能的中间人攻击。

```text
[ssh] REMOTE HOST IDENTIFICATION HAS CHANGED for 203.0.113.10:22!
[ssh] The host key does not match the one in known_hosts ...
```

如果确认是合法的密钥变更（比如服务器重装），删除 `~/.ssh/known_hosts` 里对应的那一行再重连即可。明知风险的测试场景可以用 `--insecure` 跳过校验。

### 9.2 凭据文件权限收紧

存放连接的 `workspace.json` 及其目录在写入时会被设置为**仅属主可访问**（Unix 下 `0600` / `0700`），避免同机器上的其他用户读到你的连接配置：

```rust
#[cfg(unix)]
fn restrict_file_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}
```

### 9.3 MCP 端点访问控制

MCP 服务只绑定 `127.0.0.1`，并且：

- 请求的 `Host` 头必须是回环地址（`127.0.0.1` / `localhost` / `[::1]`），以此**抵御 DNS rebinding**（恶意域名解析到本地的攻击）；
- 如果带了 `Origin` 头，必须是回环来源；**跨站来源和 `null` 来源一律拒绝**（返回 `403`）；
- MCP 的列表接口对凭据脱敏，**不返回任何明文密码或私钥**。

这些细节看似琐碎，但正是区分「能用」和「可放心用」的地方。

---

## 十、把 R-Shell 接入 AI 工具（MCP）

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

这样你就能让 AI 助手帮你「列出我所有的 SSH 连接」「新建一个连接到测试机」等等，而所有敏感凭据都不会通过 MCP 泄露出去。

---

## 十一、常见问题 FAQ

**Q1：`ls` 或 `stats` 在某些主机上报错怎么办？**

A：这两个命令面向 Linux 服务器，依赖 GNU `ls --time-style=long-iso` 和 `/proc` 文件系统。在 macOS/BSD 等主机上可能不可用，但 `exec` 可以执行任意命令，不受此限制。

**Q2：连接保存在哪里？**

A：保存在本地数据目录的 `r-shell/workspace.json`。各平台路径如下：

| 操作系统 | 路径 |
| --- | --- |
| macOS | `~/Library/Application Support/r-shell/workspace.json` |
| Linux | `~/.local/share/r-shell/workspace.json` |
| Windows | `%LOCALAPPDATA%\r-shell\workspace.json` |

**Q3：密码会明文存储吗？**

A：如果你用 `--password` 保存密码，它会写入 `workspace.json`，但该文件已被设置为仅属主可读（`0600`）。更推荐使用公钥认证，或不保存密码、连接时再交互输入。

**Q4：第一次连接提示 `permanently added` 是正常的吗？**

A：正常。这是 TOFU 首次信任机制在记录主机密钥，和 OpenSSH 第一次连接新主机时的行为一致。

**Q5：支持 Windows 吗？**

A：作为纯 Rust + tokio 的程序，`r-shell` 本身可在 Windows 上构建运行；`connections` / `exec` / `upload` / `download` / `shell` / `mcp` 都可用，仅 `ls` / `stats` 因依赖 Linux 远端特性而面向 Linux 主机。

**Q6：可以在 CI/CD 流水线里用吗？**

A：可以。`exec`、`upload`、`download` 都是一次性命令，配合保存的连接（或临时 `--host` 参数）很容易写进部署脚本；`--json` 输出也便于在流水线里解析。

---

## 十二、总结与展望

R-Shell 把「保存连接、远程执行、交互式终端、SFTP 传输、系统监控、MCP 服务」这些运维高频操作，收敛进一个用 **Rust** 写的命令行工具里，具备以下特点：

- **单二进制、可脚本化**，子命令职责清晰，输出对管道友好；
- **核心逻辑与界面解耦**，CLI 与 MCP 服务共享同一套后端；
- **安全上做了正确的事**：主机密钥 TOFU 校验、凭据文件权限收紧（0600/0700）、MCP 双重访问控制（Host + Origin）与凭据脱敏。

如果你日常需要在多台服务器之间跑命令、传文件、看负载，又想要一个轻量、安全、可被脚本和 AI 工具调用的 SSH 工具，欢迎试用并 Star：

👉 **项目地址**：[https://github.com/MageGojo/r-shell-cli](https://github.com/MageGojo/r-shell-cli)

后续计划包括：流式大文件传输、目录批量传输、更多远端平台的 `stats` 适配等。也欢迎在 GitHub 提 Issue 和 PR 一起完善。

> 如果这篇文章对你有帮助，欢迎**点赞、收藏、评论**交流。后续会继续分享 Rust 命令行工具开发与运维自动化相关的实战内容，感谢阅读！
