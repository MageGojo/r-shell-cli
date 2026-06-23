import 'dart:convert';
import 'dart:typed_data';

import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/material.dart';

import '../../shared/theme/app_colors.dart';
import '../../shared/theme/app_dimens.dart';
import '../../src/rust/api/sftp.dart' as rust;
import 'files_controller.dart';
import 'local_fs.dart';
import 'transfer_queue.dart';

/// SFTP 文件视图：左远程 / 右本地双栏 + 中间传输按钮 + 底部传输队列。
class FilesPanel extends StatelessWidget {
  final FilesController controller;
  const FilesPanel({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        if (controller.connection == null) {
          return const _EmptyState();
        }
        return Column(
          children: [
            Expanded(
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Expanded(child: _RemotePane(controller: controller)),
                  _TransferButtons(controller: controller),
                  Expanded(child: _LocalPane(controller: controller)),
                ],
              ),
            ),
            _TransferQueueBar(queue: controller.queue),
          ],
        );
      },
    );
  }
}

// ── 远程一侧（带拖拽上传） ───────────────────────────────────────
class _RemotePane extends StatefulWidget {
  final FilesController controller;
  const _RemotePane({required this.controller});

  @override
  State<_RemotePane> createState() => _RemotePaneState();
}

class _RemotePaneState extends State<_RemotePane> {
  bool _dragging = false;

  @override
  Widget build(BuildContext context) {
    final c = widget.controller;
    return DropTarget(
      onDragEntered: (_) => setState(() => _dragging = true),
      onDragExited: (_) => setState(() => _dragging = false),
      onDragDone: (detail) {
        setState(() => _dragging = false);
        for (final file in detail.files) {
          c.uploadFile(localFilePath: file.path, name: file.name);
        }
      },
      // 接受「本地一侧」拖进来的条目 → 上传到当前远程目录（本地 → 远程）。
      child: DragTarget<LocalEntry>(
        onAcceptWithDetails: (d) => c.uploadLocalEntry(d.data),
        builder: (context, candidate, rejected) {
          final active = _dragging || candidate.isNotEmpty;
          return Container(
            decoration: BoxDecoration(
              color: active ? AppColors.surfaceHover : null,
              border: Border.all(
                color: active ? AppColors.accent : Colors.transparent,
                width: 1.5,
              ),
            ),
            child: Column(
              children: [
                _PaneHeader(
                  icon: Icons.dns_outlined,
                  title: '远程',
                  subtitle: c.connection?.name ?? '',
                  path: c.remotePath,
                  onUp: c.remoteUp,
                  onRefresh: c.refreshRemote,
                ),
                Expanded(
                  child: _RemoteList(controller: c, dragging: active),
                ),
              ],
            ),
          );
        },
      ),
    );
  }
}

class _RemoteList extends StatelessWidget {
  final FilesController controller;
  final bool dragging;
  const _RemoteList({required this.controller, required this.dragging});

  @override
  Widget build(BuildContext context) {
    final c = controller;
    if (c.remoteLoading && c.remoteEntries.isEmpty) {
      return const _Loading();
    }
    if (c.remoteError != null) {
      return _ErrorHint(message: c.remoteError!, onRetry: c.refreshRemote);
    }
    if (c.remoteEntries.isEmpty) {
      return _Hint(
        dragging ? '松手即上传到此目录' : '空目录 · 可将文件拖入此处上传',
        Icons.cloud_upload_outlined,
      );
    }
    return ListView.builder(
      padding: const EdgeInsets.symmetric(vertical: AppSpacing.s1),
      itemCount: c.remoteEntries.length,
      itemBuilder: (context, i) {
        final e = c.remoteEntries[i];
        final isDir = e.kind == 'dir';
        final row = _FileRow(
          icon: _remoteIcon(e.kind),
          name: e.name,
          size: isDir ? '' : _humanSize(e.size.toInt()),
          meta: _fmtUnix(e.modifiedUnix.toInt()),
          isDir: isDir,
          selected: c.selectedRemote == e.name,
          onTap: () => c.selectRemote(e.name),
          onDoubleTap: () => c.enterRemote(e),
        );
        return GestureDetector(
          behavior: HitTestBehavior.translucent,
          onSecondaryTapDown: (d) =>
              _showRemoteMenu(context, d.globalPosition, c, e),
          // 非目录可拖到「本地」一侧下载；目录暂不支持拖拽传输。
          child: isDir
              ? row
              : Draggable<rust.SftpEntryDto>(
                  data: e,
                  dragAnchorStrategy: pointerDragAnchorStrategy,
                  feedback: _DragFeedback(name: e.name, icon: _remoteIcon(e.kind)),
                  childWhenDragging: Opacity(opacity: 0.4, child: row),
                  child: row,
                ),
        );
      },
    );
  }
}

// ── 本地一侧 ───────────────────────────────────────────────────
class _LocalPane extends StatelessWidget {
  final FilesController controller;
  const _LocalPane({required this.controller});

  @override
  Widget build(BuildContext context) {
    final c = controller;
    // 接受「远程一侧」拖进来的条目 → 下载到当前本地目录（远程 → 本地）。
    return DragTarget<rust.SftpEntryDto>(
      onAcceptWithDetails: (d) => c.downloadRemoteEntry(d.data),
      builder: (context, candidate, rejected) {
        final active = candidate.isNotEmpty;
        return Container(
          decoration: BoxDecoration(
            color: active ? AppColors.surfaceHover : null,
            border: Border(
              left: BorderSide(
                color: active ? AppColors.accent : AppColors.borderSubtle,
                width: active ? 1.5 : 1,
              ),
            ),
          ),
          child: Column(
            children: [
              _PaneHeader(
                icon: Icons.computer_outlined,
                title: '本地',
                subtitle: '此电脑',
                path: c.localPath,
                onUp: c.localUp,
                onRefresh: c.refreshLocal,
              ),
              Expanded(child: _LocalList(controller: c)),
            ],
          ),
        );
      },
    );
  }
}

class _LocalList extends StatelessWidget {
  final FilesController controller;
  const _LocalList({required this.controller});

  @override
  Widget build(BuildContext context) {
    final c = controller;
    if (c.localLoading && c.localEntries.isEmpty) {
      return const _Loading();
    }
    if (c.localError != null) {
      return _ErrorHint(message: c.localError!, onRetry: c.refreshLocal);
    }
    if (c.localEntries.isEmpty) {
      return const _Hint('空目录', Icons.folder_open_outlined);
    }
    return ListView.builder(
      padding: const EdgeInsets.symmetric(vertical: AppSpacing.s1),
      itemCount: c.localEntries.length,
      itemBuilder: (context, i) {
        final LocalEntry e = c.localEntries[i];
        final row = _FileRow(
          icon: e.isDir ? Icons.folder_rounded : Icons.insert_drive_file_outlined,
          name: e.name,
          size: e.isDir ? '' : _humanSize(e.size),
          meta: e.modified == null ? '—' : _fmtDate(e.modified!),
          isDir: e.isDir,
          selected: c.selectedLocal == e.path,
          onTap: () => c.selectLocal(e.path),
          onDoubleTap: () => c.enterLocal(e),
        );
        return GestureDetector(
          behavior: HitTestBehavior.translucent,
          onSecondaryTapDown: (d) =>
              _showLocalMenu(context, d.globalPosition, c, e),
          child: e.isDir
              ? row
              : Draggable<LocalEntry>(
                  data: e,
                  dragAnchorStrategy: pointerDragAnchorStrategy,
                  feedback: _DragFeedback(
                    name: e.name,
                    icon: Icons.insert_drive_file_outlined,
                  ),
                  childWhenDragging: Opacity(opacity: 0.4, child: row),
                  child: row,
                ),
        );
      },
    );
  }
}

// ── 通用栏头 ───────────────────────────────────────────────────
class _PaneHeader extends StatelessWidget {
  final IconData icon;
  final String title;
  final String subtitle;
  final String path;
  final VoidCallback onUp;
  final VoidCallback onRefresh;

  const _PaneHeader({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.path,
    required this.onUp,
    required this.onRefresh,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      height: AppLayout.toolbarHeight,
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.s3),
      decoration: const BoxDecoration(
        border: Border(bottom: BorderSide(color: AppColors.borderSubtle)),
      ),
      child: Row(
        children: [
          Icon(icon, size: 15, color: AppColors.accent),
          const SizedBox(width: 6),
          Text(
            title,
            style: const TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w600,
              color: AppColors.textPrimary,
            ),
          ),
          if (subtitle.isNotEmpty) ...[
            const SizedBox(width: 6),
            Flexible(
              child: Text(
                subtitle,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
              ),
            ),
          ],
          const Spacer(),
          _HeaderButton(icon: Icons.arrow_upward, tooltip: '上级目录', onTap: onUp),
          _HeaderButton(icon: Icons.refresh, tooltip: '刷新', onTap: onRefresh),
        ],
      ),
    );
  }
}

class _HeaderButton extends StatelessWidget {
  final IconData icon;
  final String tooltip;
  final VoidCallback onTap;
  const _HeaderButton({
    required this.icon,
    required this.tooltip,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      child: InkWell(
        borderRadius: BorderRadius.circular(AppRadius.sm),
        hoverColor: AppColors.surfaceHover,
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.all(5),
          child: Icon(icon, size: 15, color: AppColors.textSecondary),
        ),
      ),
    );
  }
}

// ── 文件行 ─────────────────────────────────────────────────────
class _FileRow extends StatelessWidget {
  final IconData icon;
  final String name;
  final String size;
  final String meta;
  final bool isDir;
  final bool selected;
  final VoidCallback onTap;
  final VoidCallback onDoubleTap;

  const _FileRow({
    required this.icon,
    required this.name,
    required this.size,
    required this.meta,
    required this.isDir,
    required this.selected,
    required this.onTap,
    required this.onDoubleTap,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.s2, vertical: 1),
      child: Material(
        color: selected ? AppColors.surfaceActive : Colors.transparent,
        borderRadius: BorderRadius.circular(AppRadius.sm),
        child: InkWell(
          borderRadius: BorderRadius.circular(AppRadius.sm),
          hoverColor: AppColors.surfaceHover,
          onTap: onTap,
          onDoubleTap: onDoubleTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: AppSpacing.s2,
              vertical: 6,
            ),
            child: Row(
              children: [
                Icon(
                  icon,
                  size: 16,
                  color: isDir ? AppColors.accentIndigo : AppColors.textSecondary,
                ),
                const SizedBox(width: AppSpacing.s2),
                Expanded(
                  child: Text(
                    name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                      fontSize: 12.5,
                      color: AppColors.textPrimary,
                    ),
                  ),
                ),
                const SizedBox(width: AppSpacing.s2),
                SizedBox(
                  width: 64,
                  child: Text(
                    size,
                    textAlign: TextAlign.right,
                    style: const TextStyle(
                      fontSize: 11,
                      color: AppColors.textMuted,
                      fontFamily: AppFonts.mono,
                    ),
                  ),
                ),
                const SizedBox(width: AppSpacing.s3),
                SizedBox(
                  width: 116,
                  child: Text(
                    meta,
                    textAlign: TextAlign.right,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                      fontSize: 11,
                      color: AppColors.textMuted,
                      fontFamily: AppFonts.mono,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

// ── 中间传输按钮 ───────────────────────────────────────────────
class _TransferButtons extends StatelessWidget {
  final FilesController controller;
  const _TransferButtons({required this.controller});

  @override
  Widget build(BuildContext context) {
    final c = controller;
    return Container(
      width: 56,
      color: AppColors.surface1,
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          _ArrowButton(
            icon: Icons.arrow_back_rounded,
            tooltip: '上传到远程（本地 → 远程）',
            enabled: c.canUploadSelection,
            onTap: c.uploadSelected,
          ),
          const SizedBox(height: AppSpacing.s4),
          _ArrowButton(
            icon: Icons.arrow_forward_rounded,
            tooltip: '下载到本地（远程 → 本地）',
            enabled: c.canDownloadSelection,
            onTap: c.downloadSelected,
          ),
        ],
      ),
    );
  }
}

class _ArrowButton extends StatelessWidget {
  final IconData icon;
  final String tooltip;
  final bool enabled;
  final VoidCallback onTap;
  const _ArrowButton({
    required this.icon,
    required this.tooltip,
    required this.enabled,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      child: Material(
        color: enabled ? AppColors.accent : AppColors.surface3,
        borderRadius: BorderRadius.circular(AppRadius.md),
        child: InkWell(
          borderRadius: BorderRadius.circular(AppRadius.md),
          hoverColor: enabled ? AppColors.accentHover : null,
          onTap: enabled ? onTap : null,
          child: Container(
            width: 38,
            height: 34,
            alignment: Alignment.center,
            child: Icon(
              icon,
              size: 19,
              color: enabled ? AppColors.textInverse : AppColors.textMuted,
            ),
          ),
        ),
      ),
    );
  }
}

// ── 传输队列 ───────────────────────────────────────────────────
class _TransferQueueBar extends StatelessWidget {
  final TransferQueue queue;
  const _TransferQueueBar({required this.queue});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: queue,
      builder: (context, _) {
        final items = queue.items;
        return Container(
          decoration: const BoxDecoration(
            color: AppColors.surface1,
            border: Border(top: BorderSide(color: AppColors.borderSubtle)),
          ),
          constraints: const BoxConstraints(maxHeight: 168),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              _header(items.length, queue.activeCount),
              if (items.isEmpty)
                const Padding(
                  padding: EdgeInsets.symmetric(vertical: AppSpacing.s4),
                  child: Text(
                    '暂无传输任务',
                    style: TextStyle(fontSize: 12, color: AppColors.textMuted),
                  ),
                )
              else
                Flexible(
                  child: ListView.builder(
                    padding: const EdgeInsets.only(bottom: AppSpacing.s2),
                    itemCount: items.length,
                    itemBuilder: (_, i) => _TransferRow(item: items[i]),
                  ),
                ),
            ],
          ),
        );
      },
    );
  }

  Widget _header(int total, int active) {
    return Container(
      height: AppLayout.toolbarHeight,
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.s4),
      child: Row(
        children: [
          const Icon(Icons.swap_vert, size: 15, color: AppColors.accent),
          const SizedBox(width: 6),
          const Text(
            '传输队列',
            style: TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w600,
              color: AppColors.textPrimary,
            ),
          ),
          const SizedBox(width: 8),
          if (active > 0)
            Text(
              '$active 进行中',
              style: const TextStyle(fontSize: 11, color: AppColors.accent),
            ),
          const Spacer(),
          if (queue.hasFinished)
            TextButton(
              onPressed: queue.clearFinished,
              style: TextButton.styleFrom(
                foregroundColor: AppColors.textSecondary,
                padding: const EdgeInsets.symmetric(horizontal: 8),
                minimumSize: const Size(0, 28),
                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
              ),
              child: const Text('清除已完成', style: TextStyle(fontSize: 12)),
            ),
        ],
      ),
    );
  }
}

class _TransferRow extends StatelessWidget {
  final TransferItem item;
  const _TransferRow({required this.item});

  @override
  Widget build(BuildContext context) {
    final (icon, color) = switch (item.state) {
      TransferState.completed => (Icons.check_circle, AppColors.online),
      TransferState.failed => (Icons.error_outline, AppColors.danger),
      TransferState.running => (
        item.direction == TransferDirection.upload
            ? Icons.upload_rounded
            : Icons.download_rounded,
        AppColors.accent,
      ),
    };
    return Padding(
      padding: const EdgeInsets.fromLTRB(
        AppSpacing.s4,
        4,
        AppSpacing.s4,
        4,
      ),
      child: Row(
        children: [
          Icon(icon, size: 15, color: color),
          const SizedBox(width: AppSpacing.s2),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        item.name,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                          fontSize: 12,
                          color: AppColors.textPrimary,
                        ),
                      ),
                    ),
                    const SizedBox(width: AppSpacing.s2),
                    Text(
                      _statusText(item),
                      style: TextStyle(
                        fontSize: 10.5,
                        color: item.state == TransferState.failed
                            ? AppColors.danger
                            : AppColors.textMuted,
                        fontFamily: AppFonts.mono,
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 4),
                ClipRRect(
                  borderRadius: BorderRadius.circular(99),
                  child: LinearProgressIndicator(
                    value: item.state == TransferState.running && item.total <= 0
                        ? null
                        : item.progress,
                    minHeight: 4,
                    backgroundColor: AppColors.surfaceActive,
                    color: color,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  String _statusText(TransferItem item) {
    switch (item.state) {
      case TransferState.failed:
        return '失败';
      case TransferState.completed:
        return '完成 · ${_humanSize(item.total)}';
      case TransferState.running:
        if (item.total <= 0) return '准备中…';
        return '${_humanSize(item.transferred)} / ${_humanSize(item.total)}';
    }
  }
}

// ── 状态 / 提示组件 ─────────────────────────────────────────────
class _EmptyState extends StatelessWidget {
  const _EmptyState();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            Icons.folder_shared_outlined,
            size: 46,
            color: AppColors.textMuted.withValues(alpha: 0.5),
          ),
          const SizedBox(height: AppSpacing.s4),
          const Text(
            '在左侧选择一个连接以浏览文件',
            style: TextStyle(fontSize: 15, color: AppColors.textSecondary),
          ),
          const SizedBox(height: AppSpacing.s2),
          const Text(
            '或在连接的「⋯」菜单中选择「打开文件」',
            style: TextStyle(fontSize: 12, color: AppColors.textMuted),
          ),
        ],
      ),
    );
  }
}

class _Loading extends StatelessWidget {
  const _Loading();

  @override
  Widget build(BuildContext context) {
    return const Center(
      child: SizedBox(
        width: 22,
        height: 22,
        child: CircularProgressIndicator(strokeWidth: 2.5, color: AppColors.accent),
      ),
    );
  }
}

class _Hint extends StatelessWidget {
  final String text;
  final IconData icon;
  const _Hint(this.text, this.icon);

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 28, color: AppColors.textMuted.withValues(alpha: 0.6)),
          const SizedBox(height: AppSpacing.s2),
          Text(
            text,
            textAlign: TextAlign.center,
            style: const TextStyle(fontSize: 12, color: AppColors.textMuted),
          ),
        ],
      ),
    );
  }
}

class _ErrorHint extends StatelessWidget {
  final String message;
  final VoidCallback onRetry;
  const _ErrorHint({required this.message, required this.onRetry});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(AppSpacing.s5),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              Icons.error_outline,
              size: 28,
              color: AppColors.danger.withValues(alpha: 0.8),
            ),
            const SizedBox(height: AppSpacing.s3),
            Text(
              message,
              textAlign: TextAlign.center,
              maxLines: 4,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontSize: 12, color: AppColors.textMuted),
            ),
            const SizedBox(height: AppSpacing.s3),
            TextButton(
              onPressed: onRetry,
              style: TextButton.styleFrom(foregroundColor: AppColors.accent),
              child: const Text('重试'),
            ),
          ],
        ),
      ),
    );
  }
}

// ── 格式化 ─────────────────────────────────────────────────────
String _humanSize(int bytes) {
  if (bytes < 1024) return '$bytes B';
  const units = ['KB', 'MB', 'GB', 'TB'];
  var value = bytes / 1024;
  var unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return '${value.toStringAsFixed(value >= 10 ? 0 : 1)} ${units[unit]}';
}

String _fmtUnix(int seconds) {
  if (seconds <= 0) return '—';
  return _fmtDate(DateTime.fromMillisecondsSinceEpoch(seconds * 1000));
}

String _fmtDate(DateTime dt) {
  String two(int n) => n.toString().padLeft(2, '0');
  return '${dt.year}-${two(dt.month)}-${two(dt.day)} ${two(dt.hour)}:${two(dt.minute)}';
}

IconData _remoteIcon(String kind) {
  switch (kind) {
    case 'dir':
      return Icons.folder_rounded;
    case 'symlink':
      return Icons.link_rounded;
    default:
      return Icons.insert_drive_file_outlined;
  }
}

// ── 右键菜单 ───────────────────────────────────────────────────
Future<void> _showRemoteMenu(
  BuildContext context,
  Offset pos,
  FilesController c,
  rust.SftpEntryDto e,
) async {
  final isDir = e.kind == 'dir';
  final selected = await _popupAt(context, pos, [
    if (!isDir) _menuItem('edit', Icons.edit_outlined, '打开编辑'),
    if (!isDir) _menuItem('download', Icons.download_outlined, '下载到本地'),
    if (!isDir) _menuItem('copy', Icons.copy_outlined, '复制到剪贴板'),
    _menuItem('rename', Icons.drive_file_rename_outline, '重命名'),
    _menuItem('delete', Icons.delete_outline, '删除', danger: true),
  ]);
  if (selected == null || !context.mounted) return;
  switch (selected) {
    case 'edit':
      await _openEditor(
        context,
        title: e.name,
        load: () => c.readRemoteBytes(e),
        save: (data) => c.writeRemoteBytes(e.name, data),
      );
    case 'download':
      c.downloadRemoteEntry(e);
      _snack(context, '已加入下载队列：${e.name}');
    case 'copy':
      final err = await c.copyRemoteToClipboard(e);
      if (context.mounted) {
        _snack(context, err ?? '已复制「${e.name}」到剪贴板');
      }
    case 'rename':
      final name = await _promptRename(context, e.name);
      if (name != null && context.mounted) {
        final err = await c.renameRemote(e, name);
        if (err != null && context.mounted) _snack(context, '重命名失败：$err');
      }
    case 'delete':
      final ok = await _confirmDelete(context, e.name, isDir: isDir);
      if (ok && context.mounted) {
        final err = await c.deleteRemote(e);
        if (err != null && context.mounted) _snack(context, '删除失败：$err');
      }
  }
}

Future<void> _showLocalMenu(
  BuildContext context,
  Offset pos,
  FilesController c,
  LocalEntry e,
) async {
  final canUpload = c.connection != null && !e.isDir;
  final selected = await _popupAt(context, pos, [
    if (!e.isDir) _menuItem('edit', Icons.edit_outlined, '打开编辑'),
    if (canUpload) _menuItem('upload', Icons.upload_outlined, '上传到远程'),
    if (!e.isDir) _menuItem('copy', Icons.copy_outlined, '复制到剪贴板'),
    _menuItem('rename', Icons.drive_file_rename_outline, '重命名'),
    _menuItem('delete', Icons.delete_outline, '删除', danger: true),
  ]);
  if (selected == null || !context.mounted) return;
  switch (selected) {
    case 'edit':
      await _openEditor(
        context,
        title: e.name,
        load: () => c.readLocalBytes(e),
        save: (data) => c.writeLocalBytes(e.path, data),
      );
    case 'upload':
      c.uploadLocalEntry(e);
      _snack(context, '已加入上传队列：${e.name}');
    case 'copy':
      final err = await c.copyLocalToClipboard(e);
      if (context.mounted) {
        _snack(context, err ?? '已复制「${e.name}」到剪贴板');
      }
    case 'rename':
      final name = await _promptRename(context, e.name);
      if (name != null && context.mounted) {
        final err = await c.renameLocal(e, name);
        if (err != null && context.mounted) _snack(context, '重命名失败：$err');
      }
    case 'delete':
      final ok = await _confirmDelete(context, e.name, isDir: e.isDir);
      if (ok && context.mounted) {
        final err = await c.deleteLocal(e);
        if (err != null && context.mounted) _snack(context, '删除失败：$err');
      }
  }
}

Future<String?> _popupAt(
  BuildContext context,
  Offset pos,
  List<PopupMenuEntry<String>> items,
) {
  final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
  return showMenu<String>(
    context: context,
    color: AppColors.surface2,
    position: RelativeRect.fromRect(
      Rect.fromPoints(pos, pos),
      Offset.zero & overlay.size,
    ),
    items: items,
  );
}

PopupMenuItem<String> _menuItem(
  String value,
  IconData icon,
  String label, {
  bool danger = false,
}) {
  final color = danger ? AppColors.danger : AppColors.textSecondary;
  return PopupMenuItem<String>(
    value: value,
    height: 38,
    child: Row(
      children: [
        Icon(icon, size: 16, color: color),
        const SizedBox(width: AppSpacing.s2),
        Text(
          label,
          style: TextStyle(
            fontSize: 13,
            color: danger ? AppColors.danger : AppColors.textPrimary,
          ),
        ),
      ],
    ),
  );
}

// ── 对话框 ─────────────────────────────────────────────────────
Future<String?> _promptRename(BuildContext context, String current) {
  final controller = TextEditingController(text: current);
  return showDialog<String>(
    context: context,
    builder: (ctx) => AlertDialog(
      backgroundColor: AppColors.surface2,
      title: const Text('重命名', style: TextStyle(fontSize: 16)),
      content: TextField(
        controller: controller,
        autofocus: true,
        style: const TextStyle(fontSize: 14, color: AppColors.textPrimary),
        decoration: const InputDecoration(hintText: '新名称'),
        onSubmitted: (v) => Navigator.pop(ctx, v),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(ctx),
          child: const Text('取消'),
        ),
        FilledButton(
          onPressed: () => Navigator.pop(ctx, controller.text),
          child: const Text('确定'),
        ),
      ],
    ),
  );
}

Future<bool> _confirmDelete(
  BuildContext context,
  String name, {
  required bool isDir,
}) async {
  final result = await showDialog<bool>(
    context: context,
    builder: (ctx) => AlertDialog(
      backgroundColor: AppColors.surface2,
      title: const Text('确认删除', style: TextStyle(fontSize: 16)),
      content: Text(
        isDir ? '删除目录「$name」及其全部内容？此操作不可撤销。' : '删除文件「$name」？此操作不可撤销。',
        style: const TextStyle(fontSize: 13, color: AppColors.textSecondary),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(ctx, false),
          child: const Text('取消'),
        ),
        FilledButton(
          style: FilledButton.styleFrom(backgroundColor: AppColors.danger),
          onPressed: () => Navigator.pop(ctx, true),
          child: const Text('删除'),
        ),
      ],
    ),
  );
  return result ?? false;
}

/// 打开一个内置文本编辑器：`load` 取字节、`save` 回写字节（返回错误信息或 null）。
/// 二进制内容（含 NUL）拒绝编辑。
Future<void> _openEditor(
  BuildContext context, {
  required String title,
  required Future<Uint8List> Function() load,
  required Future<String?> Function(Uint8List data) save,
}) async {
  Uint8List bytes;
  try {
    bytes = await load();
  } catch (err) {
    if (context.mounted) _snack(context, '打开失败：$err');
    return;
  }
  if (bytes.contains(0)) {
    if (context.mounted) _snack(context, '「$title」疑似二进制文件，暂不支持编辑');
    return;
  }
  String text;
  try {
    text = utf8.decode(bytes);
  } catch (_) {
    if (context.mounted) _snack(context, '「$title」非 UTF-8 文本，暂不支持编辑');
    return;
  }
  if (!context.mounted) return;
  final edited = await showDialog<String>(
    context: context,
    builder: (_) => _EditorDialog(title: title, initial: text),
  );
  if (edited == null || !context.mounted) return;
  final err = await save(Uint8List.fromList(utf8.encode(edited)));
  if (context.mounted) {
    _snack(context, err == null ? '已保存「$title」' : '保存失败：$err');
  }
}

void _snack(BuildContext context, String message) {
  ScaffoldMessenger.maybeOf(context)
    ?..clearSnackBars()
    ..showSnackBar(
      SnackBar(
        content: Text(message),
        behavior: SnackBarBehavior.floating,
        width: 360,
        backgroundColor: AppColors.surface3,
      ),
    );
}

// ── 拖拽反馈 + 编辑器 ──────────────────────────────────────────
class _DragFeedback extends StatelessWidget {
  final String name;
  final IconData icon;
  const _DragFeedback({required this.name, required this.icon});

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
        decoration: BoxDecoration(
          color: AppColors.accent.withValues(alpha: 0.92),
          borderRadius: BorderRadius.circular(AppRadius.md),
          boxShadow: [
            BoxShadow(
              color: Colors.black.withValues(alpha: 0.3),
              blurRadius: 12,
              offset: const Offset(0, 4),
            ),
          ],
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 14, color: AppColors.textInverse),
            const SizedBox(width: 6),
            Text(
              name,
              style: const TextStyle(
                fontSize: 12,
                color: AppColors.textInverse,
                fontWeight: FontWeight.w600,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _EditorDialog extends StatefulWidget {
  final String title;
  final String initial;
  const _EditorDialog({required this.title, required this.initial});

  @override
  State<_EditorDialog> createState() => _EditorDialogState();
}

class _EditorDialogState extends State<_EditorDialog> {
  late final TextEditingController _controller =
      TextEditingController(text: widget.initial);

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: AppColors.surface2,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 760, maxHeight: 560),
        child: Padding(
          padding: const EdgeInsets.all(AppSpacing.s4),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  const Icon(Icons.edit_outlined, size: 16, color: AppColors.accent),
                  const SizedBox(width: 6),
                  Expanded(
                    child: Text(
                      widget.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w600,
                        color: AppColors.textPrimary,
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: AppSpacing.s3),
              Expanded(
                child: Container(
                  decoration: BoxDecoration(
                    color: AppColors.surface1,
                    borderRadius: BorderRadius.circular(AppRadius.md),
                    border: Border.all(color: AppColors.borderSubtle),
                  ),
                  child: TextField(
                    controller: _controller,
                    autofocus: true,
                    expands: true,
                    maxLines: null,
                    minLines: null,
                    textAlignVertical: TextAlignVertical.top,
                    style: const TextStyle(
                      fontSize: 12.5,
                      height: 1.4,
                      color: AppColors.textPrimary,
                      fontFamily: AppFonts.mono,
                    ),
                    decoration: const InputDecoration(
                      border: InputBorder.none,
                      contentPadding: EdgeInsets.all(AppSpacing.s3),
                    ),
                  ),
                ),
              ),
              const SizedBox(height: AppSpacing.s3),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  TextButton(
                    onPressed: () => Navigator.pop(context),
                    child: const Text('取消'),
                  ),
                  const SizedBox(width: AppSpacing.s2),
                  FilledButton.icon(
                    onPressed: () => Navigator.pop(context, _controller.text),
                    icon: const Icon(Icons.save_outlined, size: 16),
                    label: const Text('保存'),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
