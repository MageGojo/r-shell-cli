import 'package:flutter/material.dart';

import '../../shared/theme/app_colors.dart';
import '../../shared/theme/app_dimens.dart';
import '../../shared/widgets/status_dot.dart';
import '../../src/rust/api/connections.dart';

/// 连接主页（导航「连接」专属）：把所有连接以卡片网格按分组铺开，
/// 每张卡可直接打开终端 / 文件 / 监控、编辑、删除，是应用的着陆页。
///
/// 区别于左侧常驻的连接树（快速切换）与终端视图右侧的监控侧栏。
class ConnectionsDashboard extends StatelessWidget {
  final List<ConnectionDto> connections;
  final String? selectedId;
  final String? loadError;
  final ValueChanged<ConnectionDto>? onSelect;
  final ValueChanged<ConnectionDto>? onOpenTerminal;
  final ValueChanged<ConnectionDto>? onOpenFiles;
  final ValueChanged<ConnectionDto>? onOpenMonitor;
  final ValueChanged<ConnectionDto>? onEdit;
  final ValueChanged<ConnectionDto>? onDelete;
  final VoidCallback? onAdd;
  final VoidCallback? onOpenLocalMonitor;

  const ConnectionsDashboard({
    super.key,
    required this.connections,
    this.selectedId,
    this.loadError,
    this.onSelect,
    this.onOpenTerminal,
    this.onOpenFiles,
    this.onOpenMonitor,
    this.onEdit,
    this.onDelete,
    this.onAdd,
    this.onOpenLocalMonitor,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _header(),
        const Divider(height: 1, color: AppColors.borderSubtle),
        Expanded(child: _body()),
      ],
    );
  }

  // ── 顶栏 ────────────────────────────────────────────────────
  Widget _header() {
    final online = connections
        .where((c) => connStatusFrom(c.status) == ConnStatus.connected)
        .length;
    return Container(
      height: AppLayout.tabBarHeight + 8,
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.s5),
      child: Row(
        children: [
          const Icon(Icons.dns_outlined, size: 18, color: AppColors.accent),
          const SizedBox(width: AppSpacing.s2),
          const Text(
            '连接',
            style: TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w700,
              color: AppColors.textPrimary,
            ),
          ),
          const SizedBox(width: AppSpacing.s3),
          Text(
            '${connections.length} 个连接 · $online 在线',
            style: const TextStyle(fontSize: 12, color: AppColors.textMuted),
          ),
          const Spacer(),
          _addButton(),
        ],
      ),
    );
  }

  Widget _addButton() {
    return Material(
      color: AppColors.accent,
      borderRadius: BorderRadius.circular(AppRadius.md),
      child: InkWell(
        borderRadius: BorderRadius.circular(AppRadius.md),
        hoverColor: AppColors.accentHover,
        onTap: onAdd,
        child: const Padding(
          padding: EdgeInsets.symmetric(horizontal: 14, vertical: 8),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.add, size: 16, color: AppColors.textInverse),
              SizedBox(width: 6),
              Text(
                '添加连接',
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: AppColors.textInverse,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  // ── 主体 ────────────────────────────────────────────────────
  Widget _body() {
    if (loadError != null) {
      return _hint(Icons.error_outline, AppColors.danger, '读取连接失败', loadError);
    }
    final groups = _grouped();
    return LayoutBuilder(
      builder: (context, constraints) {
        final w = constraints.maxWidth - AppSpacing.s5 * 2;
        const gap = AppSpacing.s4;
        const target = 320.0;
        final cols = (w / target).floor().clamp(1, 4);
        final cardW = (w - gap * (cols - 1)) / cols;
        final children = <Widget>[
          _localCard(),
          const SizedBox(height: AppSpacing.s6),
        ];
        if (connections.isEmpty) {
          children.add(_emptyInline());
        } else {
          groups.forEach((folder, items) {
            children.add(_groupHeader(folder, items.length));
            children.add(const SizedBox(height: AppSpacing.s3));
            children.add(
              Wrap(
                spacing: gap,
                runSpacing: gap,
                children: [
                  for (final c in items)
                    SizedBox(width: cardW, child: _ConnectionCard(
                      connection: c,
                      selected: c.id == selectedId,
                      onSelect: onSelect,
                      onOpenTerminal: onOpenTerminal,
                      onOpenFiles: onOpenFiles,
                      onOpenMonitor: onOpenMonitor,
                      onEdit: onEdit,
                      onDelete: onDelete,
                    )),
                ],
              ),
            );
            children.add(const SizedBox(height: AppSpacing.s6));
          });
        }
        return ListView(
          padding: const EdgeInsets.all(AppSpacing.s5),
          children: children,
        );
      },
    );
  }

  /// 「本机监控」入口卡：查看运行 GUI 这台电脑自身的资源（无需任何连接）。
  Widget _localCard() {
    return Material(
      color: AppColors.surface3,
      borderRadius: BorderRadius.circular(AppRadius.xl),
      child: InkWell(
        borderRadius: BorderRadius.circular(AppRadius.xl),
        hoverColor: AppColors.surfaceHover,
        onTap: onOpenLocalMonitor,
        child: Container(
          padding: const EdgeInsets.all(AppSpacing.s4),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(AppRadius.xl),
            border: Border.all(color: AppColors.borderSubtle),
          ),
          child: Row(
            children: [
              Container(
                width: 40,
                height: 40,
                decoration: BoxDecoration(
                  gradient: AppColors.accentGradient,
                  borderRadius: BorderRadius.circular(AppRadius.md),
                ),
                child: const Icon(Icons.computer, size: 20, color: AppColors.textInverse),
              ),
              const SizedBox(width: AppSpacing.s3),
              const Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      '本机监控',
                      style: TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w700,
                        color: AppColors.textPrimary,
                      ),
                    ),
                    SizedBox(height: 2),
                    Text(
                      '查看这台电脑的 CPU / 内存 / 磁盘 / 网络（无需连接）',
                      style: TextStyle(fontSize: 11.5, color: AppColors.textMuted),
                    ),
                  ],
                ),
              ),
              const SizedBox(width: AppSpacing.s3),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 7),
                decoration: BoxDecoration(
                  color: AppColors.accent.withValues(alpha: 0.14),
                  borderRadius: BorderRadius.circular(AppRadius.md),
                ),
                child: const Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Icon(Icons.insights_outlined, size: 14, color: AppColors.accent),
                    SizedBox(width: 6),
                    Text(
                      '查看监控',
                      style: TextStyle(
                        fontSize: 12,
                        fontWeight: FontWeight.w600,
                        color: AppColors.accent,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  /// 无连接时的内联提示（local 卡片下方），区别于整页居中空态。
  Widget _emptyInline() {
    return Container(
      padding: const EdgeInsets.symmetric(vertical: AppSpacing.s10),
      alignment: Alignment.center,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            Icons.dns_outlined,
            size: 40,
            color: AppColors.textMuted.withValues(alpha: 0.5),
          ),
          const SizedBox(height: AppSpacing.s3),
          const Text(
            '还没有任何连接',
            style: TextStyle(fontSize: 14, color: AppColors.textSecondary),
          ),
          const SizedBox(height: AppSpacing.s2),
          const Text(
            '点右上角「添加连接」新建一个 SSH 或 ADB 连接',
            style: TextStyle(fontSize: 12, color: AppColors.textMuted),
          ),
        ],
      ),
    );
  }

  Map<String, List<ConnectionDto>> _grouped() {
    final map = <String, List<ConnectionDto>>{};
    for (final c in connections) {
      (map[c.folder.isEmpty ? '未分组' : c.folder] ??= <ConnectionDto>[]).add(c);
    }
    return map;
  }

  Widget _groupHeader(String folder, int count) {
    return Row(
      children: [
        const Icon(Icons.folder_outlined, size: 15, color: AppColors.textSecondary),
        const SizedBox(width: 8),
        Text(
          folder,
          style: const TextStyle(
            fontSize: 13,
            fontWeight: FontWeight.w600,
            color: AppColors.textSecondary,
          ),
        ),
        const SizedBox(width: 8),
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 1),
          decoration: BoxDecoration(
            color: AppColors.surface3,
            borderRadius: BorderRadius.circular(AppRadius.sm),
          ),
          child: Text(
            '$count',
            style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
          ),
        ),
      ],
    );
  }

  Widget _hint(IconData icon, Color color, String title, String? detail) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(AppSpacing.s6),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 34, color: color.withValues(alpha: 0.7)),
            const SizedBox(height: AppSpacing.s3),
            Text(
              title,
              style: const TextStyle(fontSize: 14, color: AppColors.textSecondary),
            ),
            if (detail != null) ...[
              const SizedBox(height: AppSpacing.s2),
              Text(
                detail,
                textAlign: TextAlign.center,
                maxLines: 4,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontSize: 12, color: AppColors.textMuted),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// 单张连接卡片。
class _ConnectionCard extends StatelessWidget {
  final ConnectionDto connection;
  final bool selected;
  final ValueChanged<ConnectionDto>? onSelect;
  final ValueChanged<ConnectionDto>? onOpenTerminal;
  final ValueChanged<ConnectionDto>? onOpenFiles;
  final ValueChanged<ConnectionDto>? onOpenMonitor;
  final ValueChanged<ConnectionDto>? onEdit;
  final ValueChanged<ConnectionDto>? onDelete;

  const _ConnectionCard({
    required this.connection,
    required this.selected,
    this.onSelect,
    this.onOpenTerminal,
    this.onOpenFiles,
    this.onOpenMonitor,
    this.onEdit,
    this.onDelete,
  });

  @override
  Widget build(BuildContext context) {
    final c = connection;
    final status = connStatusFrom(c.status);
    final isAdb = c.protocol == 'ADB';
    return Material(
      color: AppColors.surface3,
      borderRadius: BorderRadius.circular(AppRadius.xl),
      child: InkWell(
        borderRadius: BorderRadius.circular(AppRadius.xl),
        hoverColor: AppColors.surfaceHover,
        onTap: () => onSelect?.call(c),
        onDoubleTap: () => onOpenTerminal?.call(c),
        child: Container(
          padding: const EdgeInsets.all(AppSpacing.s4),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(AppRadius.xl),
            border: Border.all(
              color: selected ? AppColors.accent : AppColors.borderSubtle,
              width: selected ? 1.5 : 1,
            ),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Row(
                children: [
                  StatusDot(status),
                  const SizedBox(width: AppSpacing.s3),
                  Expanded(
                    child: Text(
                      c.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w600,
                        color: AppColors.textPrimary,
                      ),
                    ),
                  ),
                  const SizedBox(width: 6),
                  _protocolBadge(isAdb),
                ],
              ),
              const SizedBox(height: AppSpacing.s3),
              Text(
                isAdb ? 'adb · ${c.host}:${c.port}' : '${c.username}@${c.host}:${c.port}',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                  fontSize: 12,
                  color: AppColors.textMuted,
                  fontFamily: AppFonts.mono,
                ),
              ),
              const SizedBox(height: 6),
              Row(
                children: [
                  Icon(
                    isAdb
                        ? Icons.usb
                        : (c.authMethod == 'publickey'
                            ? Icons.vpn_key_outlined
                            : Icons.password),
                    size: 12,
                    color: AppColors.textMuted,
                  ),
                  const SizedBox(width: 5),
                  Text(
                    isAdb
                        ? '安卓调试桥'
                        : (c.authMethod == 'publickey' ? '公钥认证' : '密码认证'),
                    style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
                  ),
                  const Spacer(),
                  Text(
                    status.label,
                    style: TextStyle(fontSize: 11, color: status.color),
                  ),
                ],
              ),
              const SizedBox(height: AppSpacing.s3),
              const Divider(height: 1, color: AppColors.borderSubtle),
              const SizedBox(height: AppSpacing.s2),
              Row(
                children: [
                  _action(Icons.terminal, '终端', AppColors.accent,
                      () => onOpenTerminal?.call(c)),
                  _action(Icons.folder_outlined, '文件', AppColors.accentIndigo,
                      () => onOpenFiles?.call(c)),
                  _action(Icons.insights_outlined, '监控', AppColors.accentTeal,
                      () => onOpenMonitor?.call(c)),
                  const Spacer(),
                  _iconButton(Icons.edit_outlined, '编辑', () => onEdit?.call(c)),
                  _iconButton(Icons.delete_outline, '删除', () => onDelete?.call(c),
                      danger: true),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _protocolBadge(bool isAdb) {
    final color = isAdb ? AppColors.online : AppColors.accent;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.14),
        borderRadius: BorderRadius.circular(AppRadius.sm),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (isAdb) ...[
            const Icon(Icons.android, size: 11, color: AppColors.online),
            const SizedBox(width: 3),
          ],
          Text(
            isAdb ? 'ADB' : 'SSH',
            style: TextStyle(
              fontSize: 10,
              fontWeight: FontWeight.w700,
              color: color,
            ),
          ),
        ],
      ),
    );
  }

  Widget _action(IconData icon, String label, Color color, VoidCallback onTap) {
    return Padding(
      padding: const EdgeInsets.only(right: 4),
      child: Material(
        color: Colors.transparent,
        borderRadius: BorderRadius.circular(AppRadius.sm),
        child: InkWell(
          borderRadius: BorderRadius.circular(AppRadius.sm),
          hoverColor: AppColors.surfaceActive,
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(icon, size: 14, color: color),
                const SizedBox(width: 5),
                Text(
                  label,
                  style: const TextStyle(fontSize: 12, color: AppColors.textSecondary),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _iconButton(IconData icon, String tip, VoidCallback onTap,
      {bool danger = false}) {
    return Tooltip(
      message: tip,
      child: InkWell(
        borderRadius: BorderRadius.circular(AppRadius.sm),
        hoverColor: AppColors.surfaceActive,
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.all(6),
          child: Icon(
            icon,
            size: 15,
            color: danger ? AppColors.danger : AppColors.textMuted,
          ),
        ),
      ),
    );
  }
}
