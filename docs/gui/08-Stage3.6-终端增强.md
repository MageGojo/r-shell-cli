# Stage 3.6:终端增强(命令高亮 · 历史命令 · 终端内容查询)

> 归属 Stage 3(多标签 PTY 终端)的体验增强。断点续作请先读 [进度.md](进度.md)。
> 最后更新:2026-06-22

## 1. 需求(用户原话)

> 优化一下,支持命令高亮,支持历史命令查询,支持终端命令内容查询
> (场景:我安装宝塔,没有记住它的密码,这个时候只有历史命令有)。

拆成三个独立特性:

1. **命令高亮**:把命令按 shell 语法着色(命令名 / 参数 / 选项 / 字符串 / 管道重定向 / 变量 / 注释)。
2. **历史命令查询**:记录本会话里用户敲过的命令,提供可过滤的列表,点击可回填到终端重跑 / 复制。
3. **终端内容查询**:在终端**回滚缓冲区(scrollback)**里做全文搜索 —— 这正是「宝塔装完把面板地址 / 账号 / 密码打印到输出里,过后忘了」的解法:搜 `password` / `宝塔` / `panel` 即可在历史输出里定位到那几行。

## 2. 关键事实与取舍

- **xterm.dart 4.0.0 没有内置搜索**(主题里虽有 `searchHit*` 配色,但无搜索实现);但提供了我们需要的底层能力:
  - `terminal.buffer.lines[i].getText()` / `terminal.buffer.getText()` 读纯文本;
  - `terminal.buffer.createAnchor(x, y)` 生成**随行滚动而稳定的锚点**(行被挤出缓冲即 `attached=false`,高亮自动跳过,零悬挂);
  - `TerminalController.highlight(p1, p2, color)` 按 `BufferRangeLine` 着色,列区间 **`[startCol, endCol)`**(end 独占);
  - `TerminalView(controller:, scrollController:)` 可外注我们自管的 controller / 滚动控制器(用于高亮 + 跳转滚动)。
- **宽字符(CJK)列映射**:`getText()` 把一个宽字会折叠成 1 个字符,但终端占 **2 个 cell 列**。为让高亮对齐,按行构建 `字符索引 → cell 列` 映射(逐 cell 读 codepoint/width,跳过 codepoint==0 的续格)。匹配在「字符串」上做,落色时换算成 cell 列。
- **命令高亮 ≠ 终端内实时高亮**:交互式 PTY 里**输入行的实时着色由远端 shell 负责**(PSReadLine / zsh-syntax-highlighting / fish),GUI 端不抢它的活、也无法可靠插手。本端的「命令高亮」作用于**历史命令列表 / 搜索回显**等我们自己渲染的文本。这点在 UI 上不误导用户。
- **历史命令捕获**:从 `terminal.onOutput`(用户键入流)里**最佳努力**还原命令行 —— 累积可见字符,遇 `\r`/`\n` 收尾成一条命令,处理退格 `\x7f`/`\b`、Ctrl-C `\x03` 丢弃整行、Ctrl-U `\x15` 清行、Ctrl-W `\x17` 删词、跳过转义序列(方向键等 `\x1b[...`)。
  - **已知局限**:用 ↑ 方向键从远端 shell 历史召回的命令来自「远端回显」而非本端输入,不计入;Tab 补全结果同理。够覆盖「手敲命令」的主场景,余量记技术债。
- **回滚容量**:`maxLines` 5000 → **10000**,让「装完软件过一会儿再回来搜密码」更可能命中(纯内存、无持久化;重启即清)。

## 3. 组件(全部新增在 `lib/features/terminal/`)

| 文件 | 职责 | 纯逻辑可测 |
| --- | --- | --- |
| `command_highlighter.dart` | shell 命令行 → `List<TextSpan>`(分词 + 着色) | ✅ `tokenizeCommand()` |
| `command_history.dart` | `CommandLineAccumulator`(从输入流抽命令)+ `CommandHistory`(去重/容量/过滤的 ChangeNotifier) | ✅ accumulator |
| `terminal_search.dart` | `TerminalSearch`(ChangeNotifier):over `Terminal`+`TerminalController`+`ScrollController`,查询/命中/高亮/上一处下一处/滚动到命中;纯匹配 `findMatchesInLine()` | ✅ 匹配函数 |
| `widgets/terminal_toolbar.dart` | 终端工具栏:搜索框(可折叠)+ 命中导航 + 大小写开关 + 历史开关 + 复制全部 + 清屏 | UI |
| `widgets/command_history_panel.dart` | 右侧停靠的历史面板:过滤框 + 高亮命令行 + 点击回填 / 复制 | UI |

**改造**:`terminal_session.dart`(持有 `TerminalController`/`ScrollController`/`TerminalSearch`/`CommandHistory`;`_onUserInput` 同时喂给历史;新增 `sendText`;`dispose` 释放)、`terminal_panel.dart`(每标签 = 工具栏 + 终端 + 历史面板;`onKeyEvent` 接 `Ctrl/Cmd+F` 开搜索、`Esc` 关;把 controller / scrollController 注入 `TerminalView`)。

## 4. 验收

`flutter analyze` 0 告警;`flutter test` 通过(原 11 + 新增 highlighter/history/search 用例);macOS 编译运行;真机:连一台机器敲几条命令验证历史与高亮、搜索一段历史输出验证定位 + 跳转滚动。

## 5. 后续(技术债)

- 历史持久化(跨重启,按连接存盘)+ 召回命令捕获;搜索支持正则的 UI 开关已留接口。
- 终端工具栏的「清屏」仅清本地缓冲(不发 `clear` 给远端);后续可加发送选项。
