import 'package:flutter/foundation.dart';

import '../../src/rust/api/connections.dart';
import 'block_repository.dart';
import 'block_session.dart';

/// 命令块终端的**多标签管理器**:持有若干 [BlockSession](每个固定绑定一个执行目标
/// = 本机或某连接)+ 当前活动标签下标。
///
/// 「在哪个标签执行哪台服务器的命令」由标签本身表达:双击侧栏连接 → 开/聚焦该连接的
/// 标签;`+` → 开一个本机标签。面板不再有「临时切目标」的下拉选择器。
///
/// 与 `FilesController` / `MonitorController` 一样由 `AppScaffold` 持有,切换视图不丢
/// 状态(后台标签会话继续存在)。
class BlockTerminalController extends ChangeNotifier {
  BlockTerminalController([this._repo = const BlockRepository()]) {
    // 默认开一个「本机」标签,进终端板块即可直接敲命令。
    _sessions.add(BlockSession.local(_repo));
  }

  final BlockRepository _repo;

  final List<BlockSession> _sessions = [];
  List<BlockSession> get sessions => List.unmodifiable(_sessions);

  int _activeIndex = 0;
  int get activeIndex => _activeIndex;

  bool get isEmpty => _sessions.isEmpty;

  /// 当前活动标签会话;无标签时为 null。
  BlockSession? get active =>
      _sessions.isEmpty ? null : _sessions[_activeIndex.clamp(0, _sessions.length - 1)];

  /// 开/聚焦某连接的标签(按连接 id 去重,已存在则切过去)。
  void openFor(ConnectionDto c) {
    final i = _sessions.indexWhere((s) => s.connection?.id == c.id);
    if (i >= 0) {
      _activeIndex = i;
    } else {
      _sessions.add(BlockSession.remote(c, _repo));
      _activeIndex = _sessions.length - 1;
    }
    notifyListeners();
  }

  /// 新开一个本机标签(始终新建,允许多个本机标签并存)。
  void openLocal() {
    _sessions.add(BlockSession.local(_repo));
    _activeIndex = _sessions.length - 1;
    notifyListeners();
  }

  void setActive(int index) {
    if (index < 0 || index >= _sessions.length || index == _activeIndex) return;
    _activeIndex = index;
    notifyListeners();
  }

  /// 关闭某标签(连带回收其会话)。关到空时显示空状态。
  void closeAt(int index) {
    if (index < 0 || index >= _sessions.length) return;
    _sessions.removeAt(index).dispose();
    if (_activeIndex >= _sessions.length) {
      _activeIndex = _sessions.isEmpty ? 0 : _sessions.length - 1;
    }
    notifyListeners();
  }

  @override
  void dispose() {
    for (final s in _sessions) {
      s.dispose();
    }
    super.dispose();
  }
}
