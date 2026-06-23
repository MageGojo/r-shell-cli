# Stage 6:MCP 面板 + 设置页（UI 优先）

> 本阶段目标：把设计稿 `design/rshell-ui-04-mcp.png` 落地为可用页面，并补上导航栏「设置」死入口。
> 约定：**先把 UI 页面全部做完，再回头修功能/接深度逻辑**（用户明确要求）。本阶段已把 MCP 起停做成自包含真实能力（不改 core），设置页以「可视 + 轻量本地状态」呈现，持久化/应用留到功能阶段。

## 背景：现状盘点

- 已完成页面：主界面三栏（终端）、SFTP 双栏、连接编辑弹窗、监控侧栏。
- 缺口：
  1. **MCP 面板**（设计稿第 4 张）——`features/mcp/` 不存在、bridge 无 `mcp` API，导航第 5 项「MCP」点击无反应。
  2. **设置页**——`features/settings/` 不存在，导航栏底部「设置」按钮点击无反应。
- core 侧 `mcp.rs` 早已是完整的进程内 axum MCP 服务（`start_mcp_server` / `server_running` / 常量 `MCP_ENDPOINT=http://127.0.0.1:9123/mcp` / `MCP_PORT=9123`），CLI `r-shell mcp` 在用。

## MCP 面板设计（对照设计稿）

布局：导航栏 | 连接树侧栏 | **MCP 内容区（占据中心 + 右侧，监控侧栏隐藏）**。

内容区自上而下：
1. 标题：`MCP 服务器` + 副标题 `(Model Context Protocol)`。
2. **状态卡**：
   - 大号 ON/OFF 开关（开=teal 实色）。
   - 状态文字：`● 运行中` / `○ 已停止`。
   - 指标列：监听地址 `127.0.0.1`、端口 `9123`、运行时长 `Xh Ym`（停止时 `—`）。
   - 操作按钮：运行时 `停止`（红）+ `重启`；停止时 `启动`（teal）。
   - 分隔线下：`MCP 端点` 标签 + 只读端点字段 + 复制按钮。
3. **可用工具**：标题 `可用工具 (Available Tools)` + 右侧工具数量徽标。
   - 列表行：分类图标 | 工具名（等宽）| 中文说明，行间细分隔线。
   - 工具目录来自 bridge `mcp_tools()`（与 core `RShellMcpServer` 的 `#[tool]` 一一对应）。

### bridge：`desktop/rust/src/api/mcp.rs`（不改 core）

| 函数 | 形态 | 说明 |
|---|---|---|
| `mcp_endpoint()` | sync → String | 端点常量 |
| `mcp_port()` | sync → u16 | 端口常量 |
| `mcp_status()` | sync → McpStatusDto | running / host / port / endpoint / uptime_secs / tool_count |
| `mcp_tools()` | sync → Vec\<McpToolDto\> | name / summary / category（静态目录，镜像 core 工具） |
| `mcp_start()` | async → Result | `tokio::spawn(start_mcp_server)`，存 JoinHandle + 起始时刻；spawn 后短延时探测是否秒退（端口占用即报错） |
| `mcp_stop()` | async → Result | `JoinHandle::abort()` 释放端口；清空起始时刻 |

- 运行状态/uptime 由 **bridge 自己的静态 `Mutex<McpState>`** 维护（不依赖 core 的 `MCP_SERVER_RUNNING`，因为 abort 不会回写它）。
- `mcp_restart()` 由 Dart 侧 `stop → start` 组合。

### Flutter：`features/mcp/`

- `mcp_controller.dart`（`ChangeNotifier`）：`refresh()` 读 `mcp_status()`；`start/stop/restart`（busy 态 + 错误）；1s 定时器刷新运行时长；一次性加载 `mcp_tools()`。
- `mcp_panel.dart`：照上面布局渲染；自定义 ON/OFF 大开关；复制端点用 `Clipboard`。

## 设置页设计

布局：导航栏 | 连接树侧栏 | 设置内容区（监控侧栏隐藏）。分区（卡片）：

- **外观**：主题（深色 / 跟随系统，本阶段固定深色）、强调色预览。
- **终端**：字号滑杆、光标样式。
- **监控**：采样间隔（1s / 2s / 5s）。
- **MCP**：随应用自动启动开关（占位）。
- **关于**：版本号、检查更新按钮、文档 / 开源地址。

实现：`features/settings/settings_controller.dart`（`ChangeNotifier`，内存态）+ `settings_panel.dart`。**持久化与「实际生效」留到功能阶段**（采样间隔接入 MonitorController、字号接入终端等）。

## 接线（app_scaffold）

- 导航：`MCP=4 → McpPanel`、`设置=90 → SettingsPanel`；侧栏底部齿轮按钮 → 设置。
- **监控侧栏仅在 连接(0)/终端(1)/监控(3) 显示**；文件(2)/MCP(4)/设置(90) 隐藏，让页面占满（与设计稿一致）。

## 验收 DoD

- `cargo build --workspace` 零警告、`cargo test` 全绿。
- `flutter analyze` 0 告警、`flutter test` 全绿、`flutter build macos` ✓ Built。
- 运行：点 MCP 导航进入面板，开关可真实起停本机 MCP 服务（:9123），端点可复制、工具列表显示；点设置进入设置页，控件可交互。

## 留到功能阶段（不在本阶段修）

- MCP「已连接客户端数」实时统计（需读 `LocalSessionManager` 会话，core 未暴露）。
- 设置项持久化 + 实际生效（采样间隔 / 字号 / 自动启动 / 主题）。
- 既有技术债：`sftp_*` 的 frb 流错误通道、终端密码框交互等（见进度.md「技术债」）。
