import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../../shared/theme/app_colors.dart';
import '../../../shared/theme/app_dimens.dart';
import '../command_highlighter.dart';

/// 钉底输入条:语法高亮输入框(回车执行 · ↑/↓ 历史/选建议 · Tab 接受 · Esc 收起)
/// + 命令提示下拉(最近历史 + 常用命令)+ 运行 / 忙碌按钮。
class BlockInputBar extends StatefulWidget {
  const BlockInputBar({
    super.key,
    required this.busy,
    required this.onSubmit,
    required this.onHistoryPrev,
    required this.onHistoryNext,
    required this.suggest,
    this.fontSize = 13,
  });

  final bool busy;

  /// 等宽输入字号(由设置页「命令块字号」驱动)。
  final double fontSize;
  final ValueChanged<String> onSubmit;

  /// 返回要回填的命令(null = 无更多历史,不改输入)。
  final String? Function() onHistoryPrev;
  final String? Function() onHistoryNext;

  /// 按前缀给出命令建议(最近历史 + 常用命令)。
  final List<String> Function(String prefix) suggest;

  @override
  State<BlockInputBar> createState() => _BlockInputBarState();
}

class _BlockInputBarState extends State<BlockInputBar> {
  final _CommandEditingController _controller = _CommandEditingController();
  final FocusNode _focus = FocusNode();

  List<String> _suggestions = const [];
  int _selected = 0;

  /// Esc 暂时收起补全,直到用户下次键入。
  bool _dismissed = false;

  bool get _showSuggestions =>
      !_dismissed && !widget.busy && _suggestions.isNotEmpty;

  @override
  void dispose() {
    _controller.dispose();
    _focus.dispose();
    super.dispose();
  }

  /// 用户键入(仅用户编辑触发,程序改值不触发)→ 重算建议。
  void _onUserTyped(String text) {
    setState(() {
      _suggestions = text.trim().isEmpty ? const [] : widget.suggest(text);
      _selected = 0;
      _dismissed = false;
    });
  }

  void _submit() {
    final text = _controller.text;
    if (text.trim().isEmpty || widget.busy) return;
    widget.onSubmit(text);
    _controller.clear();
    setState(() {
      _suggestions = const [];
      _dismissed = false;
    });
    _focus.requestFocus();
  }

  /// 程序回填文本(历史 / 接受建议),不重新弹出建议。
  void _fill(String? value, {bool collapse = true}) {
    if (value == null) return;
    _controller.value = TextEditingValue(
      text: value,
      selection: TextSelection.collapsed(offset: value.length),
    );
    if (collapse) {
      setState(() {
        _suggestions = const [];
        _dismissed = true;
      });
    }
  }

  void _accept(String suggestion) {
    _fill(suggestion);
    _focus.requestFocus();
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    final key = event.logicalKey;
    final showing = _showSuggestions;

    if (key == LogicalKeyboardKey.arrowUp) {
      if (showing) {
        setState(() {
          _selected = (_selected - 1 + _suggestions.length) % _suggestions.length;
        });
      } else {
        _fill(widget.onHistoryPrev());
      }
      return KeyEventResult.handled;
    }
    if (key == LogicalKeyboardKey.arrowDown) {
      if (showing) {
        setState(() => _selected = (_selected + 1) % _suggestions.length);
      } else {
        _fill(widget.onHistoryNext());
      }
      return KeyEventResult.handled;
    }
    if (key == LogicalKeyboardKey.tab) {
      if (showing) _accept(_suggestions[_selected]);
      return KeyEventResult.handled; // 始终消费,避免焦点跳走
    }
    if (key == LogicalKeyboardKey.escape && showing) {
      setState(() => _dismissed = true);
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: const BoxDecoration(
        color: AppColors.surface1,
        border: Border(top: BorderSide(color: AppColors.borderSubtle)),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (_showSuggestions) _suggestionList(),
          Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: AppSpacing.s4,
              vertical: AppSpacing.s3,
            ),
            child: Row(
              children: [
                const Text(
                  '❯',
                  style: TextStyle(
                    fontFamily: AppFonts.mono,
                    fontSize: 16,
                    fontWeight: FontWeight.w700,
                    color: AppColors.accent,
                  ),
                ),
                const SizedBox(width: AppSpacing.s3),
                Expanded(
                  child: Focus(
                    onKeyEvent: _onKey,
                    child: TextField(
                      controller: _controller,
                      focusNode: _focus,
                      autofocus: true,
                      cursorColor: AppColors.accent,
                      onChanged: _onUserTyped,
                      style: TextStyle(
                        fontFamily: AppFonts.mono,
                        fontSize: widget.fontSize + 0.5,
                        color: AppColors.textPrimary,
                      ),
                      decoration: InputDecoration(
                        isDense: true,
                        border: InputBorder.none,
                        hintText: '输入命令…  （回车执行 · ↑/↓ 历史 · Tab 补全）',
                        hintStyle: TextStyle(
                          fontFamily: AppFonts.mono,
                          fontSize: widget.fontSize,
                          color: AppColors.textMuted,
                        ),
                      ),
                      onSubmitted: (_) => _submit(),
                    ),
                  ),
                ),
                const SizedBox(width: AppSpacing.s3),
                _runButton(),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _suggestionList() {
    return Container(
      decoration: const BoxDecoration(
        border: Border(bottom: BorderSide(color: AppColors.borderSubtle)),
      ),
      padding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.s3,
        vertical: AppSpacing.s2,
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (var i = 0; i < _suggestions.length; i++)
            _SuggestionRow(
              command: _suggestions[i],
              selected: i == _selected,
              onTap: () => _accept(_suggestions[i]),
              onHover: () {
                if (_selected != i) setState(() => _selected = i);
              },
            ),
        ],
      ),
    );
  }

  Widget _runButton() {
    if (widget.busy) {
      return const SizedBox(
        width: 30,
        height: 30,
        child: Padding(
          padding: EdgeInsets.all(7),
          child: CircularProgressIndicator(
            strokeWidth: 2,
            valueColor: AlwaysStoppedAnimation(AppColors.accent),
          ),
        ),
      );
    }
    return Tooltip(
      message: '执行 (Enter)',
      child: Material(
        color: AppColors.accent,
        borderRadius: BorderRadius.circular(AppRadius.md),
        child: InkWell(
          borderRadius: BorderRadius.circular(AppRadius.md),
          hoverColor: AppColors.accentHover,
          onTap: _submit,
          child: const SizedBox(
            width: 30,
            height: 30,
            child: Icon(
              Icons.keyboard_return,
              size: 16,
              color: AppColors.textInverse,
            ),
          ),
        ),
      ),
    );
  }
}

/// 补全列表中的一行:左侧来源图标 + 高亮命令文本;选中态高亮背景。
class _SuggestionRow extends StatelessWidget {
  const _SuggestionRow({
    required this.command,
    required this.selected,
    required this.onTap,
    required this.onHover,
  });

  final String command;
  final bool selected;
  final VoidCallback onTap;
  final VoidCallback onHover;

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      onEnter: (_) => onHover(),
      child: Material(
        color: selected ? AppColors.surfaceActive : Colors.transparent,
        borderRadius: BorderRadius.circular(AppRadius.sm),
        child: InkWell(
          borderRadius: BorderRadius.circular(AppRadius.sm),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: AppSpacing.s2,
              vertical: 6,
            ),
            child: Row(
              children: [
                Icon(
                  Icons.bolt_outlined,
                  size: 13,
                  color: selected ? AppColors.accent : AppColors.textMuted,
                ),
                const SizedBox(width: AppSpacing.s2),
                Expanded(
                  child: Text.rich(
                    TextSpan(children: highlightCommandSpans(command)),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                      fontFamily: AppFonts.mono,
                      fontSize: 12.5,
                    ),
                  ),
                ),
                if (selected) ...[
                  const SizedBox(width: AppSpacing.s2),
                  const _KeyHint('Tab'),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _KeyHint extends StatelessWidget {
  const _KeyHint(this.label);
  final String label;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
      decoration: BoxDecoration(
        color: AppColors.surface3,
        borderRadius: BorderRadius.circular(AppRadius.sm),
        border: Border.all(color: AppColors.borderDefault),
      ),
      child: Text(
        label,
        style: const TextStyle(
          fontFamily: AppFonts.mono,
          fontSize: 10,
          color: AppColors.textMuted,
        ),
      ),
    );
  }
}

/// 输入框的语法高亮控制器:重写 [buildTextSpan],把当前命令分词着色。
class _CommandEditingController extends TextEditingController {
  @override
  TextSpan buildTextSpan({
    required BuildContext context,
    TextStyle? style,
    required bool withComposing,
  }) {
    final base = style?.color ?? AppColors.textPrimary;
    return TextSpan(
      style: style,
      children: highlightCommandSpans(text, baseColor: base),
    );
  }
}
