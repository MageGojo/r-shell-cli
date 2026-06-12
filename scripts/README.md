# Scripts

Utility scripts for the R-Shell CLI.

## Root Commands

The root `package.json` is only a thin wrapper around Cargo:

```bash
pnpm dev          # cargo run
pnpm run check   # cargo check
pnpm test        # cargo test
pnpm run build   # cargo build
pnpm run fmt     # cargo fmt
```

All commands target `cli/Cargo.toml`.

## Version Bumping

Recommended:

```bash
pnpm run version:patch
pnpm run version:minor
pnpm run version:major
```

Direct usage:

```bash
node scripts/bump-version.mjs patch
node scripts/bump-version.mjs minor --no-commit
node scripts/bump-version.mjs major --skip-changelog
```

The bash variant is also available:

```bash
./scripts/bump-version.sh patch
```

Version bump scripts update:

- `package.json`
- `cli/Cargo.toml`
- `cli/Cargo.lock`
- `CHANGELOG.md`, unless `--skip-changelog` is passed

Use `--no-commit` to update files without creating a commit.
