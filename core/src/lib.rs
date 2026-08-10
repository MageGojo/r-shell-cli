//! r-shell-core — UI-agnostic core for R-Shell.
//!
//! All connection, SSH/PTY, SFTP, monitoring, and MCP-server logic lives here so
//! the `r-shell` CLI and the desktop GUI can depend on exactly the same
//! implementation. Nothing in this crate knows about `clap`, a terminal, or any
//! UI toolkit.
//!
//! Modules:
//! - [`model`]          — persisted workspace & connection data models
//! - [`storage`]        — local `workspace.json` persistence
//! - [`connections`]    — connection CRUD service (shared by CLI & GUI)
//! - [`pty`]            — backend-agnostic interactive PTY session handle
//! - [`ssh`]            — SSH client, PTY sessions, SFTP (russh)
//! - [`adb`]            — ADB client for Android devices (`adb` subprocess)
//! - [`adb_bin`]        — locate the `adb` executable (bundled-first, PATH/SDK fallback)
//! - [`native_backend`] — connection manager over [`ssh`] / [`adb`]
//! - [`monitor`]        — remote system-resource snapshots
//! - [`blockexec`]      — Warp-style "command block" exec helpers (wrap/parse/local)
//! - [`mcp`]            — local MCP (Model Context Protocol) server

pub mod adb;
pub mod adb_bin;
pub mod blockexec;
pub mod connections;
pub mod ios_ssh;
pub mod local;
pub mod mcp;
pub mod model;
pub mod monitor;
pub mod native_backend;
pub mod pty;
pub mod ssh;
pub mod storage;
