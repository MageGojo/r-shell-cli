import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../../shared/theme/app_colors.dart';
import '../../../shared/theme/app_dimens.dart';
import '../command_highlighter.dart';
import '../block_models.dart';

/// 单个命令块:头(状态 + `cwd ❯ command` + 耗时 + 退出码 + 操作)+ 输出体。
///
/// Warp 式:头与体成一张卡,左侧用一条按状态着色的竖条标识成败。点头可折叠输出。
class CommandBlockTile extends StatefulWidget {
  const CommandBlockTile({
    super.key,
    required this.block,
    required this.onRerun,
    required this.onToggleCollapse,
    this.fontSize = 13,
  });

  final CommandBlock block;
  final VoidCallback onRerun;
  final VoidCallback onToggleCollapse;

  /// 等宽文本字号(命令头与输出体),由设置页驱动。
  final double fontSize;

  @override
  State<CommandBlockTile> createState() => _CommandBlockTileState();
}

class _CommandBlockTileState extends State<CommandBlockTile> {
  bool _hover = false;

  CommandBlock get _b => widget.block;

  Color get _accent => switch (_b.status) {
    BlockStatus.running => AppColors.accent,
    BlockStatus.success => AppColors.online,
    BlockStatus.failure => AppColors.danger,
    BlockStatus.error => AppColors.danger,
  };

  void _copy(String text, String toast) {
    Clipboard.setData(ClipboardData(text: text));
    ScaffoldMessenger.of(context)
      ..clearSnackBars()
      ..showSnackBar(
        SnackBar(
          content: Text(toast),
          behavior: SnackBarBehavior.floating,
          width: 240,
          backgroundColor: AppColors.surface3,
          duration: const Duration(milliseconds: 1200),
        ),
      );
  }

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: Container(
        margin: const EdgeInsets.only(bottom: AppSpacing.s3),
        decoration: BoxDecoration(
          color: AppColors.surface2,
          borderRadius: BorderRadius.circular(AppRadius.lg),
          border: Border.all(color: AppColors.borderSubtle),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            _header(),
            if (!_b.collapsed && (_b.hasOutput || _b.isRunning)) _body(),
          ],
        ),
      ),
    );
  }

  Widget _header() {
    return InkWell(
      borderRadius: BorderRadius.vertical(
        top: const Radius.circular(AppRadius.lg),
        bottom: Radius.circular(_b.collapsed ? AppRadius.lg : 0),
      ),
      onTap: widget.onToggleCollapse,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(
          AppSpacing.s2,
          AppSpacing.s2,
          AppSpacing.s2,
          AppSpacing.s2,
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _statusGlyph(),
            const SizedBox(width: AppSpacing.s2),
            Expanded(child: _prompt()),
            const SizedBox(width: AppSpacing.s2),
            _meta(),
          ],
        ),
      ),
    );
  }

  /// 左侧状态字形:运行中转圈、成功对勾、失败/错误叉。
  Widget _statusGlyph() {
    Widget glyph;
    if (_b.isRunning) {
      glyph = const SizedBox(
        width: 13,
        height: 13,
        child: CircularProgressIndicator(
          strokeWidth: 2,
          valueColor: AlwaysStoppedAnimation(AppColors.accent),
        ),
      );
    } else {
      glyph = Icon(
        switch (_b.status) {
          BlockStatus.success => Icons.check_circle,
          BlockStatus.failure => Icons.cancel,
          BlockStatus.error => Icons.error_outline,
          BlockStatus.running => Icons.circle,
        },
        size: 15,
        color: _accent,
      );
    }
    return Padding(padding: const EdgeInsets.only(top: 2), child: glyph);
  }

  /// `cwd ❯ command`:cwd 弱化、命令高亮等宽。
  Widget _prompt() {
    return Text.rich(
      TextSpan(
        style: TextStyle(
          fontFamily: AppFonts.mono,
          fontSize: widget.fontSize,
          height: 1.4,
        ),
        children: [
          TextSpan(
            text: compactPath(_b.cwd),
            style: const TextStyle(color: AppColors.textMuted),
          ),
          const TextSpan(
            text: '  ❯  ',
            style: TextStyle(
              color: AppColors.accent,
              fontWeight: FontWeight.w700,
            ),
          ),
          ...highlightCommandSpans(_b.command),
        ],
      ),
    );
  }

  Widget _meta() {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (_hover) ...[
          _iconAction(
            Icons.content_copy,
            '复制命令',
            () => _copy(_b.command, '已复制命令'),
          ),
          if (_b.hasOutput)
            _iconAction(
              Icons.notes_outlined,
              '复制输出',
              () => _copy(_b.output, '已复制输出'),
            ),
          _iconAction(Icons.refresh, '重跑', widget.onRerun),
          const SizedBox(width: AppSpacing.s1),
        ],
        if (_b.duration != null)
          Padding(
            padding: const EdgeInsets.only(right: 6),
            child: Text(
              formatBlockDuration(_b.duration!),
              style: const TextStyle(
                fontFamily: AppFonts.mono,
                fontSize: 11,
                color: AppColors.textMuted,
              ),
            ),
          ),
        _exitChip(),
        const SizedBox(width: 2),
        Icon(
          _b.collapsed ? Icons.expand_more : Icons.expand_less,
          size: 16,
          color: AppColors.textMuted,
        ),
      ],
    );
  }

  Widget _exitChip() {
    if (_b.isRunning) return const SizedBox.shrink();
    if (_b.status == BlockStatus.error) {
      return _chip('错误', AppColors.danger);
    }
    final code = _b.exitCode;
    if (code == null) return const SizedBox.shrink(); // 未知退出码(非 POSIX)
    if (code == 0) return _chip('0', AppColors.online);
    return _chip('exit $code', AppColors.danger);
  }

  Widget _chip(String label, Color color) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.14),
        borderRadius: BorderRadius.circular(AppRadius.sm),
      ),
      child: Text(
        label,
        style: TextStyle(
          fontFamily: AppFonts.mono,
          fontSize: 11,
          fontWeight: FontWeight.w600,
          color: color,
        ),
      ),
    );
  }

  Widget _iconAction(IconData icon, String tip, VoidCallback onTap) {
    return Tooltip(
      message: tip,
      child: InkWell(
        borderRadius: BorderRadius.circular(AppRadius.sm),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.all(4),
          child: Icon(icon, size: 14, color: AppColors.textSecondary),
        ),
      ),
    );
  }

  Widget _body() {
    final empty = !_b.hasOutput && _b.isRunning;
    return Container(
      width: double.infinity,
      constraints: const BoxConstraints(maxHeight: 360),
      decoration: const BoxDecoration(
        color: AppColors.canvas,
        borderRadius: BorderRadius.vertical(bottom: Radius.circular(AppRadius.lg)),
        border: Border(top: BorderSide(color: AppColors.borderSubtle)),
      ),
      child: Scrollbar(
        child: SingleChildScrollView(
          padding: const EdgeInsets.fromLTRB(
            AppSpacing.s3,
            AppSpacing.s2,
            AppSpacing.s3,
            AppSpacing.s3,
          ),
          child: empty
              ? Text(
                  '运行中…',
                  style: TextStyle(
                    fontFamily: AppFonts.mono,
                    fontSize: widget.fontSize - 0.5,
                    color: AppColors.textMuted,
                  ),
                )
              : SelectableText(
                  _b.output.trimRight(),
                  style: TextStyle(
                    fontFamily: AppFonts.mono,
                    fontSize: widget.fontSize - 0.5,
                    height: 1.45,
                    color: AppColors.textSecondary,
                  ),
                ),
        ),
      ),
    );
  }
}
