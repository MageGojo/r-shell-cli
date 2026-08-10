import 'package:flutter/material.dart';

import '../../shared/theme/app_colors.dart';
import '../../shared/theme/app_dimens.dart';
import '../../shared/widgets/status_dot.dart';
import '../../src/rust/api/connections.dart';

/// 左侧连接树：展示传入的连接（脱敏），按分组折叠，支持搜索与逐项编辑 / 删除。
///
/// 数据与增删改由父级（[AppScaffold]）统一持有，本组件保持「展示 + 交互回调」。
class ConnectionTree extends StatefulWidget {
  final List<ConnectionDto> connections;
  final String? selectedId;
  final String? loadError;
  final ValueChanged<ConnectionDto>? onSelect;
  final ValueChanged<ConnectionDto>? onOpen;
  final ValueChanged<ConnectionDto>? onOpenFiles;
  final ValueChanged<ConnectionDto>? onEdit;
  final ValueChanged<ConnectionDto>? onDelete;

  const ConnectionTree({
    super.key,
    required this.connections,
    this.selectedId,
    this.loadError,
    this.onSelect,
    this.onOpen,
    this.onOpenFiles,
    this.onEdit,
    this.onDelete,
  });

  @override
  State<ConnectionTree> createState() => _ConnectionTreeState();
}

class _ConnectionTreeState extends State<ConnectionTree> {
  String _query = '';
  final Set<String> _collapsed = {};

  List<ConnectionDto> get _filtered {
    final q = _query.trim().toLowerCase();
    if (q.isEmpty) return widget.connections;
    return widget.connections
        .where(
          (c) =>
              c.name.toLowerCase().contains(q) ||
              c.host.toLowerCase().contains(q) ||
              c.username.toLowerCase().contains(q),
        )
        .toList();
  }

  Map<String, List<ConnectionDto>> get _grouped {
    final map = <String, List<ConnectionDto>>{};
    for (final c in _filtered) {
      (map[c.folder.isEmpty ? '未分组' : c.folder] ??= <ConnectionDto>[]).add(c);
    }
    return map;
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        _searchBar(),
        Expanded(child: _body()),
      ],
    );
  }

  Widget _searchBar() {
    return Padding(
      padding: const EdgeInsets.fromLTRB(
        AppSpacing.s3,
        AppSpacing.s2,
        AppSpacing.s3,
        AppSpacing.s2,
      ),
      child: SizedBox(
        height: 34,
        child: TextField(
          onChanged: (v) => setState(() => _query = v),
          style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
          cursorColor: AppColors.accent,
          decoration: InputDecoration(
            hintText: '搜索连接 / 主机',
            hintStyle: const TextStyle(fontSize: 13, color: AppColors.textMuted),
            prefixIcon: const Icon(
              Icons.search,
              size: 16,
              color: AppColors.textMuted,
            ),
            prefixIconConstraints: const BoxConstraints(
              minWidth: 34,
              minHeight: 34,
            ),
            isDense: true,
            contentPadding: const EdgeInsets.symmetric(vertical: 8),
            filled: true,
            fillColor: AppColors.surface3,
            enabledBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(AppRadius.md),
              borderSide: const BorderSide(color: AppColors.borderDefault),
            ),
            focusedBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(AppRadius.md),
              borderSide: const BorderSide(color: AppColors.accent),
            ),
          ),
        ),
      ),
    );
  }

  Widget _body() {
    if (widget.loadError != null) {
      return _hint(
        '读取连接失败\n${widget.loadError}',
        Icons.error_outline,
        AppColors.danger,
      );
    }
    if (widget.connections.isEmpty) {
      return _hint(
        '暂无连接\n点击下方「添加连接」新建',
        Icons.dns_outlined,
        AppColors.textMuted,
      );
    }
    final groups = _grouped;
    if (groups.isEmpty) {
      return _hint('没有匹配「$_query」的连接', Icons.search_off, AppColors.textMuted);
    }

    final children = <Widget>[];
    groups.forEach((folder, items) {
      final collapsed = _collapsed.contains(folder);
      children.add(_groupHeader(folder, items.length, collapsed));
      if (!collapsed) children.addAll(items.map(_tile));
    });

    return ListView(
      padding: const EdgeInsets.symmetric(vertical: AppSpacing.s2),
      children: children,
    );
  }

  Widget _groupHeader(String folder, int count, bool collapsed) {
    return InkWell(
      hoverColor: AppColors.surfaceHover,
      onTap: () => setState(() {
        if (collapsed) {
          _collapsed.remove(folder);
        } else {
          _collapsed.add(folder);
        }
      }),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(
          AppSpacing.s3,
          AppSpacing.s3,
          AppSpacing.s3,
          AppSpacing.s1,
        ),
        child: Row(
          children: [
            Icon(
              collapsed ? Icons.chevron_right : Icons.expand_more,
              size: 16,
              color: AppColors.textMuted,
            ),
            const SizedBox(width: 2),
            const Icon(
              Icons.folder_outlined,
              size: 14,
              color: AppColors.textSecondary,
            ),
            const SizedBox(width: 6),
            Expanded(
              child: Text(
                folder,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                  fontSize: 12,
                  color: AppColors.textSecondary,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
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
        ),
      ),
    );
  }

  Widget _tile(ConnectionDto c) {
    final selected = c.id == widget.selectedId;
    final status = connStatusFrom(c.status);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.s2, vertical: 1),
      child: Material(
        color: selected ? AppColors.surfaceActive : Colors.transparent,
        borderRadius: BorderRadius.circular(AppRadius.md),
        child: InkWell(
          borderRadius: BorderRadius.circular(AppRadius.md),
          hoverColor: AppColors.surfaceHover,
          onTap: () => widget.onSelect?.call(c),
          onDoubleTap: () => widget.onOpen?.call(c),
          child: Container(
            height: 44,
            padding: const EdgeInsets.only(left: AppSpacing.s2, right: 2),
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(AppRadius.md),
              border: Border(
                left: BorderSide(
                  color: selected ? AppColors.accent : Colors.transparent,
                  width: 2,
                ),
              ),
            ),
            child: Row(
              children: [
                const SizedBox(width: 6),
                StatusDot(status),
                const SizedBox(width: AppSpacing.s3),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Row(
                        children: [
                          Flexible(
                            child: Text(
                              c.name,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(
                                fontSize: 13,
                                color: AppColors.textPrimary,
                                fontWeight: FontWeight.w500,
                              ),
                            ),
                          ),
                          _authBadge(c),
                        ],
                      ),
                      const SizedBox(height: 1),
                      Text(
                        c.protocol == 'ADB'
                            ? 'adb · ${c.host}:${c.port}'
                            : _isIos(c)
                                ? 'ios · ${c.username}@${c.host}:${c.port}'
                                : '${c.username}@${c.host}:${c.port}',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                          fontSize: 11,
                          color: AppColors.textMuted,
                          fontFamily: AppFonts.mono,
                        ),
                      ),
                    ],
                  ),
                ),
                _actionsMenu(c),
              ],
            ),
          ),
        ),
      ),
    );
  }

  bool _isIos(ConnectionDto c) => c.tags.any((t) {
        final lower = t.trim().toLowerCase();
        return lower == 'platform:ios' || lower == 'ios';
      });

  Widget _authBadge(ConnectionDto c) {
    if (c.protocol == 'ADB') {
      return const Padding(
        padding: EdgeInsets.only(left: 6),
        child: Tooltip(
          message: 'ADB（安卓）',
          child: Icon(Icons.android, size: 12, color: AppColors.online),
        ),
      );
    }
    if (_isIos(c)) {
      return const Padding(
        padding: EdgeInsets.only(left: 6),
        child: Tooltip(
          message: 'iOS 越狱 SSH',
          child: Icon(Icons.phone_iphone, size: 12, color: AppColors.accent),
        ),
      );
    }
    final isKey = c.authMethod == 'publickey';
    return Padding(
      padding: const EdgeInsets.only(left: 6),
      child: Tooltip(
        message: isKey ? '公钥认证' : '密码认证',
        child: Icon(
          isKey ? Icons.vpn_key_outlined : Icons.password,
          size: 11,
          color: AppColors.textMuted,
        ),
      ),
    );
  }

  Widget _actionsMenu(ConnectionDto c) {
    return PopupMenuButton<String>(
      tooltip: '更多操作',
      padding: EdgeInsets.zero,
      splashRadius: 16,
      position: PopupMenuPosition.under,
      color: AppColors.surface3,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(AppRadius.md),
        side: const BorderSide(color: AppColors.borderDefault),
      ),
      icon: const Icon(
        Icons.more_horiz,
        size: 16,
        color: AppColors.textMuted,
      ),
      onSelected: (value) {
        switch (value) {
          case 'open':
            widget.onOpen?.call(c);
          case 'files':
            widget.onOpenFiles?.call(c);
          case 'edit':
            widget.onEdit?.call(c);
          case 'delete':
            widget.onDelete?.call(c);
        }
      },
      itemBuilder: (_) => const [
        PopupMenuItem(
          value: 'open',
          height: 38,
          child: Row(
            children: [
              Icon(Icons.terminal, size: 15, color: AppColors.accent),
              SizedBox(width: 10),
              Text(
                '打开终端',
                style: TextStyle(fontSize: 13, color: AppColors.textPrimary),
              ),
            ],
          ),
        ),
        PopupMenuItem(
          value: 'files',
          height: 38,
          child: Row(
            children: [
              Icon(Icons.folder_outlined, size: 15, color: AppColors.accentIndigo),
              SizedBox(width: 10),
              Text(
                '打开文件',
                style: TextStyle(fontSize: 13, color: AppColors.textPrimary),
              ),
            ],
          ),
        ),
        PopupMenuItem(
          value: 'edit',
          height: 38,
          child: Row(
            children: [
              Icon(Icons.edit_outlined, size: 15, color: AppColors.textSecondary),
              SizedBox(width: 10),
              Text(
                '编辑',
                style: TextStyle(fontSize: 13, color: AppColors.textPrimary),
              ),
            ],
          ),
        ),
        PopupMenuItem(
          value: 'delete',
          height: 38,
          child: Row(
            children: [
              Icon(Icons.delete_outline, size: 15, color: AppColors.danger),
              SizedBox(width: 10),
              Text(
                '删除',
                style: TextStyle(fontSize: 13, color: AppColors.danger),
              ),
            ],
          ),
        ),
      ],
    );
  }

  Widget _hint(String text, IconData icon, Color color) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(AppSpacing.s6),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 30, color: color.withValues(alpha: 0.6)),
            const SizedBox(height: AppSpacing.s3),
            Text(
              text,
              textAlign: TextAlign.center,
              style: const TextStyle(fontSize: 12, color: AppColors.textMuted),
            ),
          ],
        ),
      ),
    );
  }
}
