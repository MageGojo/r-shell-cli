# R-Shell GUI 技术选型对比

> 状态:**已决策 → A. Flutter**(2026-06-20)。详见 [架构设计](02-架构设计.md) 与 [进度表](进度.md)。
> 说明:综合评分最高的是 Tauri(57),但维护者选择 **Flutter**——与本项目「桌面默认 Flutter + 调用 Rust 核心」规则一致,UI 体验与跨平台统一性更优。本文保留三方案对比作为决策依据存档。
> 目标读者:维护者 / 决策者

## 0. 背景与目标

R-Shell 现在是一个**纯 Rust 的单二进制 CLI + MCP 服务器**(`cli/` crate,二进制名 `r-shell`),README 里明确写了「它不是 GUI 应用」。本次目标是给它**补一个桌面 GUI**,且 v1 就要**完整对标 CLI 的全部能力**:

| 能力 | 对应 CLI | GUI v1 形态 |
| --- | --- | --- |
| 连接管理(保存/编辑/分组) | `connections` | 侧边栏连接树 + 增删改表单 |
| 交互式终端(PTY) | `shell` | **多标签终端**(核心,最难) |
| 远程命令执行 | `exec` | 终端内 / 快捷命令面板 |
| 文件管理(SFTP) | `ls` / `upload` / `download` | 双栏文件浏览器 + 拖拽传输 |
| 服务器监控 | `stats` | 实时仪表盘(CPU/内存/磁盘/网络) |
| MCP 服务器 | `mcp` | 一键开关 + 状态/端口/工具列表 |

平台:**macOS(Apple Silicon + Intel)、Windows、Linux**,与现有 CLI 一致。

> 本文档**只负责选型**,不写实现。选定后再写架构设计文档与分阶段任务。

---

## 1. 共同前提:先把可复用核心抽成 `core` 库(三种方案都要做)

无论最终选哪种 GUI 栈,**第一步都一样**:现在的逻辑全堆在 `cli/`(binary crate)里,GUI 没法直接复用。需要把与界面无关的核心抽成一个 **library crate**,让 `cli` 和 `gui` 共享、同源演进。

建议的重构后结构(workspace):

```text
r-shell/
├── Cargo.toml              # [workspace] members = ["core", "cli", "gui"...]
├── core/                   # 新增:r-shell-core(纯逻辑库,无 UI 无 clap)
│   └── src/
│       ├── lib.rs
│       ├── ssh.rs          # ← 从 cli 迁移:SSH/PTY/SFTP(russh)
│       ├── model.rs        # ← 迁移:工作区 & 连接模型
│       ├── storage.rs      # ← 迁移:workspace.json 持久化
│       ├── native_backend.rs # ← 迁移:连接管理
│       ├── monitor.rs      # ← 迁移:监控采集
│       └── mcp.rs          # ← 迁移:MCP 服务(可被 GUI 内嵌启动)
├── cli/                    # 保留:仅留 clap 解析 + 输出格式化,逻辑 use r_shell_core
└── gui/ (或 desktop/)      # 新增:GUI 外壳,依赖 core
```

收益:CLI / GUI / MCP **同一套 SSH 与连接逻辑**,改一处全受益;符合本项目「组件化」规范。这一步**风险低、价值高**,可独立先做(不阻塞选型),建议作为 GUI 工程的第 0 阶段。

---

## 2. 候选方案

| 方案 | UI 层 | 与 Rust 核心的关系 | 终端组件 |
| --- | --- | --- | --- |
| **A. Flutter + Rust 核心** | Flutter(Dart) | `flutter_rust_bridge` FFI 调用 `core` | `xterm.dart` |
| **B. Tauri v2** | Web 前端(React/Vue/Svelte) | Rust 后端**直接就是** `core`,加 `#[tauri::command]` | `xterm.js` |
| **C. 纯 Rust GUI** | egui / iced / slint | 同进程直接调用 `core`,无 FFI | `egui_term` 或内嵌 `alacritty_terminal` |

> 注:**A 是本项目「桌面默认用 Flutter + 调用 Rust 核心」规则的方案**;B/C 是因为本项目核心本身就是 Rust 而格外契合的备选。

---

## 3. 逐维度对比

### 3.1 终端模拟器成熟度(本应用最关键 ⭐)

SSH 客户端的体验上限基本由终端组件决定(VT100/256 色、宽字符、鼠标、复制粘贴、resize、性能)。

- **Tauri / xterm.js**:`xterm.js` 是业界事实标准(VS Code 集成终端、Hyper、众多 Web SSH 都用它),功能与兼容性最强。**最成熟**。
- **Flutter / xterm.dart**:`xterm.dart`(TerminalStudio)成熟可用,Flutter SSH App 常用;能力略逊 xterm.js 但足够。需把 `core` 的 SSH PTY 字节流通过 bridge 喂给它。**成熟**。
- **纯 Rust**:`egui_term` 还年轻;更稳的做法是内嵌 `alacritty_terminal`(VTE 解析后端)再自绘——**工作量明显更大、最不成熟**。

### 3.2 复用现有 Rust 核心

- **Tauri**:后端**就是** Rust,`core` 直接 `use`,加几个 command 即可,**零 FFI 摩擦,复用度最高**。
- **纯 Rust**:同语言直调,**也无 FFI**,但 UI 代码得自己写。
- **Flutter**:经 `flutter_rust_bridge` 生成绑定,成熟但**多一层 FFI/类型映射与 codegen 维护**;异步流(PTY 输出、stats 推送)需走 bridge 的 stream。

### 3.3 跨平台与打包

- **Tauri**:内置 bundler,一键产出 `.dmg / .nsis(exe) / .AppImage / .deb`;用系统 WebView(macOS WKWebView、Win WebView2、Linux WebKitGTK)。**打包最省事**。
- **Flutter**:`flutter build macos/windows/linux` 成熟;现有 `packaging/`(DMG/NSIS)可改造复用。
- **纯 Rust**:`cargo build` 跨平台没问题,但**打包要手搓**(可复用仓库现有 DMG/NSIS 脚本)。

### 3.4 仪表盘 / 表格 / 文件树 等 UI 生态

监控图表、SFTP 文件表格、连接树都需要现成组件。

- **Tauri**:整个 Web 生态(图表库、数据网格、虚拟列表)最丰富。
- **Flutter**:widget 生态丰富(`fl_chart`、DataTable 等),体验好。
- **纯 Rust**:`egui_extras`(表格)、`egui_plot`(图表)够用但**偏朴素**,复杂交互要自己堆。

### 3.5 包体积 / 资源占用

- **纯 Rust(egui)**:**最小**,单二进制、GPU 渲染、内存低。
- **Tauri**:二进制小(复用系统 WebView),内存中等(取决于 WebView)。
- **Flutter**:打包**最大**(内置 Skia 引擎,几十 MB 起),性能好。

### 3.6 开发效率 / 生态 / 招聘

- **Tauri**:Web 前端迭代快、人最好招;但引入 JS/TS 工具链。
- **Flutter**:hot reload、生态大、跨端经验可迁移;引入 Dart 工具链。
- **纯 Rust**:**单语言、单工具链**,但即时模式 UI 写复杂界面较费劲、组件少。

### 3.7 长期与 CLI 同源演进

- **纯 Rust**:100% Rust,一套工具链,`core` 共享最自然。
- **Tauri**:后端与 CLI 同为 Rust crate,**易保持同步**;前端是独立 Web 工程。
- **Flutter**:UI(Dart)与核心(Rust)双语言,需持续维护 bridge。

---

## 4. 加权评分

权重按本应用的重要性设定(终端体验与核心复用最关键):

| 维度 | 权重 | A. Flutter | B. Tauri | C. 纯 Rust |
| --- | :-: | :-: | :-: | :-: |
| 终端模拟器成熟度 | ×3 | 4 | **5** | 2 |
| 复用现有 Rust 核心 | ×2 | 4 | **5** | **5** |
| 跨平台 + 打包 | ×2 | **5** | **5** | 3 |
| UI 生态(图表/表格/树) | ×1 | **5** | **5** | 3 |
| 包体积 / 资源占用 | ×1 | 3 | 4 | **5** |
| 开发效率 / 生态 | ×1 | **5** | **5** | 3 |
| 与 CLI 长期同源演进 | ×2 | 3 | 4 | **5** |
| **加权总分(满分 60)** | | **49** | **57** | **43** |

> 评分为相对判断,用于排序而非绝对值。终端成熟度给了最高权重(×3),因为它是 SSH GUI 的体验天花板。

---

## 5. 三方案速览(优缺点)

### A. Flutter + Rust 核心(flutter_rust_bridge + xterm.dart)
- ✅ 本项目桌面默认栈;UI 精美、生态大、跨平台一致;`xterm.dart` 可用。
- ✅ 现有 DMG/NSIS 打包经验可迁移。
- ⚠️ 多一层 FFI bridge(codegen + 类型映射 + 流式数据)要维护;包体积最大;双语言。

### B. Tauri v2(Web 前端 + xterm.js)—— 综合得分最高
- ✅ 后端**直接复用** `core`,零 FFI;`xterm.js` 终端最成熟;打包器一键多平台;Web 生态最丰富。
- ✅ 与 CLI 后端同源(都是 Rust crate),易同步。
- ⚠️ 引入 Web 前端工具链(Node/TS);依赖系统 WebView(各平台行为有细微差异);团队需要点前端。

### C. 纯 Rust GUI(egui / iced / slint)
- ✅ 单语言单工具链、包最小、与 `core` 复用最自然、内存最低。
- ⚠️ **终端组件最不成熟**(要么用年轻的 `egui_term`,要么自己集成 `alacritty_terminal` 自绘);复杂 UI(文件树/仪表盘)做精致较费劲。

---

## 6. 推荐结论

- **综合最优:B. Tauri v2。** 对「Rust 核心 + 必须有成熟终端 + 全平台打包 + 完整 CLI 对标」这组诉求,Tauri 在最关键的两点(`xterm.js` 终端成熟度、直接复用 Rust 核心零 FFI)上都拿满分,打包也最省事,得分最高(57/60)。
- **规则对齐备选:A. Flutter。** 它是本项目「桌面默认 Flutter + 调用 Rust 核心」规则的标准方案,UI 体验好、跨平台一致;代价是多维护一层 bridge、包体积更大。若更看重「与团队其它 Flutter 桌面项目统一栈」,选 A。
- **轻量/极客备选:C. 纯 Rust。** 适合「就要单二进制、最小依赖、全 Rust」的取向;但要接受终端体验和复杂 UI 的成熟度折扣。

> 一句话:**想要最稳的终端 + 最省的核心复用与打包 → Tauri;想和项目 Flutter 桌面栈统一 → Flutter;想极致单二进制/全 Rust → egui。**

---

## 7. 需要你拍板的点

1. **技术栈**:A. Flutter / B. Tauri(推荐) / C. 纯 Rust(egui)。
2. (若选 B)前端框架偏好:React / Vue / Svelte(默认建议 **React + TypeScript**,生态与组件最全)。
3. **是否先做第 0 阶段「抽 `core` 库」**:建议先做(低风险、不阻塞,三方案通吃)。

---

## 8. 选定后的统一下一步(任一方案都适用)

1. 把 `cli/` 重构为 `workspace`,抽出 `core` 库,`cli` 改为依赖 `core`,跑通 `cargo test` 保证零回归。
2. 起 `gui/` 工程脚手架(按选定栈),打通「连接列表读取」最小闭环验证核心复用。
3. 按 v1 范围依次落地:连接管理 → 多标签终端 → SFTP 文件管理 → 监控仪表盘 → MCP 开关。
4. 接入打包(DMG / NSIS / AppImage),补 GUI 的 CI。

> 决策确定后,我会更新[进度表](进度.md)并产出对应的架构设计文档(`02-架构设计.md`)。
