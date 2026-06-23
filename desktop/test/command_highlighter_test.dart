// shell 命令分词器的纯逻辑单测(不依赖 Flutter 绑定)。
import 'package:flutter_test/flutter_test.dart';
import 'package:r_shell_desktop/features/blockterm/command_highlighter.dart';

List<CommandTokenType> _types(String input) =>
    tokenizeCommand(input).map((t) => t.type).toList();

String _rebuild(String input) => tokenizeCommand(input).map((t) => t.text).join();

void main() {
  group('tokenizeCommand', () {
    test('保持无损:拼回原文', () {
      const samples = [
        'ls -la /etc',
        'sudo apt install nginx',
        'cat a.txt | grep "foo bar" > out.log # 注释',
        'echo \$HOME && cd ~/work',
      ];
      for (final s in samples) {
        expect(_rebuild(s), s, reason: s);
      }
    });

    test('首词为命令,选项与路径分类正确', () {
      final tokens = tokenizeCommand('ls -la /etc');
      expect(tokens[0], const CommandToken('ls', CommandTokenType.command));
      expect(_types('ls -la /etc'), [
        CommandTokenType.command,
        CommandTokenType.whitespace,
        CommandTokenType.option,
        CommandTokenType.whitespace,
        CommandTokenType.path,
      ]);
    });

    test('sudo / env 之后的词仍按命令高亮', () {
      final tokens = tokenizeCommand('sudo apt install nginx')
          .where((t) => t.type != CommandTokenType.whitespace)
          .toList();
      expect(tokens[0].type, CommandTokenType.command); // sudo
      expect(tokens[1].type, CommandTokenType.command); // apt
      expect(tokens[2].type, CommandTokenType.argument); // install
      expect(tokens[3].type, CommandTokenType.argument); // nginx
    });

    test('管道 / && 之后重置为命令位', () {
      final tokens = tokenizeCommand('cat a | grep b')
          .where((t) => t.type != CommandTokenType.whitespace)
          .toList();
      expect(tokens[0].type, CommandTokenType.command); // cat
      expect(tokens[1].type, CommandTokenType.argument); // a
      expect(tokens[2].type, CommandTokenType.operator); // |
      expect(tokens[3].type, CommandTokenType.command); // grep
      expect(tokens[4].type, CommandTokenType.argument); // b
    });

    test('字符串 / 变量 / 注释', () {
      final spaced = tokenizeCommand('echo "hi there" \$USER # note')
          .where((t) => t.type != CommandTokenType.whitespace)
          .toList();
      expect(spaced[0].type, CommandTokenType.command);
      expect(spaced[1], const CommandToken('"hi there"', CommandTokenType.string));
      expect(spaced[2], const CommandToken('\$USER', CommandTokenType.variable));
      expect(spaced[3], const CommandToken('# note', CommandTokenType.comment));
    });

    test('花括号变量形式', () {
      final tokens = tokenizeCommand('echo \${HOME}/bin');
      final variable =
          tokens.firstWhere((t) => t.type == CommandTokenType.variable);
      expect(variable.text, '\${HOME}');
    });

    test('空串返回空列表', () {
      expect(tokenizeCommand(''), isEmpty);
    });
  });
}
