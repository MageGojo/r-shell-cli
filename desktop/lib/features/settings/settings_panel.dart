import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../shared/theme/app_colors.dart';
import '../../shared/theme/app_dimens.dart';
import 'settings_controller.dart';

/// 应用版本(与状态栏一致;后续可由构建注入)。
const String kAppVersion = 'v2.1.0';
const String kRepoUrl = 'https://github.com/MageGojo/conch';

/// 设置页:外观 / 终端 / 监控 / MCP / 关于。
///
/// 本阶段以「可视 + 轻量本地状态」呈现(由 [SettingsController] 承接);持久化与实际
/// 生效留到功能阶段。
class SettingsPanel extends StatelessWidget {
  final SettingsController controller;
  const SettingsPanel({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) => Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 720),
          child: ListView(
            padding: const EdgeInsets.all(AppSpacing.s6),
            children: [
              _title(),
              const SizedBox(height: AppSpacing.s5),
              _appearanceSection(),
              const SizedBox(height: AppSpacing.s5),
              _blockSection(),
              const SizedBox(height: AppSpacing.s5),
              _monitorSection(),
              const SizedBox(height: AppSpacing.s5),
              _mcpSection(),
              const SizedBox(height: AppSpacing.s5),
              _aboutSection(context),
              const SizedBox(height: AppSpacing.s6),
            ],
          ),
        ),
      ),
    );
  }

  Widget _title() {
    return const Text(
      '设置',
      style: TextStyle(
        fontSize: 20,
        fontWeight: FontWeight.w700,
        color: AppColors.textPrimary,
      ),
    );
  }

  // ── 外观 ────────────────────────────────────────────────────
  Widget _appearanceSection() {
    return _section('外观', Icons.palette_outlined, [
      _row(
        '主题',
        '深色「Midnight Ops」· 浅色/跟随系统即将支持(偏好已保存)',
        _Segmented<AppThemeMode>(
          value: controller.themeMode,
          options: const [
            (AppThemeMode.dark, '深色'),
            (AppThemeMode.system, '跟随系统'),
          ],
          onSelect: controller.setThemeMode,
        ),
      ),
      _divider(),
      _row(
        '强调色',
        'Cyan · 当前主题强调色',
        Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (final c in const [
              AppColors.accent,
              AppColors.accentTeal,
              AppColors.accentIndigo,
            ])
              Container(
                width: 20,
                height: 20,
                margin: const EdgeInsets.only(left: 6),
                decoration: BoxDecoration(
                  color: c,
                  shape: BoxShape.circle,
                  border: c == AppColors.accent
                      ? Border.all(color: AppColors.textPrimary, width: 2)
                      : null,
                ),
              ),
          ],
        ),
      ),
    ]);
  }

  // ── 命令块 ──────────────────────────────────────────────────
  Widget _blockSection() {
    return _section('命令块', Icons.view_agenda_outlined, [
      _row(
        '字号',
        '命令块文本大小(立即生效)',
        SizedBox(
          width: 220,
          child: Row(
            children: [
              Expanded(
                child: SliderTheme(
                  data: SliderThemeData(
                    trackHeight: 3,
                    activeTrackColor: AppColors.accent,
                    inactiveTrackColor: AppColors.surfaceActive,
                    thumbColor: AppColors.accent,
                    overlayShape: SliderComponentShape.noOverlay,
                    thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 7),
                  ),
                  child: Slider(
                    value: controller.blockFontSize,
                    min: 11,
                    max: 22,
                    divisions: 11,
                    onChanged: controller.setBlockFontSize,
                  ),
                ),
              ),
              SizedBox(
                width: 34,
                child: Text(
                  '${controller.blockFontSize.round()}',
                  textAlign: TextAlign.right,
                  style: const TextStyle(
                    fontSize: 13,
                    color: AppColors.textPrimary,
                    fontFamily: AppFonts.mono,
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    ]);
  }

  // ── 监控 ────────────────────────────────────────────────────
  Widget _monitorSection() {
    return _section('监控', Icons.insights_outlined, [
      _row(
        '采样间隔',
        '每次刷新远端指标的周期',
        _Segmented<int>(
          value: controller.monitorIntervalMs,
          options: const [
            (1000, '1 秒'),
            (2000, '2 秒'),
            (5000, '5 秒'),
          ],
          onSelect: controller.setMonitorIntervalMs,
        ),
      ),
    ]);
  }

  // ── MCP ─────────────────────────────────────────────────────
  Widget _mcpSection() {
    return _section('MCP', Icons.hub_outlined, [
      _row(
        '随应用自动启动',
        '应用启动时自动开启本机 MCP 服务',
        _Switch(
          value: controller.mcpAutoStart,
          onChanged: controller.setMcpAutoStart,
        ),
      ),
    ]);
  }

  // ── 关于 ────────────────────────────────────────────────────
  Widget _aboutSection(BuildContext context) {
    return _section('关于', Icons.info_outline, [
      _row(
        '版本',
        'Conch',
        Text(
          kAppVersion,
          style: const TextStyle(
            fontSize: 13,
            color: AppColors.textPrimary,
            fontFamily: AppFonts.mono,
          ),
        ),
      ),
      _divider(),
      _row(
        '检查更新',
        '查看是否有新版本',
        _MiniButton(
          label: '检查',
          onTap: () => _toast(context, '已是最新版本'),
        ),
      ),
      _divider(),
      _row(
        '项目地址',
        kRepoUrl,
        _MiniButton(
          label: '复制',
          onTap: () async {
            await Clipboard.setData(const ClipboardData(text: kRepoUrl));
            if (context.mounted) _toast(context, '已复制项目地址');
          },
        ),
      ),
    ]);
  }

  void _toast(BuildContext context, String message) {
    ScaffoldMessenger.of(context)
      ..clearSnackBars()
      ..showSnackBar(
        SnackBar(
          content: Text(message),
          behavior: SnackBarBehavior.floating,
          width: 280,
          backgroundColor: AppColors.surface3,
        ),
      );
  }

  // ── 公共片段 ────────────────────────────────────────────────
  Widget _section(String title, IconData icon, List<Widget> children) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.only(left: 2, bottom: AppSpacing.s2),
          child: Row(
            children: [
              Icon(icon, size: 15, color: AppColors.textSecondary),
              const SizedBox(width: 7),
              Text(
                title,
                style: const TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w700,
                  color: AppColors.textSecondary,
                  letterSpacing: 0.3,
                ),
              ),
            ],
          ),
        ),
        Container(
          decoration: BoxDecoration(
            color: AppColors.surface2,
            borderRadius: BorderRadius.circular(AppRadius.xl),
            border: Border.all(color: AppColors.borderDefault),
          ),
          child: Column(children: children),
        ),
      ],
    );
  }

  Widget _divider() => const Divider(height: 1, color: AppColors.borderSubtle);

  Widget _row(String label, String sub, Widget control) {
    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.s4,
        vertical: AppSpacing.s3,
      ),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  label,
                  style: const TextStyle(
                    fontSize: 14,
                    color: AppColors.textPrimary,
                    fontWeight: FontWeight.w500,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  sub,
                  style: const TextStyle(fontSize: 12, color: AppColors.textMuted),
                ),
              ],
            ),
          ),
          const SizedBox(width: AppSpacing.s4),
          control,
        ],
      ),
    );
  }
}

/// 分段选择器(通用值类型)。
class _Segmented<T> extends StatelessWidget {
  final T value;
  final List<(T, String)> options;
  final ValueChanged<T> onSelect;

  const _Segmented({
    required this.value,
    required this.options,
    required this.onSelect,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(3),
      decoration: BoxDecoration(
        color: AppColors.canvas,
        borderRadius: BorderRadius.circular(AppRadius.md),
        border: Border.all(color: AppColors.borderSubtle),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final (v, label) in options)
            GestureDetector(
              onTap: () => onSelect(v),
              child: AnimatedContainer(
                duration: const Duration(milliseconds: 140),
                padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                decoration: BoxDecoration(
                  color: v == value ? AppColors.accent : Colors.transparent,
                  borderRadius: BorderRadius.circular(AppRadius.sm),
                ),
                child: Text(
                  label,
                  style: TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                    color: v == value ? AppColors.textInverse : AppColors.textSecondary,
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// 自定义开关(贴合 Midnight Ops 主题)。
class _Switch extends StatelessWidget {
  final bool value;
  final ValueChanged<bool> onChanged;

  const _Switch({required this.value, required this.onChanged});

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: () => onChanged(!value),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 160),
        width: 44,
        height: 25,
        padding: const EdgeInsets.all(3),
        decoration: BoxDecoration(
          color: value ? AppColors.accent : AppColors.surfaceActive,
          borderRadius: BorderRadius.circular(99),
        ),
        child: AnimatedAlign(
          duration: const Duration(milliseconds: 160),
          curve: Curves.easeOut,
          alignment: value ? Alignment.centerRight : Alignment.centerLeft,
          child: Container(
            width: 19,
            height: 19,
            decoration: const BoxDecoration(
              color: Colors.white,
              shape: BoxShape.circle,
            ),
          ),
        ),
      ),
    );
  }
}

/// 小号次要按钮(关于区:检查 / 复制)。
class _MiniButton extends StatelessWidget {
  final String label;
  final VoidCallback onTap;

  const _MiniButton({required this.label, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return Material(
      color: AppColors.surface3,
      borderRadius: BorderRadius.circular(AppRadius.md),
      child: InkWell(
        borderRadius: BorderRadius.circular(AppRadius.md),
        hoverColor: AppColors.surfaceHover,
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 7),
          child: Text(
            label,
            style: const TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w600,
              color: AppColors.textSecondary,
            ),
          ),
        ),
      ),
    );
  }
}
