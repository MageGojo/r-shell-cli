import 'package:flutter_test/flutter_test.dart';
import 'package:r_shell_desktop/features/blockterm/block_models.dart';

void main() {
  group('compactPath', () {
    test('短路径原样返回', () {
      expect(compactPath('/etc'), '/etc');
      expect(compactPath('~'), '~');
      expect(compactPath('本机'), '本机');
    });

    test('长路径保留尾部层级并加 … 前缀', () {
      const long = '/Users/shcodegojo/Project/r-shell-cli/desktop/lib/features';
      final out = compactPath(long, maxLen: 24);
      expect(out.startsWith('…/'), isTrue);
      expect(out.length, lessThanOrEqualTo(26));
      // 末层级保留。
      expect(out.endsWith('features'), isTrue);
    });
  });

  group('formatBlockDuration', () {
    test('毫秒 / 秒 / 分秒', () {
      expect(formatBlockDuration(const Duration(milliseconds: 250)), '250ms');
      expect(formatBlockDuration(const Duration(milliseconds: 1500)), '1.5s');
      expect(formatBlockDuration(const Duration(seconds: 42)), '42s');
      expect(formatBlockDuration(const Duration(seconds: 95)), '1m35s');
    });
  });

  group('CommandBlock', () {
    test('默认运行中、无输出', () {
      final b = CommandBlock(
        id: 1,
        command: 'ls',
        cwd: '/tmp',
        startedAt: DateTime.now(),
      );
      expect(b.isRunning, isTrue);
      expect(b.hasOutput, isFalse);
      b.output = 'a.txt\n';
      expect(b.hasOutput, isTrue);
    });
  });

  group('suggestCommands', () {
    test('空前缀不给建议', () {
      expect(suggestCommands(const ['ls -la'], ''), isEmpty);
      expect(suggestCommands(const ['ls -la'], '   '), isEmpty);
    });

    test('历史优先、最近在前、前缀匹配', () {
      final out = suggestCommands(
        const ['git status', 'git pull', 'git status'],
        'git',
        common: const [],
      );
      // 最近的 'git status' 在前;去重后只一条 'git status',再 'git pull'。
      expect(out, ['git status', 'git pull']);
    });

    test('排除与输入完全相同者', () {
      final out = suggestCommands(
        const ['ls -la'],
        'ls -la',
        common: const [],
      );
      expect(out, isEmpty);
    });

    test('历史不足用常用命令兜底,且历史在常用之前', () {
      final out = suggestCommands(
        const ['docker compose up'],
        'docker',
        common: const ['docker ps', 'docker logs '],
      );
      expect(out.first, 'docker compose up');
      expect(out.contains('docker ps'), isTrue);
      expect(out.contains('docker logs '), isTrue);
    });

    test('限制条数', () {
      final history = List.generate(20, (i) => 'cmd$i');
      final out = suggestCommands(history, 'cmd', common: const [], limit: 3);
      expect(out.length, 3);
    });

    test('历史与常用去重', () {
      final out = suggestCommands(
        const ['git status'],
        'git s',
        common: const ['git status'],
      );
      expect(out, ['git status']);
    });
  });
}
