import 'package:flutter/foundation.dart';

import '../../src/rust/api/connections.dart';
import 'block_models.dart';
import 'block_repository.dart';

/// 一个命令块标签页的会话状态:**固定绑定一个执行目标**(本机或某连接),
/// 持有该目标的 cwd、命令块列表、命令历史。
///
/// 目标在构造时确定、之后不可变——「哪个标签执行哪台服务器」由标签本身表达,
/// 面板不再提供「临时切目标」的下拉选择。每条命令各起一个 exec 通道,通道间不
/// 共享 shell 状态;cwd 由 bridge 的 POSIX 哨兵在每次执行后回报,带进下一块
/// (于是 `cd` 跨块持久)。
class BlockSession extends ChangeNotifier {
  BlockSession.local([BlockRepository repo = const BlockRepository()])
      : _repo = repo,
        connection = null;

  BlockSession.remote(this.connection,
      [BlockRepository repo = const BlockRepository()])
      : _repo = repo;

  final BlockRepository _repo;

  /// 绑定的目标连接;`null` 表示本机。构造后不可变。
  final ConnectionDto? connection;
  bool get isLocal => connection == null;

  /// 标签标题(本机 / 连接名)。
  String get title => connection?.name ?? '本机';

  /// 当前工作目录(空串 = 尚未执行过,首条命令后由远端 / 本机回填)。
  String _cwd = '';
  String get cwd => _cwd;

  final List<CommandBlock> _blocks = [];
  List<CommandBlock> get blocks => List.unmodifiable(_blocks);

  bool _busy = false;
  bool get busy => _busy;

  int _seq = 0;

  // 命令历史(跨块);_historyCursor == length 表示停在「最新空输入」。
  final List<String> _history = [];
  int _historyCursor = 0;

  /// 执行一条命令:立刻追加「运行中」块,await 结果后回填输出 / 退出码 / cwd / 耗时。
  Future<void> run(String raw) async {
    final command = raw.trim();
    if (command.isEmpty || _busy) return;

    if (_history.isEmpty || _history.last != command) {
      _history.add(command);
    }
    _historyCursor = _history.length;

    final block = CommandBlock(
      id: ++_seq,
      command: command,
      cwd: _cwd.isEmpty ? (isLocal ? '本机' : '~') : _cwd,
      startedAt: DateTime.now(),
    );
    _blocks.add(block);
    _busy = true;
    notifyListeners();

    final sw = Stopwatch()..start();
    try {
      final res = isLocal
          ? await _repo.runLocal(command, _cwd)
          : await _repo.runRemote(connection!.id, command, _cwd);
      block.output = res.body;
      block.hadMarker = res.hadMarker;
      block.exitCode = res.hadMarker ? res.exitCode : null;
      // 未命中哨兵(非 POSIX 远端)退出码未知,不当失败处理。
      block.status = !res.hadMarker
          ? BlockStatus.success
          : (res.exitCode == 0 ? BlockStatus.success : BlockStatus.failure);
      if (res.cwd.trim().isNotEmpty) _cwd = res.cwd;
    } catch (e) {
      block.output = '$e';
      block.status = BlockStatus.error;
    } finally {
      sw.stop();
      block.duration = sw.elapsed;
      _busy = false;
      notifyListeners();
    }
  }

  /// 重跑某块的命令(追加为新块)。
  Future<void> rerun(CommandBlock block) => run(block.command);

  /// 按前缀给出命令建议(最近历史优先 + 常用命令兜底),供输入框补全用。
  List<String> suggest(String prefix) => suggestCommands(_history, prefix);

  void toggleCollapse(CommandBlock block) {
    block.collapsed = !block.collapsed;
    notifyListeners();
  }

  void clear() {
    if (_blocks.isEmpty) return;
    _blocks.clear();
    notifyListeners();
  }

  /// ↑ 历史(更旧)。返回要填入输入框的命令,无更多则 null。
  String? historyPrev() {
    if (_history.isEmpty) return null;
    if (_historyCursor > 0) _historyCursor--;
    return _history[_historyCursor];
  }

  /// ↓ 历史(更新)。越过最新返回空串(清空输入),无历史则 null。
  String? historyNext() {
    if (_history.isEmpty) return null;
    if (_historyCursor < _history.length - 1) {
      _historyCursor++;
      return _history[_historyCursor];
    }
    _historyCursor = _history.length;
    return '';
  }
}
