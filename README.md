# R-Shell

**R-Shell** is a lightweight, scriptable **command-line SSH workspace** written in
Rust. From a single `r-shell` binary you can manage saved SSH connections, run
remote commands, open interactive shells, transfer files over SFTP, snapshot
remote system stats, and run a local MCP server for AI tools. There is no
graphical UI — everything is a fast, greppable command.

```text
r-shell <command> [options]
```

---

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Quick Start](#quick-start)
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
- [MCP Integration](#mcp-integration)
- [Development](#development)
- [Project Structure](#project-structure)
- [Security](#security)
- [License](#license)

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

## Installation

### From source

Prerequisites: **Rust** and **Cargo** (and optionally **Node.js + pnpm** for the
wrapper scripts).

```bash
git clone <repo-url>
cd r-shell

# Build a release binary
cargo build --release --manifest-path cli/Cargo.toml

# The binary is produced at:
#   cli/target/release/r-shell
```

Copy `cli/target/release/r-shell` somewhere on your `PATH` (e.g.
`/usr/local/bin`) to use `r-shell` directly. The examples below assume it is on
your `PATH`; otherwise run it through Cargo:

```bash
cargo run --manifest-path cli/Cargo.toml -- <command> [options]
```

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

## MCP Integration

The MCP server exposes these tools (credentials are never returned):

| Tool | Description |
| --- | --- |
| `r_shell_ssh_connections_list` | List saved connections (sanitized) |
| `r_shell_ssh_connection_create` | Create a saved connection |
| `r_shell_ssh_connection_update` | Update a saved connection |
| `r_shell_ssh_connection_delete` | Delete a saved connection |
| `r_shell_ssh_tabs_list` | List open tabs (always empty in the CLI) |

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
├── cli/                   # the CLI crate (binary name: r-shell)
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
