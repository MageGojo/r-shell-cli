import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../src/rust/api/mcp.dart' as rust;

/// MCP 面板状态控制器:读取 / 起停本机 MCP 服务,运行时按秒刷新运行时长。
///
/// 数据源是 bridge `api/mcp.rs`(同步读状态 + 异步起停)。工具目录一次性加载。
class McpController extends ChangeNotifier {
  McpController()
      : tools = rust.mcpTools(),
        _status = rust.mcpStatus() {
    _syncTicker();
  }

  /// MCP 暴露的工具目录(静态)。
  final List<rust.McpToolDto> tools;

  rust.McpStatusDto _status;
  bool _busy = false;
  String? _error;
  Timer? _ticker;

  rust.McpStatusDto get status => _status;
  bool get running => _status.running;
  bool get busy => _busy;
  String? get error => _error;
  String get endpoint => _status.endpoint;
  String get host => _status.host;
  int get port => _status.port;
  int get uptimeSecs => _status.uptimeSecs.toInt();

  /// 重新读取状态(同步),并按运行态启停秒刷新定时器。
  void refresh() {
    _status = rust.mcpStatus();
    _syncTicker();
    notifyListeners();
  }

  /// 运行时每秒刷新运行时长;停止后取消定时器。
  void _syncTicker() {
    if (_status.running) {
      _ticker ??= Timer.periodic(const Duration(seconds: 1), (_) {
        _status = rust.mcpStatus();
        if (!_status.running) {
          _ticker?.cancel();
          _ticker = null;
        }
        notifyListeners();
      });
    } else {
      _ticker?.cancel();
      _ticker = null;
    }
  }

  Future<void> toggle() => running ? stop() : start();

  Future<void> start() => _run(() => rust.mcpStart());

  Future<void> stop() => _run(() => rust.mcpStop());

  Future<void> restart() => _run(() async {
        await rust.mcpStop();
        await rust.mcpStart();
      });

  Future<void> _run(Future<void> Function() action) async {
    if (_busy) return;
    _busy = true;
    _error = null;
    notifyListeners();
    try {
      await action();
    } catch (e) {
      _error = '$e';
    }
    _busy = false;
    refresh();
  }

  @override
  void dispose() {
    _ticker?.cancel();
    super.dispose();
  }
}
