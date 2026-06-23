/// 命令块数据模型与纯展示辅助(无 Flutter 依赖,便于单测)。
/// 见 docs/gui/11-命令块终端原型.md。
library;

/// 一个命令块的生命周期状态。
enum BlockStatus {
  /// 命令执行中(等待远端 / 本机返回)。
  running,

  /// 退出码 0(或未知退出码的 POSIX 降级)成功。
  success,

  /// 命令本身返回非零退出码。
  failure,

  /// 执行通道层面出错(连接失败 / 会话异常等)。
  error,
}

/// 一条命令 + 它的输出,构成「一块」。字段在执行过程中被回填,故为可变。
class CommandBlock {
  CommandBlock({
    required this.id,
    required this.command,
    required this.cwd,
    required this.startedAt,
  });

  /// 进程内自增唯一 id(用于列表 key)。
  final int id;

  /// 用户输入的原始命令。
  final String command;

  /// 命令运行时所在的工作目录(在块头提示符里展示)。
  final String cwd;

  /// 块创建时刻(用于耗时兜底)。
  final DateTime startedAt;

  /// 合并后的输出(stdout+stderr)。
  String output = '';

  /// 退出码:`null` = 运行中 / 未知(非 POSIX 远端)。
  int? exitCode;

  /// 是否解析到哨兵(退出码 / cwd 是否可信)。
  bool hadMarker = false;

  /// 当前状态。
  BlockStatus status = BlockStatus.running;

  /// 执行耗时(完成后填)。
  Duration? duration;

  /// 是否折叠输出体。
  bool collapsed = false;

  bool get isRunning => status == BlockStatus.running;

  bool get hasOutput => output.trim().isNotEmpty;
}

/// 路径压缩(纯展示):超过 [maxLen] 时保留尾部若干层级、前缀 `…/`。
/// 不含 `/` 的占位串(如「本机」「~」)原样返回。
String compactPath(String path, {int maxLen = 36}) {
  if (path.length <= maxLen) return path;
  final parts = path.split('/');
  final tail = <String>[];
  var len = 1; // 预留前缀 '…'
  for (var i = parts.length - 1; i >= 0; i--) {
    final seg = parts[i];
    if (seg.isEmpty) continue;
    final add = seg.length + 1; // '/seg'
    if (len + add > maxLen && tail.isNotEmpty) break;
    tail.insert(0, seg);
    len += add;
  }
  return '…/${tail.join('/')}';
}

/// 常用命令兜底建议(历史不足时补充)。尾随空格表示通常还要跟参数。
const kCommonCommands = <String>[
  'ls -la', 'cd ', 'cd ..', 'pwd', 'cat ', 'less ', 'tail -f ', 'head -n ',
  'grep -rn ', 'find . -name ', 'ps aux', 'top', 'htop', 'df -h', 'du -sh *',
  'free -h', 'uname -a', 'whoami', 'echo ', 'export ', 'env',
  'mkdir -p ', 'rm -rf ', 'cp -r ', 'mv ', 'touch ', 'chmod +x ', 'chown ',
  'ln -s ', 'curl -sSL ', 'wget ', 'tar -xzf ', 'unzip ', 'kill -9 ',
  'git status', 'git log --oneline', 'git pull', 'git push', 'git diff',
  'docker ps', 'docker logs ', 'systemctl status ', 'journalctl -u ',
];

/// 按前缀给出命令建议:最近历史优先(去重、最近在前),再用常用命令兜底。
///
/// 排除与当前输入完全相同者,只取前缀匹配,最多 [limit] 条。纯函数,便于单测。
List<String> suggestCommands(
  List<String> history,
  String prefix, {
  List<String> common = kCommonCommands,
  int limit = 6,
}) {
  final p = prefix.trimLeft();
  if (p.isEmpty) return const [];
  final seen = <String>{};
  final out = <String>[];
  void consider(String candidate) {
    if (out.length >= limit) return;
    if (candidate == p || !candidate.startsWith(p)) return;
    if (seen.add(candidate)) out.add(candidate);
  }

  for (var i = history.length - 1; i >= 0; i--) {
    consider(history[i]);
  }
  for (final c in common) {
    consider(c);
  }
  return out;
}

/// 人类可读的耗时:<1s 用毫秒,<1min 用秒,否则 `m s`。
String formatBlockDuration(Duration d) {
  final ms = d.inMilliseconds;
  if (ms < 1000) return '${ms}ms';
  final s = ms / 1000.0;
  if (s < 60) return '${s.toStringAsFixed(s < 10 ? 1 : 0)}s';
  final m = d.inMinutes;
  final rem = d.inSeconds % 60;
  return '${m}m${rem}s';
}
