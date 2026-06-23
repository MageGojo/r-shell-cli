import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../src/rust/api/connections.dart';
import '../../src/rust/api/monitor.dart' as rust;
import 'stats_repository.dart';

/// 监控侧栏的生命周期状态。
enum MonitorStatus { idle, connecting, live, error }

/// 跟随「当前选中连接」订阅实时系统指标:持有最新一帧 + 各指标的环形历史(供走势图),
/// 切换连接重订阅,出错自动重试。
///
/// 与 `BlockTerminalController` / `FilesController` 一样由 `AppScaffold` 持有,切换视图
/// 不丢状态。复用共享 SSH 会话(Rust 侧 `ensure_session`),**不主动关闭会话**。
class MonitorController extends ChangeNotifier {
  final StatsRepository repo;

  /// 采样间隔(毫秒)。默认 1s 实时刷新(Rust 后端足以支撑);Rust 侧会在「采集耗时」
  /// 内尽量贴近该周期(命令更慢的主机如 Windows CIM 则按实际耗时节流)。
  /// 经设置页 [setIntervalMs] 在运行时调整(直接改值请走该方法,以便重建流)。
  int intervalMs;

  /// 每条指标保留的历史点数(1s × 90 ≈ 最近 1.5 分钟)。
  final int historyLength;

  /// 出错后自动重试的等待时长。
  final Duration retryDelay;

  MonitorController({
    this.repo = const StatsRepository(),
    this.intervalMs = 1000,
    this.historyLength = 90,
    this.retryDelay = const Duration(seconds: 3),
  });

  ConnectionDto? _connection;
  ConnectionDto? get connection => _connection;

  /// 是否处于「本机监控」模式（数据来自本地 sysinfo，而非某个连接）。
  bool _isLocal = false;
  bool get isLocal => _isLocal;

  rust.SystemStatsDto? _latest;
  rust.SystemStatsDto? get latest => _latest;

  MonitorStatus _status = MonitorStatus.idle;
  MonitorStatus get status => _status;

  String? _error;
  String? get error => _error;

  final List<double> cpuHistory = [];
  final List<double> memHistory = [];
  final List<double> netRxHistory = [];
  final List<double> netTxHistory = [];

  StreamSubscription<rust.StatsFrame>? _sub;
  Timer? _retry;
  bool _disposed = false;

  /// 当前订阅的「代次」：每次切换目标 / 停止都自增，订阅回调据此忽略过期帧
  /// （连接与本机统一一套，不再靠 connectionId 比对）。
  int _gen = 0;

  /// 当前目标的流工厂（用于重试时重新建流）；null 表示空闲。
  Stream<rust.StatsFrame> Function()? _streamFactory;

  /// 绑定(或切换)当前连接。同一连接重复调用不重订阅。
  void setConnection(ConnectionDto? c) {
    if (!_isLocal && c?.id == _connection?.id) return;
    _isLocal = false;
    _connection = c;
    _restart(
      c == null ? null : () => repo.stream(c.id, intervalMs: intervalMs),
    );
  }

  /// 切到「本机监控」（数据来自本地 sysinfo）。重复调用不重订阅。
  void setLocal() {
    if (_isLocal) return;
    _isLocal = true;
    _connection = null;
    _restart(() => repo.localStream(intervalMs: intervalMs));
  }

  /// 设置采样间隔(设置页驱动)。若有活动目标,用新间隔重建当前流。
  void setIntervalMs(int ms) {
    if (ms <= 0 || ms == intervalMs) return;
    intervalMs = ms;
    if (_streamFactory == null) return;
    if (_isLocal) {
      _restart(() => repo.localStream(intervalMs: intervalMs));
    } else {
      final c = _connection;
      if (c != null) _restart(() => repo.stream(c.id, intervalMs: intervalMs));
    }
  }

  /// 手动立即重试(出错卡片上的「重试」按钮)。
  void retryNow() {
    if (_streamFactory == null) return;
    _stop();
    _subscribe();
  }

  void _restart(Stream<rust.StatsFrame> Function()? factory) {
    _streamFactory = factory;
    _stop();
    _resetData();
    if (factory == null) {
      _setStatus(MonitorStatus.idle);
      return;
    }
    _subscribe();
  }

  void _subscribe() {
    final factory = _streamFactory;
    if (factory == null) return;
    _gen++;
    final gen = _gen;
    _error = null;
    _setStatus(MonitorStatus.connecting);
    _sub = factory().listen(
      (frame) => _onFrame(gen, frame),
      // 正常情况下采集失败走数据帧(frame.error),Rust 内部自重试;onError/onDone 仅在
      // 流本身异常 / 结束(如 frb 内部错误)时触发,作为兜底重订阅。
      onError: (Object e) => _onStreamGone(gen, '$e'),
      onDone: () => _onStreamGone(gen, '监控流已结束'),
      cancelOnError: true,
    );
  }

  void _onFrame(int gen, rust.StatsFrame frame) {
    if (_disposed || gen != _gen) return;
    final error = frame.error;
    if (error != null) {
      _applyError(error);
      return;
    }
    final sample = frame.sample;
    if (sample != null) _applySample(sample);
  }

  void _applySample(rust.SystemStatsDto s) {
    _latest = s;
    _error = null;
    _push(cpuHistory, s.cpuPercent);
    _push(memHistory, s.memPercent);
    _push(netRxHistory, s.netRxPerSec);
    _push(netTxHistory, s.netTxPerSec);
    _setStatus(MonitorStatus.live);
  }

  /// 采集失败:标错并保留最近一帧数据(图表停在最后已知值),等 Rust 自重试恢复。
  void _applyError(String message) {
    _error = message;
    _setStatus(MonitorStatus.error);
  }

  void _push(List<double> buf, double value) {
    buf.add(value);
    if (buf.length > historyLength) buf.removeAt(0);
  }

  /// 流本身断了(非业务失败):取消并定时重订阅。
  void _onStreamGone(int gen, String message) {
    if (_disposed || gen != _gen) return;
    _error = message;
    _setStatus(MonitorStatus.error);
    _scheduleRetry(gen);
  }

  void _scheduleRetry(int gen) {
    _retry?.cancel();
    _retry = Timer(retryDelay, () {
      if (_disposed || gen != _gen) return;
      _sub?.cancel();
      _sub = null;
      _subscribe();
    });
  }

  void _resetData() {
    _latest = null;
    _error = null;
    cpuHistory.clear();
    memHistory.clear();
    netRxHistory.clear();
    netTxHistory.clear();
  }

  void _stop() {
    _retry?.cancel();
    _retry = null;
    _sub?.cancel();
    _sub = null;
  }

  void _setStatus(MonitorStatus status) {
    _status = status;
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _stop();
    super.dispose();
  }
}
