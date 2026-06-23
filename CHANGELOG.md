# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Conch desktop GUI: a Flutter front-end backed by the Rust core, with an application icon
- MCP server mode with persistent SSH sessions, plus the open-sourced `r-shell-ssh` skill
- Windows NSIS desktop installer that creates a desktop shortcut
- Empty default workspace on first launch
- Release CI that builds the macOS DMG and the Windows installer

### Changed

- Renamed the project from R-Shell to Conch
- Rewrote the SSH core as a standalone Rust CLI and hardened security
- Switched russh to pure-Rust crypto (dropped the OpenSSL dependency) so Windows builds run with no external runtime
- Rewrote the English and Chinese README files

### Fixed

- `build_win` now locates the VC++ CRT redistributable reliably (recursive search, non-fatal, with a System32 fallback)

## [2.1.0] - 2026-06-04

### Added

- Cache the remote directory tree's expand/collapse state and scroll position, and restore them when switching file browser tabs

### Fixed

- Reduce PTY terminal memory use with credit-based flow control and per-session output limits for stable behavior under heavy I/O
- Resolve conflicts between terminal and application-level keyboard shortcuts; preserve editable shortcut handling across component re-renders
- Fix Windows SFTP upload for paths with Windows-style basenames and folders (#24)

## [2.0.0] - 2026-05-30

### Added

- Open remote files in a dedicated popup window from the file browser

### Changed

- Cache remote OS detection with `OnceCell` for lock-free concurrent access, avoiding redundant SSH round-trips

### Fixed

- Restore keyboard focus to the active terminal when switching tabs (PR #13)
- Always render the PTY terminal scrollbar and correct its visibility logic so it appears when content overflows

## [1.8.0] - 2026-05-21

### Added

- Automatically attempt to reconnect interactive terminal sessions after a connection drop

### Changed

- Improve titlebar drag-region behavior for double-click maximize/restore on non-macOS platforms

### Fixed

- Improve PTY connection-drop handling to reduce manual recovery steps when SSH sessions are interrupted

## [1.7.0] - 2026-05-16

### Added

- Lazy-loading, expandable directory tree panel in the integrated file browser
- Client-side SSH keepalive every 60 seconds (3 missed replies triggers a clean disconnect) to prevent idle-timeout drops

### Changed

- Improve the integrated file browser toolbar layout and action grouping

### Fixed

- Quote paths containing apostrophes, spaces, and other shell special characters in file listing and stat commands
- Show `[SSH session lost. Use right-click → Reconnect]` after a dropped connection instead of silently spawning a fresh shell
- Add cancellation-aware exponential backoff for PTY reconnect; fail fast on permanent errors instead of exhausting retries

## [1.6.0] - 2026-05-08

### Added

- Native macOS menu bar via `NSMenu` with standard application menus (File, Edit, View, Window, Help) and keyboard shortcuts
- Quick Connect shortcut in the Connection Manager for recently used hosts
- Draggable titlebar region so the window can be moved without a traditional title bar
- Native window maximize/restore control in the menu bar

### Changed

- Refresh tab styling for clearer active, hover, and inactive states
- Update scrollbar track and thumb colors for better visibility on dark backgrounds

### Fixed

- Expand `~/` in SSH key paths correctly on Linux, macOS, and Windows
- Normalize Windows-style `\r\n` line endings in private keys before use to fix auth failures

## [1.5.0] - 2026-04-30

### Fixed

- Reconnect actions (tab bar button and right-click menu) now re-authenticate before restarting the PTY instead of reusing the dead SSH connection
  - `handleReconnect` in `App.tsx` dispatches `RECONNECT_TAB` after a successful reconnect to remount the terminal
  - Tab bar Reconnect uses the full backend reconnect path (`onReconnectTab`) instead of a bare state dispatch
  - Right-click Reconnect delegates to `onReconnectTab` from context, with a WebSocket-only fallback outside a provider

## [1.4.0] - 2026-04-24

### Added

- OS detection module (`os_detect`) that detects the remote OS type and distribution (distro, version, package manager) on connect and caches `OsInfo` per connection
- React `ErrorBoundary` around key UI sections to catch render errors gracefully

### Changed

- Distro-aware system monitor: `get_system_stats` uses OS-specific commands for CPU, memory, disk, and uptime, selecting the right variant per distro
- Refactor the WebSocket server for better flow control and connection lifecycle handling
- Clean up formatting and imports across `commands.rs`, `ftp_client.rs`, `sftp_client.rs`, `ssh/mod.rs`, and `lib.rs`

## [1.3.1] - 2026-04-01

### Added

- Multiple simultaneous connections to the same profile, each with an independent session and unique ID and automatic numeric suffixes (e.g. "my-server (2)"); works across SSH, SFTP, and FTP
- "Duplicate Tab" context-menu action for SSH terminals, SFTP, and FTP sessions

### Changed

- Rework the SSH file browser: reducer-based state, integrated transfer-queue UI, OS-native drag-and-drop upload, native file-picker downloads, and removal of the legacy SFTP panel
- Document performance metrics and lightweight positioning in the README

### Fixed

- Resolve "No common key algorithm" failures for RSA-keyed servers by adding `ssh-rsa` host key support for legacy servers
- Add timeout handling during session restoration to prevent indefinite hangs
- Improve terminal dimension validation to prevent layout issues

## [1.2.0] - 2026-03-16

### Added

- CodeMirror-based in-app code editor for remote files: syntax highlighting for 15+ languages, One Dark theme, line numbers, code folding, bracket matching, auto-completion, and search/replace
- RDP and VNC desktop protocol clients integrated as tab types alongside SSH terminals
- Editor tab type tracked in `ActiveConnectionState` so editor sessions restore on restart

### Fixed

- Map `Cmd` to the `Ctrl` equivalent on macOS so layout shortcuts (`Cmd+B/J/M/Z`) work reliably
- Use theme-aware contrast colors for terminal text selection in both light and dark themes
- Clear file browser selection when the context menu closes

## [1.1.0] - 2026-03-01

### Added

- Dual-pane SFTP/FTP file browser with side-by-side local/remote panes, drag-and-drop transfers, and a transfer queue with pause/resume/cancel and progress tracking
- Directory synchronization with one-way and two-way modes, conflict detection/resolution, and dry-run preview
- Recursive directory upload and download via context menu
- "Open in Log Monitor" action from the file browser
- Bookmark bar and breadcrumb navigation with path history in the integrated file browser
- Rebuilt Log Monitor with real-time tailing, configurable refresh, syntax highlighting, filtering, search, and line-range selection
- ESLint v10 setup with type-aware `typescript-eslint` rules, `react-hooks` v7, and `react-refresh`; resolved all existing lint errors

### Changed

- Rewrite the README and welcome screen for the v1.0.0 feature set

### Fixed

- Prevent space-key and IME input loss in the terminal: bail out during IME composition (`isComposing` / keyCode 229), stop calling `preventDefault()` on textarea events in the capture phase, and remove per-keystroke allocations from the `onData` path
- Replace sensitive FTP test credentials with placeholders

## [1.0.0] - 2026-02-28

### Added

- VS Code-style terminal groups: horizontal/vertical splits with keyboard shortcuts, drag-and-drop tabs between groups, recursive grid layout, per-group tab bar with context menu, drop-zone overlays, and layout serialization/restoration
- Reconnect a disconnected session from the terminal tab context menu
- `AGENTS.md` project guide covering architecture, build instructions, conventions, data flow, and key files

### Changed

- Migrate terminal state from a flat tab list to reducer-based terminal groups (`TerminalGroupProvider`, `useTerminalGroups()`, actions `ADD_TAB` / `REMOVE_TAB` / `SPLIT_GROUP` / `ACTIVATE_TAB` / `MOVE_TAB`, localStorage persistence)
- Add `AGENTS.md` and `copilot-instructions.md` for contributor onboarding

### Fixed

- Switch the active group when clicking the terminal output area
- Hide the right sidebar when no terminal is open and polish the welcome screen layout
- Fix tooltip content obscured by the arrow overlay
- Add a padding wrapper to correct FitAddon height measurement in PTY terminals
- Close the WebSocket on disconnection to prevent stale PTY state
- Improve terminal connection lifecycle and UI responsiveness

## [0.7.1] - 2026-02-10

### Fixed

- Add padding to the PTY terminal container for better spacing
- Fix duplicate paste triggered by the copy command

## [0.7.0] - 2026-02-08

### Added

- Auto-update support via the Tauri updater plugin: background checks on startup, manual check from the Help menu, and update notifications
- Terminal context menu with copy, paste, select all, clear, and search
- Terminal search bar with case-sensitive and regex options and result navigation
- File browser sorting by name, size, or modification date, ascending or descending, with current-sort indicators
- Dynamic WebSocket port assignment to avoid conflicts, with a port-retrieval command for the frontend

### Changed

- Rename "session" to "connection" throughout the codebase (`session-storage.ts` → `connection-storage.ts`) with automatic migration from the old format
- Show GPU memory usage in MiB for readability
- Update the README with new screenshots and feature descriptions

## [0.6.4] - 2026-01-29

### Added

- GPU monitoring with multi-GPU selection, real-time usage/memory/temperature, a combined usage-history chart, and automatic detection
- Network interface selection for per-interface bandwidth monitoring (Wi-Fi, Ethernet, etc.)
- Reconnect action on connection tabs with reconnect-count tracking
- Real-time connection status indicators

### Fixed

- Correct anchor tag styling syntax

## [0.6.3] - 2026-01-23

### Added

- Edit existing connections from the connection manager, with form state, loading states, and error handling
- 3-second SSH client connection timeout to avoid indefinite attempts

### Changed

- Rename "Session Manager" to "Connection Manager" across UI labels, tooltips, menus, shortcuts, and settings (component renamed `SessionManager` → `ConnectionManager`)
- Update and activate existing tabs when confirming a connection; hide "Save as session" when editing
- Update the README for the connection manager naming

### Fixed

- Switch from WebGL to canvas renderer when a terminal background image is added so images appear on already-open terminals

## [0.6.2] - 2026-01-17

### Added

- Auto-save resizable panel sizes to localStorage, per panel group

### Changed

- Improve resize-handle cursors and visibility for clearer feedback
- Add `getValidFolders()` to session storage to filter orphaned folders

### Fixed

- Show only valid folders in the connection dialog's folder dropdown
- Use `currentColor` for chart text to support light/dark transitions
- Reset connection state on dialog open/close and improve cancel behavior

## [0.6.1] - 2026-01-10

### Added

- Official Homebrew cask for macOS (`brew install --cask r-shell`) with automated checksum generation and tap updates, supporting Intel and Apple Silicon

### Changed

- Improve the release pipeline: SHA256 checksums for all assets, automated Homebrew tap updates via GitHub Actions, better asset naming, and a new `homebrew-tap` repository with secure token-based dispatch
- Update the README with Homebrew installation instructions and clean up obsolete docs

## [0.6.0] - 2026-01-03

### Added

- Quick Connect dropdown for fast reconnection to recent servers
- Terminal background image support with configurable settings
- Theme management with localStorage persistence across sessions
- UI refinements: updated slider, switch, and scrollbar styling, with dynamic terminal appearance updates

### Changed

- Update `@tauri-apps/api` to 2.9.1 and `@tauri-apps/cli` to 2.9.6
- Improve visual consistency and settings/terminal integration

### Fixed

- Resolve theme persistence issues
- Improve scrollbar rendering
- Sync the settings modal with the terminal display

## [0.5.0] - 2025-12-23

### Added

- Duplicate SSH connection tabs via context menu or the Session menu, with full state persistence, correct ordering across restarts, credential reuse, and chained duplication
- Real-time SSH connection latency monitoring in the system monitor
- Persist resizable panel sizes for the left/right sidebars and bottom panel
- Detailed session-restoration overlay with per-session progress, target host/username, a progress bar, and error reporting
- Cancel in-progress connections cleanly without leaving orphaned connections

### Changed

- More informative session-restoration UI
- Better connection error recovery
- UI polish for connection dialogs and session management

### Fixed

- Connection stability improvements
- Better handling of duplicate session credentials
- Session state persistence edge cases

## [0.4.0] - 2025-11-27

### Added

- SSH key authentication for new and saved connections
- Theme customization for light, dark, and high-contrast layouts
- Command history search
- Multi-language (i18n) support for the core UI
- Plugin system foundations
- Batch command execution across sessions with grouped controls
- Port forwarding utilities for exposing remote services locally

### Changed

- UI polish across session tabs, the system monitor, and the toolbar
- Dependency updates for the frontend, Tauri backend, and terminal utilities

### Fixed

- Stability and connection resiliency improvements for session management

## [0.3.0] - 2025-11-17

### Added

- New features and improvements
- Package updates and dependency optimizations

### Changed

- Codebase refinements and optimizations
- Documentation updates

### Fixed

- Bug fixes and stability improvements

## [0.2.0] - 2025-11-17

### Added

- Enhanced UI components and styling
- Improved session management interface
- Better error handling and user feedback
- Additional terminal customization options

### Changed

- Performance optimizations for terminal rendering
- Improved session state persistence
- Enhanced system monitoring display

### Fixed

- Various bug fixes and stability improvements
- Terminal display issues on some platforms
- File browser navigation edge cases

## [0.1.0] - 2025-10-30

### Added

- Initial release of R-Shell
- Multi-session SSH connection management with a tabbed interface
- Integrated file browser for remote file management
- Real-time system monitoring (CPU, memory, disk, processes)
- Process management with kill support
- Password-based SSH authentication
- Connection profile management (save, load, edit, delete)
- UI built with React 19, TypeScript, and Tailwind CSS
- Rust + Tauri 2 backend
- Responsive resizable panel layout
- Toast notifications
- xterm.js terminal emulator
- Session state persistence

### Technical details

- Frontend: React 19, TypeScript, Vite, Tailwind CSS
- Backend: Rust, Tauri 2.0
- UI components: Radix UI primitives
- Terminal: xterm.js
- Icons: Lucide React
- File browser: custom implementation with SFTP support

### Known issues

- Process list refresh interval is fixed at 5 seconds
- No SSH key authentication yet
- Limited error handling for network interruptions
- Terminal history not persisted between sessions

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for details on how to contribute to this project.

## License

This project is licensed under the MIT License — see [LICENSE](LICENSE) for details.
