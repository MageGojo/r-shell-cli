# R-Shell Copilot Instructions

## Project Overview

R-Shell is a Rust-native **command-line** SSH workspace. The entire application lives in `cli/` — there is no graphical UI, web frontend, or Tauri backend. SSH transport, persistence, and the MCP server all run inside a single `clap`-based CLI binary named `r-shell`.

## Architecture

The app is a `clap` CLI (entry point `cli/src/main.rs`). Subcommands map to single responsibilities and reuse a set of UI-free core modules. A Tokio runtime is built per command invocation to drive async SSH/SFTP and the MCP HTTP server.

### Subcommands

| Command | Responsibility |
| --- | --- |
| `connections list/add/update/remove` | Manage saved connections in `workspace.json` |
| `exec` | Run one remote command and print output |
| `shell` | Interactive PTY shell (crossterm raw mode) |
| `ls` | List a remote directory |
| `upload` / `download` | Single-file SFTP transfer |
| `stats` | One-shot remote system resource snapshot |
| `mcp` | Run the local MCP server |

### Module Map (`cli/src/`)

| Module | Responsibility |
| --- | --- |
| `main.rs` | CLI entry, `clap` arg parsing, command handlers, output formatting |
| `model.rs` | Persisted workspace + connection data models (`SavedConnection`, `TerminalTab`, `ConnectionStatus`, `PersistedWorkspace`) |
| `storage.rs` | Local workspace persistence (load/save JSON on disk) |
| `ssh.rs` (+ `ssh/tests.rs`) | SSH, PTY, and SFTP implementation on `russh` / `russh-sftp` |
| `native_backend.rs` | Connection manager (`NativeConnectionManager`, `SshConfig`, `AuthMethod`, remote file types) |
| `monitor.rs` | Remote system monitoring/stats parsing and formatting |
| `mcp.rs` | Local MCP Streamable HTTP server |

### MCP Server

A local MCP Streamable HTTP server is exposed while the app runs, bound to localhost only:

```text
http://127.0.0.1:9123/mcp
```

Tools cover SSH connection management and listing open tabs. **MCP responses must never include passwords, private keys, or passphrases.**

## Development Workflow

The root `package.json` is only a thin wrapper around Cargo.

### Running the CLI

```bash
pnpm dev
# equivalent to:
cargo run --manifest-path cli/Cargo.toml -- --help
```

### Check, Test, Build, Format

```bash
pnpm run check   # cargo check --manifest-path cli/Cargo.toml
pnpm test        # cargo test  --manifest-path cli/Cargo.toml
pnpm run build   # cargo build --manifest-path cli/Cargo.toml
pnpm run fmt     # cargo fmt   --manifest-path cli/Cargo.toml
```

Run `cargo fmt`, `cargo check`, `cargo test`, and `cargo build` after meaningful changes.

### Version Bumping

```bash
pnpm run version:patch   # 2.1.0 → 2.1.1
pnpm run version:minor   # 2.1.0 → 2.2.0
pnpm run version:major   # 2.1.0 → 3.0.0
```

The script updates `package.json`, `cli/Cargo.toml`, `cli/Cargo.lock`, and (unless skipped) `CHANGELOG.md`.

## CLI Direction

`r-shell` is a focused, scriptable command-line tool:

- Subcommands map to single responsibilities (`connections`, `exec`, `shell`, `ls`, `upload`, `download`, `stats`, `mcp`).
- A remote target is either a saved connection (`-c <id|name>`) or ad-hoc `--host`/`--user` flags.
- Prefer plain, greppable output; offer `--json` where structured output helps.
- One-shot commands connect, run, and disconnect within a single invocation; `shell` uses crossterm raw mode for an interactive PTY.

## Conventions

- Prefer small, focused Rust modules.
- Rust structs/enums: PascalCase; modules and functions: snake_case.
- Use `anyhow::Result<T>` for fallible operations.
- Serialize persisted models with `serde`.
- Keep the application CLI-only — do not reintroduce web or Tauri layers.

## Security

- Never print or return passwords, private keys, or passphrases.
- Bind the MCP endpoint to localhost only unless a change explicitly updates the security model.
- Avoid logging sensitive connection configuration.
