import 'package:flutter/widgets.dart';

import '../../shared/theme/app_colors.dart';
import '../../shared/theme/app_dimens.dart';

/// 一条 shell 命令行被分出的词法单元类型。
enum CommandTokenType {
  command, // 命令名(行首,或管道 / && / ; 之后的第一个词)
  argument, // 普通参数
  option, // 选项(-x / --long)
  string, // 引号字符串
  operator, // 管道 / 重定向 / 逻辑连接符
  variable, // $VAR / ${VAR} / $1 / $?
  comment, // # 注释到行尾
  path, // 含 / 或以 ~ 开头的路径样参数
  whitespace, // 空白(保留以便原样重建)
}

/// 词法单元(保留原文,便于无损重建整行)。
class CommandToken {
  final String text;
  final CommandTokenType type;
  const CommandToken(this.text, this.type);

  @override
  bool operator ==(Object other) =>
      other is CommandToken && other.text == text && other.type == type;

  @override
  int get hashCode => Object.hash(text, type);

  @override
  String toString() => 'CommandToken(${type.name}, "$text")';
}

const _tab = 0x09;
const _space = 0x20;
const _hash = 0x23; // #
const _dollar = 0x24; // $
const _squote = 0x27; // '
const _dquote = 0x22; // "
const _backslash = 0x5c; // \
const _lbrace = 0x7b; // {
const _rbrace = 0x7d; // }
const _tilde = 0x7e; // ~

bool _isSpace(int c) => c == _space || c == _tab;

/// 管道 / 重定向 / 逻辑连接 / 分隔符:| & ; < >
bool _isOperatorChar(int c) =>
    c == 0x7c || c == 0x26 || c == 0x3b || c == 0x3c || c == 0x3e;

bool _isWordChar(int c) =>
    (c >= 0x30 && c <= 0x39) || // 0-9
    (c >= 0x41 && c <= 0x5a) || // A-Z
    (c >= 0x61 && c <= 0x7a) || // a-z
    c == 0x5f; // _

/// $? $$ $! $# $* $@ 这类单字符特殊变量。
bool _isVarSpecial(int c) =>
    c == 0x3f || c == _dollar || c == 0x21 || c == _hash || c == 0x2a || c == 0x40;

/// 命令位之后仍把下一个词当命令的「前缀词」(sudo apt … 让 apt 也高亮)。
const _commandPrefixes = <String>{
  'sudo', 'doas', 'env', 'time', 'nohup', 'exec', 'command', 'nice', 'xargs',
  'watch', 'then', 'do', 'else', 'su',
};

/// 把一条命令行切分为词法单元序列(纯函数,无 Flutter 依赖,便于单测)。
List<CommandToken> tokenizeCommand(String input) {
  final tokens = <CommandToken>[];
  final n = input.length;
  var i = 0;
  var atCommandPos = true;

  while (i < n) {
    final c = input.codeUnitAt(i);

    if (_isSpace(c)) {
      var j = i + 1;
      while (j < n && _isSpace(input.codeUnitAt(j))) {
        j++;
      }
      tokens.add(CommandToken(input.substring(i, j), CommandTokenType.whitespace));
      i = j;
      continue;
    }

    // # 注释:吃到行尾。
    if (c == _hash) {
      tokens.add(CommandToken(input.substring(i), CommandTokenType.comment));
      break;
    }

    if (_isOperatorChar(c)) {
      var j = i + 1;
      while (j < n && _isOperatorChar(input.codeUnitAt(j))) {
        j++;
      }
      tokens.add(CommandToken(input.substring(i, j), CommandTokenType.operator));
      i = j;
      atCommandPos = true; // 管道 / 逻辑符之后又是一条命令的开头
      continue;
    }

    // 单引号字符串(内部不转义)。
    if (c == _squote) {
      var j = i + 1;
      while (j < n && input.codeUnitAt(j) != _squote) {
        j++;
      }
      if (j < n) j++; // 含右引号
      tokens.add(CommandToken(input.substring(i, j), CommandTokenType.string));
      i = j;
      atCommandPos = false;
      continue;
    }

    // 双引号字符串(允许 \" 转义)。
    if (c == _dquote) {
      var j = i + 1;
      while (j < n) {
        final d = input.codeUnitAt(j);
        if (d == _backslash && j + 1 < n) {
          j += 2;
          continue;
        }
        if (d == _dquote) break;
        j++;
      }
      if (j < n) j++; // 含右引号
      tokens.add(CommandToken(input.substring(i, j), CommandTokenType.string));
      i = j;
      atCommandPos = false;
      continue;
    }

    // 变量:$VAR / ${...} / $1 / $? …
    if (c == _dollar) {
      var j = i + 1;
      if (j < n && input.codeUnitAt(j) == _lbrace) {
        while (j < n && input.codeUnitAt(j) != _rbrace) {
          j++;
        }
        if (j < n) j++; // 含 }
      } else if (j < n && _isVarSpecial(input.codeUnitAt(j))) {
        j++;
      } else {
        while (j < n && _isWordChar(input.codeUnitAt(j))) {
          j++;
        }
      }
      tokens.add(CommandToken(input.substring(i, j), CommandTokenType.variable));
      i = j;
      atCommandPos = false;
      continue;
    }

    // 普通词:吃到空白 / 操作符 / 引号 / $ / # 为止。
    var j = i;
    while (j < n) {
      final d = input.codeUnitAt(j);
      if (_isSpace(d) ||
          _isOperatorChar(d) ||
          d == _squote ||
          d == _dquote ||
          d == _dollar ||
          d == _hash) {
        break;
      }
      j++;
    }
    final word = input.substring(i, j);
    final type = _classifyWord(word, atCommandPos);
    tokens.add(CommandToken(word, type));
    atCommandPos =
        type == CommandTokenType.command && _commandPrefixes.contains(word);
    i = j;
  }

  return tokens;
}

CommandTokenType _classifyWord(String word, bool atCommandPos) {
  if (atCommandPos) return CommandTokenType.command;
  if (word.startsWith('-')) return CommandTokenType.option;
  if (word.contains('/') || word.startsWith('~')) return CommandTokenType.path;
  // 单独的 ~
  if (word.codeUnitAt(0) == _tilde) return CommandTokenType.path;
  return CommandTokenType.argument;
}

TextStyle _styleFor(CommandTokenType type, Color base) {
  switch (type) {
    case CommandTokenType.command:
      return const TextStyle(
        color: AppColors.accent,
        fontWeight: FontWeight.w600,
      );
    case CommandTokenType.option:
      return const TextStyle(color: AppColors.warning);
    case CommandTokenType.string:
      return const TextStyle(color: AppColors.online);
    case CommandTokenType.variable:
      return const TextStyle(color: AppColors.accentTeal);
    case CommandTokenType.operator:
      return const TextStyle(
        color: AppColors.accentIndigo,
        fontWeight: FontWeight.w600,
      );
    case CommandTokenType.comment:
      return const TextStyle(
        color: AppColors.textMuted,
        fontStyle: FontStyle.italic,
      );
    case CommandTokenType.path:
      return const TextStyle(color: AppColors.textSecondary);
    case CommandTokenType.argument:
    case CommandTokenType.whitespace:
      return TextStyle(color: base);
  }
}

/// 把命令行渲染成带语法着色的 `TextSpan` 列表(子 span 仅覆盖颜色 / 字重,
/// 字体与字号由父级 `Text` 继承)。
List<InlineSpan> highlightCommandSpans(String command, {Color? baseColor}) {
  final base = baseColor ?? AppColors.textPrimary;
  return [
    for (final token in tokenizeCommand(command))
      TextSpan(text: token.text, style: _styleFor(token.type, base)),
  ];
}

/// 便捷:一条命令的等宽高亮文本组件。
class HighlightedCommand extends StatelessWidget {
  final String command;
  final double fontSize;
  final Color? baseColor;
  final int? maxLines;
  final TextOverflow overflow;

  const HighlightedCommand(
    this.command, {
    super.key,
    this.fontSize = 12.5,
    this.baseColor,
    this.maxLines = 1,
    this.overflow = TextOverflow.ellipsis,
  });

  @override
  Widget build(BuildContext context) {
    return Text.rich(
      TextSpan(children: highlightCommandSpans(command, baseColor: baseColor)),
      maxLines: maxLines,
      overflow: overflow,
      style: TextStyle(
        fontFamily: AppFonts.mono,
        fontSize: fontSize,
        height: 1.35,
      ),
    );
  }
}
