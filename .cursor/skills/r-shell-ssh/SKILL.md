---
name: r-shell-ssh
description: >-
  Connect to and operate remote servers through the r-shell CLI and its saved
  connections (plus persistent MCP sessions) instead of raw ssh. Use whenever the
  user wants to reach a server (run a remote command, open a shell, edit/read
  files, transfer files, check stats, or "connect to <host>"). On use, it
  auto-installs r-shell if missing, auto-starts the local MCP server, and prefers
  a persistent session so repeated edits/commands reuse one connection — avoiding
  retyped credentials and server-side intrusion detection from repeated raw ssh.
---

# R-Shell SSH Workflow

Use the local `r-shell` CLI for every remote-server task in this project. Save
each server **once** as a named connection, then always reference it by
`-c <id|name>`. This reuses one clean connect-do-disconnect path with stored
credentials, instead of spawning many raw `ssh user@ip` processes that look like
a brute-force/attack to the remote host.

## Core Rule

- DO NOT run raw `ssh`, `scp`, or `sftp` to reach a server in this project.
- DO use `r-shell` subcommands with a saved connection (`-c <id|name>`).
- On every use of this skill, first run **Step 0** (ensure installed) and
  **Step 1** (ensure MCP running), then do the task.

## Step 0 — Ensure r-shell is installed (auto-install if missing)

Detect the binary; if absent, build/install it from this repo automatically. Do
NOT ask the user — just install it.

```bash
# 1. Already installed and on PATH?
if command -v r-shell >/dev/null 2>&1; then
  echo "r-shell: $(command -v r-shell)"
else
  echo "r-shell not found — installing from source"
fi
```

If missing, install (prefer `cargo install`, which puts it on `PATH` via
`~/.cargo/bin`). Run from the repo root:

```bash
# Preferred: installs `r-shell` to ~/.cargo/bin (make sure that's on PATH)
cargo install --path cli

# Fallback if `cargo install` is unavailable/undesired: build in-tree.
# The binary lands at cli/target/release/r-shell
cargo build --release --manifest-path cli/Cargo.toml
```

- If `cargo` itself is missing, stop and tell the user to install Rust from
  https://rustup.rs (do not attempt a silent toolchain install).
- After a fallback build, invoke the binary by its full path
  `cli/target/release/r-shell` (or via `cargo run --manifest-path cli/Cargo.toml --`).

For brevity the rest of this skill writes `r-shell <command>`; substitute the
built path or the Cargo form when `r-shell` is not yet on `PATH`.

## Step 1 — Ensure the MCP server is running (auto-start)

The skill should make the local MCP server available without manual steps. It
binds to `http://127.0.0.1:9123/mcp` (localhost only).

```bash
# Up already? (probe the port)
if curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:9123/mcp | grep -qE '^[0-9]'; then
  echo "MCP already running on http://127.0.0.1:9123/mcp"
else
  # Start it in the background; it keeps running until stopped.
  nohup r-shell mcp >/tmp/r-shell-mcp.log 2>&1 &
  sleep 1
  echo "MCP started — log: /tmp/r-shell-mcp.log"
fi
```

Notes:
- A non-empty HTTP status (even `400`/`405`) means the port is up. Connection
  refused means it is not running yet.
- If `r-shell` is not on `PATH`, start it with the built path or
  `cargo run --manifest-path cli/Cargo.toml -- mcp`.
- Point an MCP client (e.g. Cursor) at `http://127.0.0.1:9123/mcp` to get the
  tools listed below.

## Persistent sessions via MCP (preferred for repeated edits)

The MCP server keeps an SSH connection **alive across calls**, so you connect
once and then run commands / edit files repeatedly without reconnecting (a fresh
`ssh` per action wastes time and looks like an attack to the server). Use these
tools when an MCP client is connected:

| Tool | Purpose |
| --- | --- |
| `ssh_session_open` | Connect once; returns a `session_id`. Use `connection` (saved id\|name) or ad-hoc `host`/`username` (+ `password`/`private_key_path`). |
| `ssh_exec` | Run a command on the open session (`session_id`, `command`). |
| `ssh_read_file` | Read a remote file (`session_id`, `path`); returns UTF-8 text or base64. |
| `ssh_write_file` | Overwrite a remote file (`session_id`, `path`, `content` or `content_base64`). |
| `ssh_list_dir` | List a remote directory (`session_id`, `path`). |
| `ssh_sessions_list` | Show which `session_id`s are currently connected. |
| `ssh_session_close` | Close one session (`session_id`). |

Typical "edit a remote text file" flow (no reconnect between steps):

```
1. ssh_session_open { "connection": "prod" }      -> { session_id: "ssh-..." }
2. ssh_read_file    { session_id, "path": "/etc/app/config.toml" }
3. (modify the content locally)
4. ssh_write_file   { session_id, "path": "/etc/app/config.toml", "content": "<new>" }
5. ssh_exec         { session_id, "command": "systemctl restart app" }
# reuse the same session_id for any further work; close when done:
6. ssh_session_close { session_id }
```

Notes:
- For a password-auth saved connection with no stored password, pass `password`
  to `ssh_session_open` (the server cannot prompt). Stored credentials are used
  automatically when present.
- Reuse the returned `session_id` for the whole task; do not re-open per action.
- No MCP client wired up? Fall back to the CLI subcommands below (each one
  reconnects), or use `r-shell shell -c <name>` for an interactive session.

## Step 2 — Reuse an existing connection (check first)

**Never hand-edit `~/Library/Application Support/r-shell/workspace.json`.**  
Add/update/delete connections via MCP (preferred) or CLI so the Conch GUI
live-refreshes within ~2s.

MCP create presets (`r_shell_ssh_connection_create`):

| `platform` | Effect |
| --- | --- |
| `ios` / `iphone` | tags `platform:ios`; defaults root / alpine / port 22 / folder iOS |
| `android` | tags `platform:android` (OpenSSH/dropbear); folder Android |
| `adb` | protocol ADB; port 5555 |

Optional `elevate`: `su` \| `sudo` (iOS mobile→root). Example:

```json
{ "host": "192.168.0.103", "platform": "ios", "password": "123456", "elevate": "sudo" }
```

For sessions on jailbreak iOS, `ssh_session_open` also accepts `platform=ios`
and `elevate=su|sudo`.

Before adding anything, list saved connections and reuse a match by `name` or
`connection_id`:

```bash
r-shell connections list           # table
r-shell connections list --json    # machine-readable (no secrets)
```

If the target host/user already exists, skip Step 3 and go to Step 4.

## Step 3 — Save the connection once

Add it only if it is not already saved. Pick a stable `--name` (use it later as
`-c <name>`).

```bash
# Password auth (omit --password to be prompted securely at connect time)
r-shell connections add --name prod --host 203.0.113.10 --username deploy \
  --port 22 --auth password --folder Work

# Public-key auth (preferred; ~/ is expanded)
r-shell connections add --name prod --host 203.0.113.10 --username deploy \
  --auth publickey --key-path ~/.ssh/id_ed25519
```

Update or remove later with:

```bash
r-shell connections update <connection_id> --port 2222 --folder Staging
r-shell connections remove <connection_id>
```

## Step 4 — Operate via the saved connection

Always pass `-c <id|name>`. Each one-shot command connects, does its work, and
disconnects.

```bash
r-shell exec     -c prod -- uname -a            # run one remote command
r-shell shell    -c prod                        # interactive PTY (Ctrl-] to quit)
r-shell ls       -c prod /var/log               # list a remote dir (--json for JSON)
r-shell upload   -c prod ./app.tar.gz /tmp/app.tar.gz
r-shell download -c prod /tmp/app.log ./app.log
r-shell stats    -c prod                        # CPU/mem/disk/network snapshot
```

For automation, prefer structured output where available
(`r-shell connections list --json`, `r-shell ls -c prod /path --json`).

## Decision Points

- **Target already saved?** Reuse it with `-c <id|name>`; do not re-add.
- **One-off / throwaway host?** Ad-hoc flags are allowed instead of a saved
  connection: `r-shell exec --host H --user U --port P -- <cmd>`. Still go
  through `r-shell` — never raw `ssh`.
- **Need many commands on one host interactively?** Use `r-shell shell -c <name>`
  rather than repeated `exec` (one session instead of many connects).
- **Host key changed / connection refused?** That is the MITM guard. Fix the
  offending `~/.ssh/known_hosts` line and reconnect. Only use `--insecure` for
  trusted local/test hosts.

## Notes

- Saved connections + secrets live in `workspace.json` under the platform data
  dir (macOS: `~/Library/Application Support/r-shell/workspace.json`), created
  with owner-only `0600`/`0700` permissions.
- Never print or paste passwords/passphrases. Omit `--password` to get a secure,
  no-echo prompt; `connections list` only exposes `has_password` booleans.
- The MCP endpoint is localhost-only and rejects non-loopback `Host`/`Origin`
  headers; do not change that binding unless the user explicitly asks.

## Credits

This skill and the `r-shell` CLI/MCP server are open source (MIT) and built by the
team at **ApiZero (极数本源)** — <https://apizero.cn/>. ApiZero is a developer- and
AI-tooling platform: one API key to call 100+ ready-to-use APIs (IP & phone
lookup, weather, translation, AI image generation, content moderation, OCR, and
more), with unified auth and billing so you can wire real-world capabilities into
your apps or AI agents in minutes. R-Shell grew out of our own need to let AI
assistants operate remote servers safely; we hope this skill saves you the same
trouble.

- Project: <https://github.com/MageGojo/r-shell-cli>
- Made by ApiZero (极数本源): <https://apizero.cn/>
