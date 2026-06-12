# AGENTS.md - R-Shell

## Project Summary

R-Shell is a Rust-native command-line SSH workspace. The app lives in `cli/` and
is a single `clap`-based CLI binary named `r-shell`. There is no graphical UI.

Core features currently in scope:

- saved SSH connection management (`connections` subcommand)
- password and private-key auth
- one-shot remote command execution (`exec`)
- interactive PTY shell (`shell`)
- remote directory listing (`ls`)
- single-file SFTP upload/download (`upload` / `download`)
- remote system resource snapshot (`stats`)
- local MCP Streamable HTTP server at `http://127.0.0.1:9123/mcp` (`mcp`),
  including persistent SSH sessions (`ssh_session_open`/`ssh_exec`/
  `ssh_read_file`/`ssh_write_file`/`ssh_list_dir`) that reuse one connection
  across calls

## Key Files

| Area | File |
| --- | --- |
| CLI entry, arg parsing, commands | `cli/src/main.rs` |
| Workspace models | `cli/src/model.rs` |
| Local persistence | `cli/src/storage.rs` |
| SSH/PTY/SFTP | `cli/src/ssh.rs` |
| Connection manager | `cli/src/native_backend.rs` |
| Remote system monitor | `cli/src/monitor.rs` |
| MCP server | `cli/src/mcp.rs` |

## Commands

```bash
pnpm dev          # run the CLI (cargo run)
pnpm run check   # cargo check
pnpm test        # cargo test
pnpm run build   # cargo build
pnpm run fmt     # cargo fmt
```

Direct Cargo commands are also fine:

```bash
cargo run --manifest-path cli/Cargo.toml
cargo check --manifest-path cli/Cargo.toml
cargo test --manifest-path cli/Cargo.toml
cargo build --manifest-path cli/Cargo.toml
```

## CLI Direction

`r-shell` is a focused, scriptable command-line tool. Keep it that way:

- Subcommands map to single responsibilities (`connections`, `exec`, `shell`,
  `ls`, `upload`, `download`, `stats`, `mcp`).
- A remote target is either a saved connection (`-c <id|name>`) or ad-hoc
  `--host`/`--user` flags.
- Prefer plain, greppable output; offer `--json` where structured output helps.
- Never echo secrets; prompt for missing passwords without echo.
- One-shot commands connect, do their work, and disconnect within a single
  invocation. `shell` uses crossterm raw mode for an interactive PTY.

## Coding Notes

- Prefer small, focused Rust modules.
- Keep MCP responses free of passwords, private keys, and passphrases.
- Bind MCP only to localhost unless the user explicitly changes the security model.
- Use `cargo fmt`, `cargo check`, `cargo test`, and `cargo build` after meaningful changes.
- Keep the application CLI-only; do not reintroduce any graphical layer.
