# Contributing

Thanks for helping improve R-Shell.

## Setup

Install Rust, Cargo, Node.js, and pnpm.

```bash
pnpm dev
```

The command above runs the `r-shell` CLI from `cli/Cargo.toml`.

## Validation

Before opening a pull request or handing work back, run:

```bash
pnpm run fmt
pnpm run check
pnpm test
pnpm run build
```

## Project Layout

```text
cli/src/main.rs            # CLI entry, clap commands
cli/src/model.rs           # persisted data model
cli/src/storage.rs         # local storage
cli/src/ssh.rs             # SSH, PTY, SFTP
cli/src/native_backend.rs  # connection manager
cli/src/monitor.rs         # remote system monitor
cli/src/mcp.rs             # local MCP server
```

## CLI Guidelines

- Keep subcommands focused on single responsibilities.
- Prefer plain, greppable output; add `--json` where structured output helps.
- Never echo secrets; prompt for missing passwords without echo.
- Keep the tool CLI-only — do not reintroduce a graphical UI.

## Security

- Do not print or return passwords, private keys, or passphrases.
- Keep the MCP endpoint bound to localhost unless a change explicitly updates the security model.
- Avoid logging sensitive connection configuration.
