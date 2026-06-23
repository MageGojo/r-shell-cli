import '../../src/rust/api/monitor.dart' as rust;

/// Repository 层:封装 frb 的监控流调用,监控 UI / 控制器不直接接触 frb。
class StatsRepository {
  const StatsRepository();

  /// 订阅某连接的实时系统指标流。`intervalMs` 为相邻两次采样间隔(下限由 Rust 兜底)。
  ///
  /// 每帧 [rust.StatsFrame] 的 `sample` / `error` 互斥:采集成功带 `sample`,失败带
  /// `error`(连接 / 命令失败)。错误走数据帧(Rust 内部自重试),流只在取消时结束。
  Stream<rust.StatsFrame> stream(String connectionId, {int intervalMs = 2000}) =>
      rust.statsStream(connectionId: connectionId, intervalMs: intervalMs);

  /// 订阅**本机**(运行 GUI 的这台电脑)的实时系统指标流（基于 sysinfo，无需连接）。
  Stream<rust.StatsFrame> localStream({int intervalMs = 1000}) =>
      rust.localStatsStream(intervalMs: intervalMs);
}
