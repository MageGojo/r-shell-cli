# R-Shell — The AI-Native SSH Client & MCP Server for Remote Servers (Rust)

**中文文档：[README.zh-CN.md](README.zh-CN.md)** ｜ 关键词：AI 助手 SSH 工具 · MCP 服务器 · 让 Cursor / Claude 操作远程服务器 · AI 远程运维 · 大模型连接服务器

> **R-Shell is an open-source, AI-native SSH tool built so AI assistants can
> operate your remote servers safely.** It ships a built-in **MCP (Model Context
> Protocol) server**, so agents like **Cursor, Claude, and Claude Code** can open
> persistent SSH sessions, run remote commands, read and write files over SFTP,
> list directories, and check system stats — all through one secure,
> localhost-only endpoint. The same single `r-shell` binary is also a fast,
> scriptable SSH client for humans, working on **macOS (Apple Silicon & Intel),
> Linux, and Windows**.

> **AI agents:** start the server with `r-shell mcp`, then connect to
> `http://127.0.0.1:9123/mcp`. See
> [R-Shell for AI Assistants](#r-shell-for-ai-assistants-mcp) for ready-to-paste
> Cursor/Claude config and example prompts.

<p align="center">
  <a href="https://github.com/MageGojo/r-shell-cli/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/MageGojo/r-shell-cli?label=download&sort=semver"></a>
  <a href="https://github.com/MageGojo/r-shell-cli/actions/workflows/release.yml"><img alt="Release builds" src="https://img.shields.io/github/actions/workflow/status/MageGojo/r-shell-cli/release.yml?label=release%20build"></a>
  <img alt="Platforms" src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-blue">
  <img alt="Language" src="https://img.shields.io/badge/built%20with-Rust-orange?logo=rust">
  <a href="#license"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-green"></a>
</p>

```text
r-shell <command> [options]
```

**Jump to:** [AI Assistants (MCP)](#r-shell-for-ai-assistants-mcp) ·
[Download (DMG / installer)](#download--install-prebuilt-binaries) ·
[Quick Start](#quick-start) · [Tutorial](#tutorial-from-zero-to-your-first-remote-command) ·
[Command Reference](#command-reference) · [FAQ](#faq--frequently-asked-questions)

---

## What is R-Shell? (TL;DR)

_一句话：R-Shell 是 **AI 原生的 SSH 工具 / MCP 服务器**，让 AI 助手(Cursor、Claude)通过一条安全的持久连接运行远程命令、读写文件、管理服务器。_

**R-Shell is an AI-native, Rust-based SSH tool.** Its core purpose is to give AI
assistants a safe, structured way to operate remote servers via the **Model
Context Protocol (MCP)** — while doubling as a fast, scriptable SSH client for
humans. In short:

- **What it is:** an **MCP server for SSH** plus a single-binary SSH client, SFTP
  transfer tool, and remote system monitor.
- **Who it is for:** anyone using **AI coding agents (Cursor, Claude, Claude
  Code)** to manage servers, plus developers, DevOps/SRE engineers, and sysadmins
  who want reproducible, scriptable SSH workflows.
- **Why it exists:** AI agents shouldn’t shell out to raw `ssh` and re-authenticate
  on every step. R-Shell keeps **one persistent SSH session** alive and exposes
  safe, named tools (`ssh_exec`, `ssh_read_file`, `ssh_write_file`, …) so agents
  can iterate quickly without leaking credentials or tripping intrusion detection.
- **What it is *not*:** it is **not** a GUI app and not a background daemon — it
  connects, does its job, and exits (except `shell` and `mcp`, which stay open).

> **In one sentence:** *R-Shell is the AI-native SSH workspace — an MCP server
> that lets AI assistants run commands, edit files, and manage remote servers
> through one secure, persistent connection.*

---

## Table of Contents

- [What is R-Shell? (TL;DR)](#what-is-r-shell-tldr)
- [R-Shell for AI Assistants (MCP)](#r-shell-for-ai-assistants-mcp)
- [Why R-Shell? (Key Benefits)](#why-r-shell-key-benefits)
- [Features](#features)
- [Download & Install (Prebuilt Binaries)](#download--install-prebuilt-binaries)
  - [macOS — install from a `.dmg`](#macos--install-from-a-dmg)
  - [Windows — install with the `.exe` installer](#windows--install-with-the-exe-installer)
  - [Linux / build from source](#linux--build-from-source)
- [Quick Start](#quick-start)
- [Tutorial: From Zero to Your First Remote Command](#tutorial-from-zero-to-your-first-remote-command)
- [Connecting to a Host](#connecting-to-a-host)
- [Command Reference](#command-reference)
  - [`connections`](#connections--manage-saved-hosts)
  - [`exec`](#exec--run-a-remote-command)
  - [`shell`](#shell--interactive-terminal)
  - [`ls`](#ls--list-a-remote-directory)
  - [`upload` / `download`](#upload--download--sftp-transfer)
  - [`stats`](#stats--remote-system-snapshot)
  - [`mcp`](#mcp--run-the-mcp-server)
- [Authentication](#authentication)
- [Data & Configuration](#data--configuration)
- [MCP Integration (AI Assistants)](#mcp-integration-ai-assistants)
- [Troubleshooting](#troubleshooting)
- [FAQ — Frequently Asked Questions](#faq--frequently-asked-questions)
- [Development](#development)
- [Project Structure](#project-structure)
- [Security](#security)
- [License](#license)

---

## R-Shell for AI Assistants (MCP)

**中文关键词｜AI 助手 SSH 工具 · MCP 服务器 · 让 Cursor / Claude 操作远程服务器 · AI 运维 · AI 连接服务器 · 大模型远程命令执行**

R-Shell is built **AI-first**: its primary job is to let AI coding agents operate
remote servers safely. Instead of an agent spawning raw `ssh`/`scp` subprocesses
and re-authenticating on every step, R-Shell exposes a small set of **named,
sandboxed tools over the Model Context Protocol (MCP)** and keeps a single SSH
session alive between calls.

### Why agents should use R-Shell instead of raw `ssh` ｜为什么 AI 智能体该用 R-Shell 而非裸 `ssh`

- **One persistent session** — open it once with `ssh_session_open`, then reuse it
  for every command and file edit. No reconnect storms, no repeated auth prompts.
- **Structured tools, not string-munging** — `ssh_exec`, `ssh_read_file`,
  `ssh_write_file`, `ssh_list_dir` return clean results an agent can reason about.
- **Credentials never leak** — passwords, keys, and passphrases are **never**
  echoed or returned; list calls only expose `has_password` booleans.
- **Localhost-only & rebinding-safe** — the endpoint binds to `127.0.0.1` and
  rejects non-loopback `Host`/`Origin`, so a browser tab or remote site can’t
  reach it.
- **Fewer false alarms** — reusing one connection avoids the rapid repeated
  logins that can trip server-side intrusion detection (e.g. fail2ban).

### 1-minute setup for Cursor / Claude ｜Cursor / Claude 一分钟接入配置

**Step 1 — start the server:**

```bash
r-shell mcp
# R-Shell MCP server listening on http://127.0.0.1:9123/mcp
```

**Step 2 — point your AI client at it.** Cursor (`~/.cursor/mcp.json`) or any
MCP-compatible client:

```json
{
  "mcpServers": {
    "r-shell": {
      "url": "http://127.0.0.1:9123/mcp"
    }
  }
}
```

> Claude Desktop and other clients use the same URL — add it as a Streamable HTTP
> MCP server, then restart the client so it picks up the new server.

**Step 3 — talk to your servers in natural language.** Example prompts that work
once a connection is saved (or given inline):

- *“Open a session to my `prod` server and show disk usage plus the last 50 lines
  of `/var/log/nginx/error.log`.”*
- *“Edit `/etc/nginx/sites-enabled/app.conf` on `prod` to add `gzip on;`, then
  reload nginx.”*
- *“Check CPU and memory on `staging`, and list what’s in `/opt/app/releases`.”*
- *“Deploy: upload `./build.tar.gz` to `/tmp` on `prod`, extract it to
  `/opt/app`, and restart the service.”*

### Tools available to AI agents ｜AI 智能体可调用的工具(MCP Tools)

| Tool | What the agent can do |
| --- | --- |
| `ssh_session_open` | Open/reuse a persistent SSH session → returns a `session_id` |
| `ssh_exec` | Run a command on the open session |
| `ssh_read_file` | Read a remote file (UTF-8, or base64 for binary) |
| `ssh_write_file` | Create/overwrite a remote file |
| `ssh_list_dir` | List a remote directory |
| `ssh_sessions_list` / `ssh_session_close` | Inspect / close live sessions |
| `r_shell_ssh_connections_list` | List saved connections (sanitized) |
| `r_shell_ssh_connection_create/update/delete` | Manage saved connections |

See [MCP Integration (AI Assistants)](#mcp-integration-ai-assistants) for the
full tool schema, security model, and persistent-session details.

---

## Why R-Shell? (Key Benefits)

| If you want to… | R-Shell gives you… |
| --- | --- |
| **Let AI agents (Cursor/Claude) run your servers** | **`r-shell mcp` — a secure, localhost-only MCP server with persistent SSH sessions** |
| Stop re-typing `ssh user@host -p port -i key` | **Saved connections** referenced by short name (`-c prod`) |
| Run one remote command and capture output | `r-shell exec -c prod -- <cmd>` with clean, greppable stdout |
| A real interactive shell (vim, htop, less) | `r-shell shell -c prod` over a full PTY in raw mode |
| Move files without remembering `scp` syntax | `r-shell upload` / `download` over SFTP |
| A quick health check of a remote box | `r-shell stats -c prod` (CPU, memory, disk, network) |
| One tool on macOS, Linux, and Windows | A **single static-ish Rust binary**, no runtime to install |

---

## Features

| Feature | Command |
| --- | --- |
| Manage saved SSH connections | `connections list/add/update/remove` |
| Password & private-key auth | (all connecting commands) |
| Run a single remote command | `exec` |
| Interactive PTY shell | `shell` |
| List a remote directory | `ls` |
| Upload a file over SFTP | `upload` |
| Download a file over SFTP | `download` |
| Remote system resource snapshot | `stats` |
| Local MCP server for AI tools | `mcp` |

> **Note:** `ls` and `stats` target Linux hosts (they rely on GNU `ls` and the
> `/proc` filesystem). `exec` runs any command on any POSIX host.

---

## Download & Install (Prebuilt Binaries)

_下载与安装：macOS(Apple Silicon / Intel)`.dmg` 镜像、Windows x64 `.exe` 安装器；Linux 用 Cargo 源码编译。_

The easiest way to install R-Shell is to grab a prebuilt package from the
**[GitHub Releases page](https://github.com/MageGojo/r-shell-cli/releases/latest)**.
Every tagged release is built automatically by CI and ships three downloads:

| Platform | Download | What you get |
| --- | --- | --- |
| **macOS — Apple Silicon (M1/M2/M3/M4)** | `r-shell-macos-apple-silicon.dmg` | A signed-style `.dmg` disk image with the `r-shell` binary |
| **macOS — Intel** | `r-shell-macos-intel.dmg` | The Intel (`x86_64`) `.dmg` build |
| **Windows — x64** | `r-shell-windows-x64-installer.exe` | An NSIS installer that adds `r-shell` to your `PATH` |

> Not sure which macOS build you need? Click the Apple menu →  **About This Mac**.
> A chip labeled **Apple M1/M2/M3/M4** means Apple Silicon; an **Intel** processor
> means the Intel build.

### macOS — install from a `.dmg`

**Step-by-step:**

1. Download `r-shell-macos-apple-silicon.dmg` (Apple Silicon) or
   `r-shell-macos-intel.dmg` (Intel) from the
   [latest release](https://github.com/MageGojo/r-shell-cli/releases/latest).
2. **Double-click the `.dmg`** to mount it. A Finder window opens showing the
   `r-shell` binary.
3. **Copy `r-shell` to a folder on your `PATH`.** The simplest place is
   `/usr/local/bin`:

```bash
# After mounting the DMG (volume name shown in Finder, e.g. "R-Shell")
sudo cp /Volumes/R-Shell/r-shell /usr/local/bin/r-shell
sudo chmod +x /usr/local/bin/r-shell
```

4. **Allow the unsigned binary the first time.** Because the binary is not
   notarized, macOS Gatekeeper may block it. Clear the quarantine flag once:

```bash
xattr -dr com.apple.quarantine /usr/local/bin/r-shell
```

> Alternatively: try to run it, then go to **System Settings → Privacy &
> Security** and click **“Open Anyway”**.

5. **Verify the install:**

```bash
r-shell --version
r-shell --help
```

### Windows — install with the `.exe` installer

**Step-by-step:**

1. Download `r-shell-windows-x64-installer.exe` from the
   [latest release](https://github.com/MageGojo/r-shell-cli/releases/latest).
2. **Run the installer** (double-click). If Windows SmartScreen warns about an
   unknown publisher, click **More info → Run anyway**.
3. Follow the wizard. The installer copies `r-shell.exe` and **adds it to your
   `PATH`** automatically.
4. **Open a new terminal** (PowerShell or Windows Terminal) so the updated `PATH`
   takes effect, then verify:

```powershell
r-shell --version
r-shell --help
```

> Prefer no installer? You can also extract `r-shell.exe` from the installer or
> build from source (below) and place the `.exe` anywhere on your `PATH`.

### Linux / build from source

There is no prebuilt Linux package yet, and building from source works on any
platform. You need **Rust** and **Cargo** (install via
[rustup.rs](https://rustup.rs)); **Node.js + pnpm** are optional for the wrapper
scripts.

```bash
# 1. Clone the repository
git clone https://github.com/MageGojo/r-shell-cli.git
cd r-shell-cli

# 2. Build a release binary
cargo build --release --manifest-path cli/Cargo.toml

# 3. The binary is produced at:
#    cli/target/release/r-shell

# 4. Install it onto your PATH (Linux/macOS example)
sudo install -m 0755 cli/target/release/r-shell /usr/local/bin/r-shell

# 5. Verify
r-shell --version
```

Prefer not to install it globally? Run it straight through Cargo:

```bash
cargo run --manifest-path cli/Cargo.toml -- <command> [options]
```

> **Build prerequisites note:** R-Shell uses the pure-Rust `russh` SSH stack, so
> you do **not** need OpenSSL or `libssh` system libraries to compile it.

---

## Quick Start

```bash
# 1. Save a connection
r-shell connections add --name prod --host 203.0.113.10 --username deploy \
  --auth publickey --key-path ~/.ssh/id_ed25519

# 2. List saved connections
r-shell connections list

# 3. Run a command on it
r-shell exec -c prod -- uptime

# 4. Open an interactive shell
r-shell shell -c prod

# 5. Copy a file up and back down
r-shell upload   -c prod ./app.tar.gz /tmp/app.tar.gz
r-shell download -c prod /tmp/app.tar.gz ./app-copy.tar.gz
```

---

## Tutorial: From Zero to Your First Remote Command

_新手教程：从安装到保存连接、运行命令、交互式 shell、SFTP 传输、查看服务器状态、接入 AI 助手(MCP)的完整流程。_

This walkthrough takes you from a fresh install to running commands, opening a
shell, transferring files, and wiring R-Shell into an AI assistant. Every step is
self-contained — copy, paste, and adjust the host details.

### Step 1 — Confirm R-Shell is installed

```bash
r-shell --version     # prints the installed version
r-shell --help        # lists every subcommand
```

If `r-shell` is “command not found”, revisit
[Download & Install](#download--install-prebuilt-binaries) and make sure the
binary is on your `PATH`.

### Step 2 — Save your first connection

Saving a connection means you never re-type host, user, port, or key path again.

**Using a private key (recommended):**

```bash
r-shell connections add \
  --name prod \
  --host 203.0.113.10 \
  --username deploy \
  --port 22 \
  --auth publickey \
  --key-path ~/.ssh/id_ed25519 \
  --folder Work \
  --description "Production web server"
```

**Using a password** (you can omit `--password` to be prompted securely later):

```bash
r-shell connections add --name staging --host 203.0.113.20 \
  --username deploy --auth password
```

Confirm it was saved:

```bash
r-shell connections list
```

### Step 3 — Run your first remote command

The first time you connect, R-Shell records the server’s host key in
`~/.ssh/known_hosts` (trust-on-first-use). Then it runs your command and exits:

```bash
r-shell exec -c prod -- uptime
r-shell exec -c prod -- "df -h && free -m"
```

Everything after `--` is sent to the remote host **verbatim**, so quoting works
exactly like a normal shell.

### Step 4 — Open a full interactive shell

When you need `vim`, `htop`, `top`, `less`, or an interactive session:

```bash
r-shell shell -c prod
```

This is a real PTY in raw mode. Press **`Ctrl-]`** to force-quit the local loop
if a session hangs.

### Step 5 — Transfer files over SFTP

```bash
# Upload a local file to the server
r-shell upload -c prod ./release.tar.gz /tmp/release.tar.gz

# Download a remote file back to your machine
r-shell download -c prod /var/log/app.log ./app.log
```

### Step 6 — Check the server’s health

```bash
r-shell stats -c prod
```

You’ll get a one-shot snapshot of CPU %, load, memory, swap, disk, and network
throughput (Linux hosts).

### Step 7 (optional) — Let an AI assistant drive your servers

Start the local MCP server and point a tool like **Cursor** or **Claude
Desktop** at it:

```bash
r-shell mcp
# R-Shell MCP server listening on http://127.0.0.1:9123/mcp
```

See [MCP Integration](#mcp-integration-ai-assistants) for the client config and
the full list of tools. That’s it — you now have a complete, scriptable SSH
workflow.

---

## Connecting to a Host

Every command that talks to a remote host accepts a **target**, specified one of
two ways:

**1. Saved connection** — reference a previously saved connection by id or name:

```bash
r-shell exec -c prod -- whoami
r-shell exec --connection ssh-1781247286839 -- whoami
```

**2. Ad-hoc host** — pass connection details inline:

```bash
r-shell exec --host 203.0.113.10 --user deploy --port 22 -- whoami
```

If a password is required but not provided, R-Shell prompts for it securely
(input is not echoed).

Common target flags (available on `exec`, `shell`, `ls`, `upload`, `download`,
`stats`):

| Flag | Alias | Description | Default |
| --- | --- | --- | --- |
| `--connection <id\|name>` | `-c` | Use a saved connection | — |
| `--host <host>` | | Ad-hoc host (IP or hostname) | — |
| `--user <user>` | `-u` | Ad-hoc SSH username | — |
| `--port <port>` | `-p` | Ad-hoc SSH port | `22` |
| `--password <pw>` | | Ad-hoc password (prefer the prompt) | — |
| `--key-path <path>` | | Ad-hoc private key path | — |
| `--passphrase <pp>` | | Passphrase for an encrypted key | — |
| `--insecure` | | Skip host-key verification (dangerous) | `false` |

---

## Command Reference

Run `r-shell --help` or `r-shell <command> --help` at any time.

### `connections` — manage saved hosts

Saved connections live in a local `workspace.json` (see
[Data & Configuration](#data--configuration)).

```bash
# List (table or JSON)
r-shell connections list
r-shell connections list --json

# Add (password auth)
r-shell connections add \
  --name prod \
  --host 203.0.113.10 \
  --username deploy \
  --port 22 \
  --auth password \
  --password 's3cret' \
  --folder Work \
  --description "Production web server"

# Add (public-key auth)
r-shell connections add \
  --name prod \
  --host 203.0.113.10 \
  --username deploy \
  --auth publickey \
  --key-path ~/.ssh/id_ed25519 \
  --passphrase 'key-passphrase'

# Update individual fields
r-shell connections update <connection_id> --port 2222 --folder Staging

# Remove
r-shell connections remove <connection_id>
```

`connections add` flags: `--name`, `--host`, `--username` (required); `--port`
(default `22`); `--auth password|publickey` (default `password`); `--password`,
`--key-path`, `--passphrase`; `--folder` (default `All Connections`);
`--description`.

`connections update` takes a `<connection_id>` plus any of the same flags to
change just those fields.

### `exec` — run a remote command

Runs a single command and prints its output. Everything after `--` is sent to the
remote host verbatim.

```bash
r-shell exec -c prod -- uname -a
r-shell exec -c prod -- "ls -la /var/www && df -h"
r-shell exec --host 203.0.113.10 --user deploy -- systemctl status nginx
```

### `shell` — interactive terminal

Opens a full interactive PTY shell (supports `vim`, `htop`, `less`, etc.) using
raw terminal mode.

```bash
r-shell shell -c prod
```

Press **`Ctrl-]`** to force-quit the local shell loop.

### `ls` — list a remote directory

```bash
r-shell ls -c prod /var/log
r-shell ls -c prod /var/log --json
r-shell ls -c prod            # defaults to the home/current directory
```

Output columns: kind (`DIR`/`FILE`/`LNK`), permissions, size, modified time,
name. _(Linux hosts.)_

### `upload` / `download` — SFTP transfer

Single-file transfers over SFTP.

```bash
# Local -> remote
r-shell upload -c prod ./local.tar.gz /tmp/remote.tar.gz

# Remote -> local
r-shell download -c prod /tmp/remote.log ./local.log
```

### `stats` — remote system snapshot

Takes two quick samples and prints CPU %, load, memory, swap, disk usage, and
network throughput. _(Linux hosts; relies on `/proc`.)_

```bash
r-shell stats -c prod
```

Example output:

```text
OS:      Linux 6.1.0
Uptime:  12d 4h 31m
CPU:     7.4%  (8 cores, load 0.42)
Memory:  61.2%  (4.9/7.8 GB)
Disk:    40.0%  (3.8/9.5 GB)
Network: down 1.5 KB/s  up 320 B/s
```

### `mcp` — run the MCP server

_启动 MCP 服务器(供 AI 助手 / Cursor / Claude 连接)_

Starts a local **MCP (Model Context Protocol)** Streamable HTTP server so AI tools
can manage your saved connections. Bound to localhost only.

```bash
r-shell mcp
# R-Shell MCP server listening on http://127.0.0.1:9123/mcp
# Press Ctrl-C to stop.
```

---

## Authentication

R-Shell supports two methods:

- **Password** — `--auth password` with `--password`, or omit it and you will be
  prompted securely at connect time.
- **Public key** — `--auth publickey` with `--key-path` (and `--passphrase` for
  encrypted keys). Paths beginning with `~/` are expanded.

For saved connections, the auth method and any stored secrets are read from
`workspace.json`. If a saved password-auth connection has no stored password,
R-Shell prompts for it when you connect.

### Host-key verification

R-Shell verifies server host keys against your standard `~/.ssh/known_hosts`
file, using **trust-on-first-use (TOFU)**:

- **First connection** to a host: its key is recorded in `known_hosts` and the
  connection proceeds.
- **Subsequent connections**: the key must match the recorded one.
- **Key mismatch**: the connection is **refused** — this is the signal of a
  possible man-in-the-middle attack. To accept a legitimate change, remove the
  offending line from `~/.ssh/known_hosts` and reconnect.

Pass `--insecure` to skip host-key verification entirely. This disables MITM
protection and should only be used for throwaway or local test hosts.

---

## Data & Configuration

Saved connections are persisted as JSON at:

```text
<local data dir>/r-shell/workspace.json
```

The `<local data dir>` is platform-specific:

| OS | Path |
| --- | --- |
| macOS | `~/Library/Application Support/r-shell/workspace.json` |
| Linux | `~/.local/share/r-shell/workspace.json` |
| Windows | `%LOCALAPPDATA%\r-shell\workspace.json` |

This file is compatible with workspaces created by earlier R-Shell versions.

---

## MCP Integration (AI Assistants)

**中文关键词｜MCP 集成 · Model Context Protocol 服务器 · AI 助手对接 · Cursor / Claude Desktop 配置 · 持久 SSH 会话 · AI Agent 工具列表**

**R-Shell ships a built-in MCP (Model Context Protocol) server**, so AI coding
assistants — Cursor, Claude Desktop, and other MCP-compatible clients — can
manage your saved connections and operate remote servers through a secure,
localhost-only endpoint. Start it with `r-shell mcp` and point your client at
`http://127.0.0.1:9123/mcp`.

The MCP server exposes these tools (credentials are never returned).

**Connection management** (operates on `workspace.json`):

| Tool | Description |
| --- | --- |
| `r_shell_ssh_connections_list` | List saved connections (sanitized) |
| `r_shell_ssh_connection_create` | Create a saved connection |
| `r_shell_ssh_connection_update` | Update a saved connection |
| `r_shell_ssh_connection_delete` | Delete a saved connection |
| `r_shell_ssh_tabs_list` | List open tabs (always empty in the CLI) |

**Persistent sessions** — keep one SSH connection alive across calls so you can
run commands and read/write remote files repeatedly **without reconnecting**:

| Tool | Description |
| --- | --- |
| `ssh_session_open` | Open (or reuse) a session; returns a `session_id`. Use a saved `connection` (id\|name) or ad-hoc `host`/`username` (+ credentials) |
| `ssh_exec` | Run a command on the open session |
| `ssh_read_file` | Read a remote file (UTF-8 text, or base64 for binary) |
| `ssh_write_file` | Overwrite a remote file (`content` text or `content_base64`) |
| `ssh_list_dir` | List a remote directory |
| `ssh_sessions_list` | List session ids that are currently connected |
| `ssh_session_close` | Close a session |

The session lives for as long as `r-shell mcp` runs. For a password-auth
connection with no stored password, pass `password` to `ssh_session_open`
(the server cannot prompt). Sessions are held in memory only and are never
written to `workspace.json`.

Start it with `r-shell mcp`, then point an MCP client at
`http://127.0.0.1:9123/mcp`. For example, a Cursor / Claude-style MCP config:

```json
{
  "mcpServers": {
    "r-shell": {
      "url": "http://127.0.0.1:9123/mcp"
    }
  }
}
```

The server only accepts requests whose `Host` header is loopback (`localhost`,
`127.0.0.1`, `[::1]`); if an `Origin` header is present it must also be loopback.
Cross-site origins, the literal `null` origin, and rebound hostnames receive
`403 Forbidden`.

---

## Troubleshooting

Common issues and how to fix them:

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| `r-shell: command not found` | The binary is not on your `PATH` | Move it to `/usr/local/bin` (macOS/Linux) or open a **new** terminal after the Windows installer |
| macOS: *“r-shell” cannot be opened because the developer cannot be verified* | Gatekeeper quarantine | `xattr -dr com.apple.quarantine /usr/local/bin/r-shell`, or **System Settings → Privacy & Security → Open Anyway** |
| Windows SmartScreen blocks the installer | Unsigned installer | Click **More info → Run anyway** |
| `Host key verification failed` | The server’s key changed (or possible MITM) | If the change is expected, remove the host’s line from `~/.ssh/known_hosts` and reconnect |
| `Permission denied (publickey)` | Wrong key path, key not added to the server, or encrypted key | Check `--key-path`, add your public key to the server’s `~/.ssh/authorized_keys`, and pass `--passphrase` if the key is encrypted |
| `Connection refused` / timeout | Wrong host/port, firewall, or sshd down | Verify `--host`/`--port`, security-group/firewall rules, and that `sshd` is running |
| `ls` / `stats` print nothing useful | Target is not Linux | These rely on GNU `ls` and `/proc`; use `exec` for non-Linux POSIX hosts |
| AI client can’t reach MCP | Server not running or wrong URL | Run `r-shell mcp` and use exactly `http://127.0.0.1:9123/mcp` (loopback only) |

Still stuck? Run any command with `--help`, or open an issue on the
[GitHub repository](https://github.com/MageGojo/r-shell-cli/issues).

---

## FAQ — Frequently Asked Questions

_常见问题(中文)：R-Shell 是什么？怎么和 AI 助手 / MCP 对接？为什么比裸 `ssh` 适合 AI Agent？支持哪些系统？怎么用 SSH 密钥？安全吗？——下方英文 FAQ 逐条解答。_

**What is R-Shell?**
R-Shell is an open-source, AI-native SSH tool written in Rust. Its primary purpose
is to act as an **MCP (Model Context Protocol) server** so AI assistants can
operate remote servers safely; it is also a single-binary SSH client that manages
saved connections, runs remote commands, opens interactive PTY shells, transfers
files over SFTP, and snapshots remote system stats.

**How do I use R-Shell with an AI agent / MCP client?**
Run `r-shell mcp` and add `http://127.0.0.1:9123/mcp` to your MCP client (e.g.
Cursor’s `~/.cursor/mcp.json` or Claude Desktop). The agent then opens a
persistent SSH session and uses tools like `ssh_exec`, `ssh_read_file`, and
`ssh_write_file`. See [R-Shell for AI Assistants](#r-shell-for-ai-assistants-mcp).

**Why is R-Shell better than letting an AI agent call raw `ssh`?**
R-Shell keeps a single SSH session alive across tool calls, returns structured
results, never leaks credentials, and binds to localhost only. Raw `ssh`
subprocesses re-authenticate every step, leak secrets into command lines, and can
trip server-side intrusion detection from repeated logins.

**How is R-Shell different from plain `ssh` / `scp`?**
R-Shell unifies `ssh`, `scp`/`sftp`, a system monitor, and an AI integration into
one tool. You save a connection once and reference it by name (`-c prod`) instead
of re-typing host, user, port, and key flags. Output is clean and greppable, and
structured `--json` output is available where it helps scripting.

**Is R-Shell free and open source?**
Yes. R-Shell is released under the **MIT License**, so it’s free for personal and
commercial use.

**Which operating systems does R-Shell support?**
Prebuilt downloads are provided for **macOS (Apple Silicon and Intel)** as `.dmg`
images and **Windows x64** as an `.exe` installer. **Linux** and any other
platform can build from source with Cargo. The interactive `shell`, `exec`,
`upload`, and `download` commands work against any POSIX SSH host, while `ls` and
`stats` are tuned for Linux servers.

**Does R-Shell support SSH key (public-key) authentication?**
Yes. Use `--auth publickey` with `--key-path` (and `--passphrase` for encrypted
keys). Password authentication is also supported, with a secure no-echo prompt
when no password is stored.

**Is R-Shell secure?**
Yes. It verifies host keys against `~/.ssh/known_hosts` (trust-on-first-use),
never prints or returns passwords/keys, stores its config with owner-only
permissions, and binds the MCP server to **localhost only** with DNS-rebinding
and cross-origin protections. See [Security](#security).

**Can AI assistants like Cursor or Claude use R-Shell?**
Yes. Run `r-shell mcp` to start a Model Context Protocol server at
`http://127.0.0.1:9123/mcp`. MCP-compatible clients can then list connections and
run commands, read/write files, and reuse a persistent SSH session across calls.
See [MCP Integration](#mcp-integration-ai-assistants).

**Where does R-Shell store my saved connections?**
In a local `workspace.json` under your platform’s data directory (for example,
`~/Library/Application Support/r-shell/` on macOS). See
[Data & Configuration](#data--configuration).

**How do I install R-Shell on macOS from the DMG?**
Download the matching `.dmg`, mount it, copy `r-shell` to `/usr/local/bin`, run
`xattr -dr com.apple.quarantine /usr/local/bin/r-shell`, then run
`r-shell --version`. Full steps are in
[Download & Install](#macos--install-from-a-dmg).

**How do I update R-Shell?**
Download the newest `.dmg` / installer from the
[Releases page](https://github.com/MageGojo/r-shell-cli/releases/latest) and
reinstall, or rebuild from source with `git pull` + `cargo build --release`.

---

## Development

The root `package.json` is a thin wrapper around Cargo:

```bash
pnpm dev          # cargo run   (shows --help)
pnpm run check    # cargo check
pnpm test         # cargo test
pnpm run build    # cargo build
pnpm run fmt      # cargo fmt
```

Equivalent direct Cargo commands:

```bash
cargo run   --manifest-path cli/Cargo.toml -- --help
cargo check --manifest-path cli/Cargo.toml
cargo test  --manifest-path cli/Cargo.toml
cargo build --manifest-path cli/Cargo.toml
```

### Version bumping

```bash
pnpm run version:patch
pnpm run version:minor
pnpm run version:major
```

These update `package.json`, `cli/Cargo.toml`, `cli/Cargo.lock`, and
(unless skipped) `CHANGELOG.md`.

---

## Project Structure

```text
r-shell/
├── cli/                        # the CLI crate (binary name: r-shell)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             # CLI entry, clap commands, output formatting
│       ├── model.rs            # persisted workspace & connection models
│       ├── storage.rs          # local workspace persistence (workspace.json)
│       ├── ssh.rs              # SSH, PTY, SFTP implementation (russh)
│       ├── ssh/tests.rs        # SSH / key handling tests
│       ├── native_backend.rs   # connection manager
│       ├── monitor.rs          # remote system resource monitor
│       └── mcp.rs              # local MCP Streamable HTTP server
├── packaging/                  # release packaging
│   ├── macos/create_dmg.sh     # builds the macOS .dmg
│   └── windows/r-shell.nsi     # NSIS installer script (Windows .exe)
├── .github/workflows/          # CI: tests + multi-platform release builds
├── scripts/                    # version bump helpers
└── package.json                # thin command wrapper around Cargo
```

---

## Security

- Server host keys are verified against `~/.ssh/known_hosts` (trust-on-first-use);
  a changed key aborts the connection unless `--insecure` is passed.
- Passwords, private keys, and passphrases are **never** printed or returned by
  MCP calls — list responses only expose `has_password` / `has_private_key_path`
  booleans.
- Password prompts do not echo input.
- `workspace.json` and its directory are created with owner-only permissions
  (`0600` / `0700` on Unix) so other local users cannot read stored credentials.
- The MCP endpoint binds to **localhost only**. Requests must carry a loopback
  `Host` header (defeating DNS-rebinding) and, if an `Origin` is present, it must
  be a loopback origin. A `null` or cross-site `Origin` is rejected.

---

## License

MIT. See [LICENSE](LICENSE).
