import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../shared/theme/app_colors.dart';
import '../../shared/theme/app_dimens.dart';
import '../../src/rust/api/mcp.dart' as rust;
import 'mcp_controller.dart';

/// MCP 面板:服务状态卡(开关 / 端点 / 起停)+ 可用工具清单。
///
/// 对照设计稿 `docs/gui/design/rshell-ui-04-mcp.png`。数据由 [McpController] 驱动。
class McpPanel extends StatelessWidget {
  final McpController controller;
  const McpPanel({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) => ListView(
        padding: const EdgeInsets.all(AppSpacing.s6),
        children: [
          _title(),
          const SizedBox(height: AppSpacing.s5),
          _statusCard(context),
          const SizedBox(height: AppSpacing.s6),
          _toolsHeader(),
          const SizedBox(height: AppSpacing.s3),
          _toolsCard(),
        ],
      ),
    );
  }

  // ── 标题 ────────────────────────────────────────────────────
  Widget _title() {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.end,
      children: [
        const Text(
          'MCP 服务器',
          style: TextStyle(
            fontSize: 20,
            fontWeight: FontWeight.w700,
            color: AppColors.textPrimary,
          ),
        ),
        const SizedBox(width: AppSpacing.s2),
        Padding(
          padding: const EdgeInsets.only(bottom: 2),
          child: Text(
            '(Model Context Protocol)',
            style: TextStyle(
              fontSize: 13,
              color: AppColors.textMuted,
            ),
          ),
        ),
      ],
    );
  }

  // ── 状态卡 ──────────────────────────────────────────────────
  Widget _statusCard(BuildContext context) {
    final running = controller.running;
    return Container(
      padding: const EdgeInsets.all(AppSpacing.s5),
      decoration: BoxDecoration(
        color: AppColors.surface2,
        borderRadius: BorderRadius.circular(AppRadius.xl),
        border: Border.all(color: AppColors.borderDefault),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              _PowerToggle(
                on: running,
                busy: controller.busy,
                onTap: controller.toggle,
              ),
              const SizedBox(width: AppSpacing.s4),
              _runState(running),
              const Spacer(),
              _stat('监听地址', controller.host),
              const SizedBox(width: AppSpacing.s6),
              _stat('端口', '${controller.port}'),
              const SizedBox(width: AppSpacing.s6),
              _stat('运行时长', running ? _formatUptime(controller.uptimeSecs) : '—'),
              const SizedBox(width: AppSpacing.s6),
              _actions(running),
            ],
          ),
          if (controller.error != null) ...[
            const SizedBox(height: AppSpacing.s3),
            _errorLine(controller.error!),
          ],
          const SizedBox(height: AppSpacing.s4),
          const Divider(height: 1, color: AppColors.borderSubtle),
          const SizedBox(height: AppSpacing.s4),
          _endpointRow(context),
        ],
      ),
    );
  }

  Widget _runState(bool running) {
    final color = running ? AppColors.online : AppColors.textMuted;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 8,
          height: 8,
          decoration: BoxDecoration(color: color, shape: BoxShape.circle),
        ),
        const SizedBox(width: 7),
        Text(
          running ? '运行中' : '已停止',
          style: TextStyle(
            fontSize: 15,
            fontWeight: FontWeight.w600,
            color: AppColors.textPrimary,
          ),
        ),
      ],
    );
  }

  Widget _stat(String label, String value) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          label,
          style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
        ),
        const SizedBox(height: 4),
        Text(
          value,
          style: const TextStyle(
            fontSize: 14,
            fontWeight: FontWeight.w700,
            color: AppColors.textPrimary,
            fontFamily: AppFonts.mono,
            fontFeatures: [FontFeature.tabularFigures()],
          ),
        ),
      ],
    );
  }

  Widget _actions(bool running) {
    if (!running) {
      return _ActionButton(
        icon: Icons.play_arrow_rounded,
        label: '启动',
        color: AppColors.accent,
        textColor: AppColors.textInverse,
        onTap: controller.busy ? null : controller.start,
      );
    }
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        _ActionButton(
          icon: Icons.stop_rounded,
          label: '停止',
          color: AppColors.danger,
          textColor: AppColors.textPrimary,
          onTap: controller.busy ? null : controller.stop,
        ),
        const SizedBox(width: AppSpacing.s2),
        _ActionButton(
          icon: Icons.refresh_rounded,
          label: '重启',
          color: AppColors.surface3,
          textColor: AppColors.textSecondary,
          onTap: controller.busy ? null : controller.restart,
        ),
      ],
    );
  }

  Widget _errorLine(String error) {
    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.s3,
        vertical: AppSpacing.s2,
      ),
      decoration: BoxDecoration(
        color: AppColors.danger.withValues(alpha: 0.10),
        borderRadius: BorderRadius.circular(AppRadius.md),
        border: Border.all(color: AppColors.danger.withValues(alpha: 0.35)),
      ),
      child: Row(
        children: [
          const Icon(Icons.error_outline, size: 14, color: AppColors.danger),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              error,
              style: const TextStyle(fontSize: 12, color: AppColors.textSecondary),
            ),
          ),
        ],
      ),
    );
  }

  Widget _endpointRow(BuildContext context) {
    return Row(
      children: [
        const Text(
          'MCP 端点',
          style: TextStyle(fontSize: 13, color: AppColors.textSecondary),
        ),
        const SizedBox(width: AppSpacing.s4),
        Expanded(
          child: Container(
            padding: const EdgeInsets.symmetric(
              horizontal: AppSpacing.s3,
              vertical: 9,
            ),
            decoration: BoxDecoration(
              color: AppColors.canvas,
              borderRadius: BorderRadius.circular(AppRadius.md),
              border: Border.all(color: AppColors.borderSubtle),
            ),
            child: Text(
              controller.endpoint,
              style: const TextStyle(
                fontSize: 13,
                color: AppColors.accent,
                fontFamily: AppFonts.mono,
              ),
            ),
          ),
        ),
        const SizedBox(width: AppSpacing.s2),
        _iconButton(
          icon: Icons.copy_rounded,
          tooltip: '复制端点',
          onTap: () => _copyEndpoint(context),
        ),
      ],
    );
  }

  Future<void> _copyEndpoint(BuildContext context) async {
    await Clipboard.setData(ClipboardData(text: controller.endpoint));
    if (!context.mounted) return;
    ScaffoldMessenger.of(context)
      ..clearSnackBars()
      ..showSnackBar(
        const SnackBar(
          content: Text('已复制 MCP 端点'),
          behavior: SnackBarBehavior.floating,
          width: 280,
          backgroundColor: AppColors.surface3,
        ),
      );
  }

  Widget _iconButton({
    required IconData icon,
    required String tooltip,
    required VoidCallback onTap,
  }) {
    return Tooltip(
      message: tooltip,
      child: Material(
        color: AppColors.surface3,
        borderRadius: BorderRadius.circular(AppRadius.md),
        child: InkWell(
          borderRadius: BorderRadius.circular(AppRadius.md),
          hoverColor: AppColors.surfaceHover,
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.all(9),
            child: Icon(icon, size: 16, color: AppColors.textSecondary),
          ),
        ),
      ),
    );
  }

  // ── 可用工具 ────────────────────────────────────────────────
  Widget _toolsHeader() {
    return Row(
      children: [
        const Text(
          '可用工具',
          style: TextStyle(
            fontSize: 16,
            fontWeight: FontWeight.w700,
            color: AppColors.textPrimary,
          ),
        ),
        const SizedBox(width: AppSpacing.s2),
        Text(
          '(Available Tools)',
          style: TextStyle(fontSize: 12, color: AppColors.textMuted),
        ),
        const Spacer(),
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 3),
          decoration: BoxDecoration(
            color: AppColors.accent.withValues(alpha: 0.13),
            borderRadius: BorderRadius.circular(99),
          ),
          child: Text(
            '${controller.tools.length} 个工具',
            style: const TextStyle(
              fontSize: 11,
              color: AppColors.accent,
              fontWeight: FontWeight.w600,
            ),
          ),
        ),
      ],
    );
  }

  Widget _toolsCard() {
    final tools = controller.tools;
    return Container(
      decoration: BoxDecoration(
        color: AppColors.surface2,
        borderRadius: BorderRadius.circular(AppRadius.xl),
        border: Border.all(color: AppColors.borderDefault),
      ),
      child: Column(
        children: [
          for (var i = 0; i < tools.length; i++) ...[
            if (i > 0) const Divider(height: 1, color: AppColors.borderSubtle),
            _toolRow(tools[i]),
          ],
        ],
      ),
    );
  }

  Widget _toolRow(rust.McpToolDto tool) {
    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.s4,
        vertical: AppSpacing.s3,
      ),
      child: Row(
        children: [
          Container(
            width: 30,
            height: 30,
            decoration: BoxDecoration(
              color: AppColors.surface3,
              borderRadius: BorderRadius.circular(AppRadius.md),
            ),
            child: Icon(
              _categoryIcon(tool.category),
              size: 16,
              color: AppColors.accent,
            ),
          ),
          const SizedBox(width: AppSpacing.s3),
          SizedBox(
            width: 240,
            child: Text(
              tool.name,
              style: const TextStyle(
                fontSize: 13,
                fontWeight: FontWeight.w600,
                color: AppColors.textPrimary,
                fontFamily: AppFonts.mono,
              ),
            ),
          ),
          Container(
            width: 1,
            height: 16,
            color: AppColors.borderSubtle,
            margin: const EdgeInsets.symmetric(horizontal: AppSpacing.s4),
          ),
          Expanded(
            child: Text(
              tool.summary,
              style: const TextStyle(fontSize: 13, color: AppColors.textSecondary),
            ),
          ),
        ],
      ),
    );
  }

  static IconData _categoryIcon(String category) {
    switch (category) {
      case 'session':
        return Icons.cable_rounded;
      case 'exec':
        return Icons.terminal_rounded;
      case 'file':
        return Icons.description_outlined;
      case 'dir':
        return Icons.folder_outlined;
      case 'connection':
        return Icons.dns_outlined;
      default:
        return Icons.extension_outlined;
    }
  }

  static String _formatUptime(int secs) {
    if (secs <= 0) return '0s';
    final h = secs ~/ 3600;
    final m = (secs % 3600) ~/ 60;
    final s = secs % 60;
    if (h > 0) return '${h}h ${m}m';
    if (m > 0) return '${m}m ${s}s';
    return '${s}s';
  }
}

/// 大号 ON/OFF 电源开关(开=teal 渐变,关=灰);busy 时显示进度圈。
class _PowerToggle extends StatelessWidget {
  final bool on;
  final bool busy;
  final VoidCallback onTap;

  const _PowerToggle({required this.on, required this.busy, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return Semantics(
      button: true,
      toggled: on,
      label: 'MCP 服务开关',
      child: GestureDetector(
        onTap: busy ? null : onTap,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 180),
          curve: Curves.easeOut,
          width: 68,
          height: 34,
          padding: const EdgeInsets.all(3),
          decoration: BoxDecoration(
            gradient: on ? AppColors.accentGradient : null,
            color: on ? null : AppColors.surfaceActive,
            borderRadius: BorderRadius.circular(99),
            boxShadow: on
                ? [
                    BoxShadow(
                      color: AppColors.accent.withValues(alpha: 0.45),
                      blurRadius: 12,
                      spreadRadius: -2,
                    ),
                  ]
                : null,
          ),
          child: Stack(
            children: [
              AnimatedAlign(
                duration: const Duration(milliseconds: 180),
                curve: Curves.easeOut,
                alignment: on ? Alignment.centerRight : Alignment.centerLeft,
                child: Container(
                  width: 28,
                  height: 28,
                  decoration: const BoxDecoration(
                    color: Colors.white,
                    shape: BoxShape.circle,
                  ),
                  child: busy
                      ? const Padding(
                          padding: EdgeInsets.all(7),
                          child: CircularProgressIndicator(
                            strokeWidth: 2,
                            color: AppColors.accentPressed,
                          ),
                        )
                      : Icon(
                          on ? Icons.power_settings_new_rounded : Icons.power_off_rounded,
                          size: 16,
                          color: on ? AppColors.accentPressed : AppColors.textMuted,
                        ),
                ),
              ),
              Align(
                alignment: on ? Alignment.centerLeft : Alignment.centerRight,
                child: Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 8),
                  child: Text(
                    on ? 'ON' : 'OFF',
                    style: TextStyle(
                      fontSize: 10,
                      fontWeight: FontWeight.w800,
                      color: on ? AppColors.textInverse : AppColors.textMuted,
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// 状态卡右侧的动作按钮(启动 / 停止 / 重启)。
class _ActionButton extends StatelessWidget {
  final IconData icon;
  final String label;
  final Color color;
  final Color textColor;
  final VoidCallback? onTap;

  const _ActionButton({
    required this.icon,
    required this.label,
    required this.color,
    required this.textColor,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final enabled = onTap != null;
    return Opacity(
      opacity: enabled ? 1 : 0.5,
      child: Material(
        color: color,
        borderRadius: BorderRadius.circular(AppRadius.md),
        child: InkWell(
          borderRadius: BorderRadius.circular(AppRadius.md),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 9),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(icon, size: 16, color: textColor),
                const SizedBox(width: 5),
                Text(
                  label,
                  style: TextStyle(
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    color: textColor,
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
