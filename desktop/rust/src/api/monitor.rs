//! GUI 监控仪表盘 API —— 复用 r-shell-core 的系统指标采集能力。
//!
//! [`stats_stream`] 在一个已保存连接上**周期性**快照远端资源(`/proc` + `df`,
//! 一次 SSH 往返),用 [`SystemStats::from_samples`] 把两次采样差分出 CPU% 与网络
//! 速率,再把 [`StatsFrame`] 经 [`StreamSink`] 单向推给 Dart(与终端 PTY / SFTP
//! 进度同范式)。调用前 [`ensure_session`](super::ensure_session) 复用终端 / 文件
//! 管理器共享的活跃 SSH 会话,**绝不另开连接**。
//!
//! **错误也走流**:本函数**永不返回 `Err`**(返回 `()`)。frb 对 `async fn(sink) ->
//! Result<(), E>` 会把函数的 `Err` 放进一个 *unawaited* 的 Future,Dart 端 `onError`
//! 收不到、最终变成 "Unhandled Exception"。所以这里把采集失败封装成带 `error` 的
//! [`StatsFrame`] 推进 sink(Dart `onData` 可处理),并**在 Rust 内部自重试**
//! (主机不可达时每 `interval` 重试一次),流只在 Dart 取消订阅时结束。
//! (用 `Option` 字段的普通 struct 而非数据枚举,避免引入 `freezed` 工具链。)
//!
//! 数值字段用 `f64` / `i64` 干净映射 Dart `double` / `int`;**展示格式化交给 Dart 侧**。
//! 凭据只在 Rust 侧用,绝不回传。

use std::time::{Duration, Instant};

use r_shell_core::monitor::{DiskUsage, StatsSnapshot, SystemStats};

use crate::frb_generated::StreamSink;

/// 单个磁盘 / 卷的用量(KB 为千字节,`percent` 为 0.0..=100.0)。
/// `mount` 是人类可读标签:Unix 挂载点(`/`、`/data`)或 Windows 盘符(`C:`)。
pub struct DiskUsageDto {
    pub mount: String,
    pub used_kb: i64,
    pub total_kb: i64,
    pub percent: f64,
}

impl From<DiskUsage> for DiskUsageDto {
    fn from(d: DiskUsage) -> Self {
        let percent = d.percent() as f64;
        Self {
            mount: d.mount,
            used_kb: d.used_kb as i64,
            total_kb: d.total_kb as i64,
            percent,
        }
    }
}

/// 一帧派生后的系统指标(百分比 0.0..=100.0,KB 为千字节,速率为字节/秒)。
/// 与 core [`SystemStats`] 一一映射,仅做 frb 友好的类型收敛。
pub struct SystemStatsDto {
    pub cpu_percent: f64,
    pub mem_percent: f64,
    pub mem_used_kb: i64,
    pub mem_total_kb: i64,
    pub swap_percent: f64,
    pub swap_used_kb: i64,
    pub swap_total_kb: i64,
    /// 主盘(最大的真实文件系统):百分比 / 已用 / 总量。
    pub disk_percent: f64,
    pub disk_used_kb: i64,
    pub disk_total_kb: i64,
    /// 全部真实磁盘 / 卷(按容量从大到小),用于多盘视图。
    pub disks: Vec<DiskUsageDto>,
    /// 字节/秒。
    pub net_rx_per_sec: f64,
    pub net_tx_per_sec: f64,
    pub load1: f64,
    pub uptime_secs: f64,
    pub cpu_cores: u32,
    pub os: String,
}

impl From<SystemStats> for SystemStatsDto {
    fn from(s: SystemStats) -> Self {
        Self {
            cpu_percent: s.cpu_percent as f64,
            mem_percent: s.mem_percent as f64,
            mem_used_kb: s.mem_used_kb as i64,
            mem_total_kb: s.mem_total_kb as i64,
            swap_percent: s.swap_percent as f64,
            swap_used_kb: s.swap_used_kb as i64,
            swap_total_kb: s.swap_total_kb as i64,
            disk_percent: s.disk_percent as f64,
            disk_used_kb: s.disk_used_kb as i64,
            disk_total_kb: s.disk_total_kb as i64,
            disks: s.disks.into_iter().map(DiskUsageDto::from).collect(),
            net_rx_per_sec: s.net_rx_per_sec,
            net_tx_per_sec: s.net_tx_per_sec,
            load1: s.load1 as f64,
            uptime_secs: s.uptime_secs,
            cpu_cores: s.cpu_cores,
            os: s.os,
        }
    }
}

/// 监控流的一帧:`sample` 与 `error` 互斥——成功时 `sample=Some`、`error=None`;
/// 失败时 `sample=None`、`error=Some(原因)`。错误走数据帧而非 frb 的 `Err`(见模块
/// 文档),Dart 端在 `onData` 内按字段分支处理。
pub struct StatsFrame {
    pub sample: Option<SystemStatsDto>,
    pub error: Option<String>,
}

impl StatsFrame {
    fn sample(dto: SystemStatsDto) -> Self {
        Self { sample: Some(dto), error: None }
    }
    fn error(message: String) -> Self {
        Self { sample: None, error: Some(message) }
    }
}

/// 周期性推送某连接的实时系统指标(Dart 得到 `Stream<StatsFrame>`)。
///
/// - `interval_ms`:相邻两次采样的间隔(下限 250ms 防过载,远端命令本身也有耗时)。
/// - 首个成功帧的 CPU%/网速为 0(需两帧差分),内存 / 磁盘 / 负载 / uptime / OS 立即可用。
/// - 采集失败 → 推带 `error` 的帧并在 `interval` 后自重试(主机恢复即自动回到 `sample`)。
/// - Dart 取消订阅 → `sink.add` 失败 → 循环退出。**永不返回 `Err`**(避免 frb unawaited 未捕获)。
/// - 复用共享会话,**不主动关闭**(终端 / SFTP 可能在用;由终端「最后标签关闭」统一回收)。
pub async fn stats_stream(connection_id: String, interval_ms: u32, sink: StreamSink<StatsFrame>) {
    let interval = Duration::from_millis(interval_ms.max(250) as u64);
    let mgr = super::manager();

    let mut previous: Option<StatsSnapshot> = None;
    let mut last_at = Instant::now();

    loop {
        let tick = Instant::now();
        let frame = match collect(mgr, &connection_id, previous.as_ref(), last_at).await {
            Ok((dto, snapshot, now)) => {
                previous = Some(snapshot);
                last_at = now;
                StatsFrame::sample(dto)
            }
            Err(e) => {
                previous = None; // 重连后 CPU%/网速从头差分,避免跨断点的脏速率
                StatsFrame::error(e)
            }
        };

        if sink.add(frame).is_err() {
            break; // Dart 端已取消订阅
        }

        // 贴近设定周期:扣掉本次采集耗时再睡;采集比间隔还慢(如 Windows CIM)则不睡,
        // 让刷新尽量跟上「每秒」而不堆积。
        let remaining = interval.saturating_sub(tick.elapsed());
        if !remaining.is_zero() {
            tokio::time::sleep(remaining).await;
        }
    }
}

/// 周期性推送**本机**(运行 GUI 的这台电脑)的实时系统指标(Dart 得到 `Stream<StatsFrame>`)。
///
/// 与 [`stats_stream`] 同范式,但数据来自本机 `sysinfo`(见 [`r_shell_core::local`]),
/// **无需任何连接 / 会话**:本机采集几乎不会失败,故恒推 `sample` 帧。`LocalCollector`
/// 跨循环复用(CPU% 依赖两次刷新间的增量),首帧 CPU%/网速为 0(需两帧差分)。
pub async fn local_stats_stream(interval_ms: u32, sink: StreamSink<StatsFrame>) {
    let interval = Duration::from_millis(interval_ms.max(250) as u64);
    let mut collector = r_shell_core::local::LocalCollector::new();
    let mut previous: Option<StatsSnapshot> = None;
    let mut last_at = Instant::now();

    loop {
        let tick = Instant::now();
        let snapshot = collector.snapshot();
        let now = Instant::now();
        let elapsed = now.duration_since(last_at).as_secs_f64();
        let stats = SystemStats::from_samples(previous.as_ref(), &snapshot, elapsed);
        previous = Some(snapshot);
        last_at = now;

        if sink.add(StatsFrame::sample(SystemStatsDto::from(stats))).is_err() {
            break; // Dart 端已取消订阅
        }

        let remaining = interval.saturating_sub(tick.elapsed());
        if !remaining.is_zero() {
            tokio::time::sleep(remaining).await;
        }
    }
}

/// 采一帧:确保会话 → 抓快照 → 解析 → 与上帧差分。返回(指标, 本次快照, 采样时刻)。
async fn collect(
    mgr: &'static r_shell_core::native_backend::NativeConnectionManager,
    connection_id: &str,
    previous: Option<&StatsSnapshot>,
    last_at: Instant,
) -> Result<(SystemStatsDto, StatsSnapshot, Instant), String> {
    super::ensure_session(connection_id).await?;
    let snapshot = mgr
        .fetch_system_snapshot(connection_id)
        .await
        .map_err(|e| e.to_string())?;
    let now = Instant::now();
    let elapsed = now.duration_since(last_at).as_secs_f64();
    let stats = SystemStats::from_samples(previous, &snapshot, elapsed);
    Ok((SystemStatsDto::from(stats), snapshot, now))
}
