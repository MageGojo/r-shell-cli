import 'package:flutter/material.dart';

import '../../shared/theme/app_colors.dart';
import '../../shared/theme/app_dimens.dart';
import '../../src/rust/api/connections.dart';
import 'block_models.dart';
import 'block_session.dart';
import 'block_terminal_controller.dart';
import 'widgets/block_input_bar.dart';
import 'widgets/command_block_tile.dart';

/// 命令块终端整页:**标签条(每个标签固定绑定一个执行目标)** + 当前标签的块流
/// (上→下,自动滚底)+ 钉底输入条。
///
/// 「在哪个标签执行哪台服务器的命令」由标签本身表达;不再有面板内的目标下拉选择器。
/// 见 docs/gui/11-命令块终端原型.md。
class BlockTerminalPanel extends StatefulWidget {
  const BlockTerminalPanel({
    super.key,
    required this.controller,
    required this.connections,
    this.fontSize = 13,
  });

  final BlockTerminalController controller;
  final List<ConnectionDto> connections;

  /// 命令块文本字号(由设置页「命令块字号」驱动)。
  final double fontSize;

  @override
  State<BlockTerminalPanel> createState() => _BlockTerminalPanelState();
}

class _BlockTerminalPanelState extends State<BlockTerminalPanel> {
  final ScrollController _scroll = ScrollController();
  String _lastSig = '';

  BlockTerminalController get _c => widget.controller;

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  void _scrollToBottom() {
    if (!_scroll.hasClients) return;
    _scroll.jumpTo(_scroll.position.maxScrollExtent);
  }

  /// 活动标签 / 块数 / 忙碌态变化即滚到底(切标签或新块加入 / 输出回填后)。
  void _maybeAutoScroll(BlockSession s) {
    final sig = '${identityHashCode(s)}-${s.blocks.length}-${s.busy}';
    if (sig == _lastSig) return;
    _lastSig = sig;
    WidgetsBinding.instance.addPostFrameCallback((_) => _scrollToBottom());
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: _c,
      builder: (context, _) {
        final active = _c.active;
        return Column(
          children: [
            _TabStrip(controller: _c, connections: widget.connections),
            const Divider(height: 1, color: AppColors.borderSubtle),
            if (active == null)
              const Expanded(child: _EmptyTabsState())
            else
              Expanded(
                child: ListenableBuilder(
                  listenable: active,
                  builder: (context, _) => _sessionBody(active),
                ),
              ),
          ],
        );
      },
    );
  }

  Widget _sessionBody(BlockSession s) {
    _maybeAutoScroll(s);
    return Column(
      children: [
        _CwdBar(session: s),
        Expanded(
          child: s.blocks.isEmpty
              ? const _EmptyState()
              : ListView.builder(
                  controller: _scroll,
                  padding: const EdgeInsets.fromLTRB(
                    AppSpacing.s4,
                    AppSpacing.s4,
                    AppSpacing.s4,
                    AppSpacing.s2,
                  ),
                  itemCount: s.blocks.length,
                  itemBuilder: (context, i) {
                    final b = s.blocks[i];
                    return CommandBlockTile(
                      key: ValueKey(b.id),
                      block: b,
                      fontSize: widget.fontSize,
                      onRerun: () => s.rerun(b),
                      onToggleCollapse: () => s.toggleCollapse(b),
                    );
                  },
                ),
        ),
        BlockInputBar(
          // 切标签即换一套输入状态(各标签独立草稿)。
          key: ValueKey(identityHashCode(s)),
          busy: s.busy,
          fontSize: widget.fontSize,
          onSubmit: s.run,
          onHistoryPrev: s.historyPrev,
          onHistoryNext: s.historyNext,
          suggest: s.suggest,
        ),
      ],
    );
  }
}

/// 顶部标签条:每个标签 = 一个执行目标(本机 / 连接)+ 关闭;末尾 `+` 开本机标签。
class _TabStrip extends StatelessWidget {
  const _TabStrip({required this.controller, required this.connections});

  final BlockTerminalController controller;
  final List<ConnectionDto> connections;

  @override
  Widget build(BuildContext context) {
    final sessions = controller.sessions;
    return Container(
      height: AppLayout.tabBarHeight + 6,
      color: AppColors.surface1,
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.s2),
      child: Row(
        children: [
          Expanded(
            child: ListView.builder(
              scrollDirection: Axis.horizontal,
              itemCount: sessions.length,
              itemBuilder: (context, i) => _TabChip(
                session: sessions[i],
                active: i == controller.activeIndex,
                onTap: () => controller.setActive(i),
                onClose: () => controller.closeAt(i),
              ),
            ),
          ),
          Tooltip(
            message: '新建本机标签',
            child: InkWell(
              borderRadius: BorderRadius.circular(AppRadius.sm),
              onTap: controller.openLocal,
              child: const Padding(
                padding: EdgeInsets.all(7),
                child: Icon(Icons.add, size: 18, color: AppColors.textSecondary),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _TabChip extends StatefulWidget {
  const _TabChip({
    required this.session,
    required this.active,
    required this.onTap,
    required this.onClose,
  });

  final BlockSession session;
  final bool active;
  final VoidCallback onTap;
  final VoidCallback onClose;

  @override
  State<_TabChip> createState() => _TabChipState();
}

class _TabChipState extends State<_TabChip> {
  bool _hover = false;

  bool get _isAdb => widget.session.connection?.protocol.toUpperCase() == 'ADB';

  ({IconData icon, Color color}) get _glyph {
    if (widget.session.isLocal) {
      return (icon: Icons.computer_outlined, color: AppColors.accentTeal);
    }
    return _isAdb
        ? (icon: Icons.android, color: AppColors.online)
        : (icon: Icons.dns_outlined, color: AppColors.accent);
  }

  @override
  Widget build(BuildContext context) {
    final g = _glyph;
    final active = widget.active;
    return MouseRegion(
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: GestureDetector(
        onTap: widget.onTap,
        child: Container(
          margin: const EdgeInsets.symmetric(vertical: 6, horizontal: 3),
          padding: const EdgeInsets.only(left: 10, right: 4),
          decoration: BoxDecoration(
            color: active ? AppColors.surfaceActive : AppColors.surface3,
            borderRadius: BorderRadius.circular(AppRadius.md),
            border: Border.all(
              color: active ? AppColors.borderStrong : AppColors.borderSubtle,
            ),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(g.icon, size: 14, color: g.color),
              const SizedBox(width: 7),
              ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 140),
                child: Text(
                  widget.session.title,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    fontSize: 12.5,
                    fontWeight: active ? FontWeight.w600 : FontWeight.w500,
                    color: active ? AppColors.textPrimary : AppColors.textSecondary,
                  ),
                ),
              ),
              const SizedBox(width: 4),
              // 活动或悬停时显示关闭按钮。
              SizedBox(
                width: 20,
                height: 20,
                child: (active || _hover)
                    ? InkWell(
                        borderRadius: BorderRadius.circular(AppRadius.sm),
                        onTap: widget.onClose,
                        child: const Icon(
                          Icons.close,
                          size: 14,
                          color: AppColors.textMuted,
                        ),
                      )
                    : null,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// 活动标签的 cwd 栏:目标无需再选(由标签表达),这里只展示当前工作目录 + 清空。
class _CwdBar extends StatelessWidget {
  const _CwdBar({required this.session});

  final BlockSession session;

  @override
  Widget build(BuildContext context) {
    final cwd = session.cwd;
    return Container(
      height: 30,
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.s4),
      decoration: const BoxDecoration(
        color: AppColors.surface1,
        border: Border(bottom: BorderSide(color: AppColors.borderSubtle)),
      ),
      child: Row(
        children: [
          const Icon(Icons.folder_outlined, size: 13, color: AppColors.textMuted),
          const SizedBox(width: 6),
          Expanded(
            child: Text(
              cwd.trim().isEmpty
                  ? (session.isLocal ? '本机 · 命令块' : '~ · 命令块')
                  : compactPath(cwd, maxLen: 72),
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(
                fontFamily: AppFonts.mono,
                fontSize: 12,
                color: AppColors.textSecondary,
              ),
            ),
          ),
          if (session.blocks.isNotEmpty)
            Tooltip(
              message: '清空命令块',
              child: InkWell(
                borderRadius: BorderRadius.circular(AppRadius.sm),
                onTap: session.clear,
                child: const Padding(
                  padding: EdgeInsets.all(4),
                  child: Icon(
                    Icons.delete_sweep_outlined,
                    size: 16,
                    color: AppColors.textSecondary,
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// 当前标签还没有命令块时的空态。
class _EmptyState extends StatelessWidget {
  const _EmptyState();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            Icons.view_agenda_outlined,
            size: 44,
            color: AppColors.textMuted.withValues(alpha: 0.5),
          ),
          const SizedBox(height: AppSpacing.s4),
          const Text(
            '输入命令开始',
            style: TextStyle(fontSize: 15, color: AppColors.textSecondary),
          ),
          const SizedBox(height: AppSpacing.s2),
          const Text(
            '每条命令与它的输出成一块 · 从上往下读',
            style: TextStyle(fontSize: 12, color: AppColors.textMuted),
          ),
        ],
      ),
    );
  }
}

/// 所有标签都被关闭时的空态(引导从侧栏打开连接,或 + 开本机)。
class _EmptyTabsState extends StatelessWidget {
  const _EmptyTabsState();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            Icons.tab_unselected,
            size: 44,
            color: AppColors.textMuted.withValues(alpha: 0.5),
          ),
          const SizedBox(height: AppSpacing.s4),
          const Text(
            '没有打开的标签',
            style: TextStyle(fontSize: 15, color: AppColors.textSecondary),
          ),
          const SizedBox(height: AppSpacing.s2),
          const Text(
            '双击左侧连接打开它的命令块 · 或点右上「+」开本机',
            style: TextStyle(fontSize: 12, color: AppColors.textMuted),
          ),
        ],
      ),
    );
  }
}
