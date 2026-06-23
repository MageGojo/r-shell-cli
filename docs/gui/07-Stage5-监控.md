# Stage 5：监控仪表盘(实时系统指标 + 走势图)

> 状态：macOS 编译验证通过 | 上游：[架构设计 02](02-架构设计.md)、[UI 规范 03](03-UI设计规范.md) §3.7、[进度.md](进度.md)
> 决策延续：核心能力复用 `r-shell-core`(CLI/GUI 同源)、bridge 薄封装用 `StreamSink` 单向推流、凭据只在 Rust 侧用、状态延续轻量 `ChangeNotifier`(Riverpod 继续推迟)。

## 1. 目标

把右侧「服务器监控」侧栏从占位(`—` + 「Stage 5 接入」)接成**实时数据**:跟随当前选中连接,周期采样远端资源并渲染:

- **CPU**:大数值百分比 + 面积走势图 + 「N 核 · 负载 X.XX」。
- **内存 / 磁盘**:百分比 + 进度条 + `used/total`(共享单位)。
- **网络吞吐**:↓接收 / ↑发送 速率 + 双线走势图。
- **系统信息**:OS 名 + 运行时长(uptime)。
- 阈值高亮:>80% `warning`、>92% `danger`(色板见 03 §3.7)。

## 2. 为什么 core 零改动

core 早已具备完整且**有单测**的监控能力(`core/src/monitor.rs` + `native_backend.rs`),CLI `stats` 子命令在用:

- `monitor::stats_command()`:一条命令一次往返抓 `/proc/stat`、`/proc/meminfo`、`loadavg`、`uptime`、`/proc/net/dev`、`df -kP /`、`nproc`、`uname`(用 `===RSHELL-SECTION===` 分段,缺失文件自然空)。
- `monitor::parse_snapshot(raw) -> StatsSnapshot`:解析成原始快照(累计计数)。
- `SystemStats::from_samples(prev, cur, elapsed)`:把两次快照**差分**出 CPU% 与网络速率(速率量需要时间差),其余字段直接取 current。
- `native_backend::fetch_system_stats(id)`:在活跃会话上执行 `stats_command()` 拿原始输出。

→ Stage 5 **不动 core**,只在 bridge 周期调用 + 差分 + 推流,Flutter 订阅渲染。零回归。

## 3. 分层设计

### 3.1 bridge(`desktop/rust/src/api/monitor.rs`,新增)

```rust
pub struct SystemStatsDto {            // 与 core SystemStats 一一映射,收敛成 frb 友好类型
    cpu_percent: f64, mem_percent: f64, mem_used_kb: i64, mem_total_kb: i64,
    swap_percent: f64, swap_used_kb: i64, swap_total_kb: i64,
    disk_percent: f64, disk_used_kb: i64, disk_total_kb: i64,
    net_rx_per_sec: f64, net_tx_per_sec: f64, load1: f64, uptime_secs: f64,
    cpu_cores: u32, os: String,
}
impl From<SystemStats> for SystemStatsDto { /* 数值 as 转换 */ }

// 一帧:sample 与 error 互斥(成功带 sample,失败带 error)。
pub struct StatsFrame { sample: Option<SystemStatsDto>, error: Option<String> }

pub async fn stats_stream(connection_id, interval_ms: u32, sink: StreamSink<StatsFrame>)   // 注意:返回 ()
```

`stats_stream` 行为:

- 先 `ensure_session`(复用终端 / SFTP 共享的活跃会话,**绝不另开连接**)。
- 循环:`fetch_system_stats` → `parse_snapshot` → 用上一帧 `from_samples` 差分(`Instant` 实测 elapsed)→ `sink.add(StatsFrame::sample(dto))` → `sleep(interval)`。
- **第一帧** `previous=None`:CPU% 与网络速率为 0(需两帧差分),内存 / 磁盘 / 负载 / 运行时长 / OS 立即可用;第二帧起速率填充。
- `interval_ms` 下限 250ms(防过载;远端命令本身也有耗时)。
- **采集 / 连接失败 → 推 `StatsFrame::error(原因)` 帧 + `previous=None`,`interval` 后自重试**(主机恢复即自动回到 sample),**绝不返回 `Err`**。
- Dart 取消订阅 → `sink.add` 失败 → 退出循环。
- **不主动关闭会话**(终端 / SFTP 可能在用;由终端「最后标签关闭」统一回收)。

> **为什么错误走数据帧而非 `Err`(关键经验)**:frb 对 `async fn(sink: StreamSink<T>) -> Result<(), E>` 生成的 Dart 端是——数据走 `sink.stream`,而**函数的返回值(含 `Err`)走另一个 `unawaited(...)` 的 Future**。所以函数 `return Err(..)` 时,Dart 的 `stream.listen(onError:)` **根本收不到**,该错误最终变成控制台的 **"Unhandled Exception"**(实测:连不可达主机时每隔几秒刷一条未捕获异常)。修法:让 `stats_stream` 永不外抛错误(返回 `()`),把失败封装进 `StatsFrame.error` 走 `sink.stream`,Dart 在 `onData` 里分支处理。(数据枚举需 `freezed`,为省依赖改用 `Option` 字段的普通 struct。)**同款隐患存在于 Stage 4 的 `sftp_upload/download`**——见进度表技术债。

`Cargo.toml` 给 bridge crate 加 `tokio = { workspace = true }`(用 `tokio::time::sleep`,与 core/CLI 同源版本)。`mod.rs` 注册 `pub mod monitor;`。frb codegen 重新生成绑定(`lib/src/rust/api/monitor.dart`)。

### 3.2 Flutter(`desktop/lib/features/monitor/`,新增)

- `stats_repository.dart`:`StatsRepository.stream(id, intervalMs)` 薄封装 frb `statsStream`(UI 不直接碰 frb)。
- `monitor_controller.dart`:`MonitorController extends ChangeNotifier`
  - `setConnection(ConnectionDto?)`:跟随当前选中连接;同 id 幂等不重订阅;切换即取消旧订阅、清历史、重订阅。
  - 持有 `latest` 一帧 + 四条**环形历史**(cpu% / mem% / netRx / netTx,默认 60 点 ≈ 2 分钟)供走势图。
  - `onData(StatsFrame)` 分支:`frame.error != null` → 标错(保留最近一帧,图表停在最后已知值,等 Rust 自重试恢复);否则 `frame.sample` → 更新 + 推历史。
  - 状态机 `idle / connecting / live / error`;**业务失败走帧、Rust 内部自重试**,Dart 端 `onError/onDone` 仅作流本身异常的兜底重订阅(3s);`retryNow()` 手动立即重订阅;用 connectionId 校验丢弃过期回调。
  - 默认 `intervalMs=2000`(设计稿默认 1s「可配」,远端每帧一条 SSH 命令,取 2s 更省)。
- `monitor_format.dart`:`formatBytes / formatRate / formatKbPair / formatUptime / formatPercent` —— 对照 core Rust 版,按项目约定(同 SFTP)把格式化放 Dart 侧;有单测固定行为防漂移。
- `monitor_panel.dart`:`MonitorPanel(controller)`,`ListenableBuilder` 驱动。头部状态药丸(实时/连接中/已断开/未选择);卡片用 `fl_chart` 的极简 `LineChart`(无网格 / 坐标轴 / 触摸,平滑折线 + 顶部渐变填充,网络双线共享 Y 轴);空闲 / 采集中 / 出错有对应横幅(出错带「重试」)。

### 3.3 接入(`app_scaffold.dart`)

- 持有 `MonitorController _monitor`(随 State `dispose`)。
- `_syncMonitor() => _monitor.setConnection(_selected)`:在每个改变选中连接的入口(选中 / 打开终端 / 打开文件 / 新建保存 / 重载)调用。
- 中心右栏从内联 `_monitorPanel()`/`_monitorCard()`(已删除)换成 `MonitorPanel(controller: _monitor)`。

依赖:`flutter pub add fl_chart`(1.2.0)。

## 4. 数据流

```
选中连接 → MonitorController.setConnection
  → StatsRepository.stream(id,2000ms) → frb statsStream
    → [Rust] ensure_session → loop{ fetch_system_stats → parse_snapshot → from_samples(prev) → sink.add → sleep }
      → Stream<SystemStatsDto> → onData: 更新 latest + 推四条环形历史 → notifyListeners
        → MonitorPanel 重建:数值卡 + fl_chart 走势图(阈值着色)
```

## 5. 验收(DoD)

- `cargo build --workspace` 零警告 ✓
- `cargo test --workspace` 32 通过 / 5 ignored ✓(core 未改,全绿)
- `flutter analyze` 0 告警 ✓
- `flutter test` 11 通过 ✓(原 6 + 新增 monitor_format 5)
- `flutter build macos` ✓ Built(82.6MB)
- **`flutter run -d macos` 真机冒烟 ✓**:连真实 Windows 开发机(`shanh@192.168.0.112`),监控面板优雅显示 **「监控已断开 · Command failed with code: Some(1)」**(Windows 无 `/proc`/`df`,Linux 指标命令非零退出)+「重试」按钮;**控制台 0 条 Unhandled Exception**(修复前每隔几秒刷一条,见 §3.1)。证明错误降级链路正确;Linux 真机的 live 数据/图表待连可达 Linux 机补。

## 6. 已知限制 / 后续

- **指标命令是 Linux 专用**(`/proc` + `df`)。连**非 Linux**主机(如 Stage 3.5 的 Windows OpenSSH):命令非零退出 → `fetch_system_stats` 报错 → 面板显示 **「监控已断开 · Command failed…」错误态**(优雅降级,非崩溃)。Windows/macOS 远端监控需另写采集命令(`Get-Counter`/`top` 等),后续阶段补。
- **真机端到端(Linux)的 live 数据/图表验证待补**:错误降级链路已在真实 Windows 机验证;成功取数逻辑与 CLI `stats` 同源(后者已在真实主机取到 OS/CPU/RAM/磁盘/Uptime),复用已开会话。等有可达 Linux 服务器时补 live 截图。
- **采样间隔不可在 UI 配置**:目前固定 2s(代码可调)。Stage 7 设置页再暴露。
- **历史不持久**:切换连接 / 重启清空(监控是瞬时量,无需持久)。
- 选中连接即开始监控 → 会**触发 `ensure_session`**(可能为只想浏览的连接建立 SSH 会话)。符合「监控侧栏跟随选中主机」的预期;如需「仅在打开终端/文件时才连」可后续加开关。
