import 'package:flutter/material.dart';

import '../../shared/theme/app_colors.dart';
import '../../shared/theme/app_dimens.dart';
import '../../src/rust/api/monitor.dart' as rust;
import 'monitor_controller.dart';
import 'monitor_format.dart';
import 'monitor_widgets.dart';

/// 监控整页仪表盘（导航「监控」专属）：跟随当前选中连接，宽屏自适应铺开
/// CPU / 内存 / 磁盘 / 网络 / 系统信息 / 多盘明细，区别于终端视图右侧的窄监控侧栏。
class MonitorPage extends StatelessWidget {
  final MonitorController controller;

  /// 跳去「连接」页选择一个连接（空态引导按钮）。
  final VoidCallback? onPickConnection;

  const MonitorPage({super.key, required this.controller, this.onPickConnection});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _header(),
          const Divider(height: 1, color: AppColors.borderSubtle),
          Expanded(
            child: (controller.connection == null && !controller.isLocal)
                ? _empty()
                : _dashboard(),
          ),
        ],
      ),
    );
  }

  // ── 顶栏 ────────────────────────────────────────────────────
  Widget _header() {
    final c = controller.connection;
    return Container(
      height: AppLayout.tabBarHeight + 8,
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.s5),
      child: Row(
        children: [
          const Icon(Icons.insights_outlined, size: 18, color: AppColors.accent),
          const SizedBox(width: AppSpacing.s2),
          const Text(
            '监控',
            style: TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w700,
              color: AppColors.textPrimary,
            ),
          ),
          if (controller.isLocal) ...[
            const SizedBox(width: AppSpacing.s3),
            const Text(
              '本机 · 这台电脑',
              style: TextStyle(
                fontSize: 12,
                color: AppColors.textMuted,
                fontFamily: AppFonts.mono,
              ),
            ),
          ] else if (c != null) ...[
            const SizedBox(width: AppSpacing.s3),
            Flexible(
              child: Text(
                c.protocol == 'ADB'
                    ? '${c.name} · adb · ${c.host}:${c.port}'
                    : '${c.name} · ${c.username}@${c.host}:${c.port}',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                  fontSize: 12,
                  color: AppColors.textMuted,
                  fontFamily: AppFonts.mono,
                ),
              ),
            ),
          ],
          const SizedBox(width: AppSpacing.s3),
          _statusPill(),
          const Spacer(),
          if (controller.status == MonitorStatus.error)
            _ghostButton(Icons.refresh, '重试', controller.retryNow),
        ],
      ),
    );
  }

  Widget _statusPill() {
    final (label, color, dot) = switch (controller.status) {
      MonitorStatus.idle => ('未选择', AppColors.textMuted, false),
      MonitorStatus.connecting => ('连接中', AppColors.warning, false),
      MonitorStatus.live => ('实时', AppColors.online, true),
      MonitorStatus.error => ('已断开', AppColors.danger, false),
    };
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 3),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.13),
        borderRadius: BorderRadius.circular(99),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (dot) ...[
            Container(
              width: 6,
              height: 6,
              decoration: BoxDecoration(color: color, shape: BoxShape.circle),
            ),
            const SizedBox(width: 5),
          ],
          Text(
            label,
            style: TextStyle(fontSize: 11, color: color, fontWeight: FontWeight.w600),
          ),
        ],
      ),
    );
  }

  Widget _ghostButton(IconData icon, String label, VoidCallback onTap) {
    return Material(
      color: AppColors.surface3,
      borderRadius: BorderRadius.circular(AppRadius.md),
      child: InkWell(
        borderRadius: BorderRadius.circular(AppRadius.md),
        hoverColor: AppColors.surfaceHover,
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 7),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(icon, size: 14, color: AppColors.textSecondary),
              const SizedBox(width: 6),
              Text(
                label,
                style: const TextStyle(fontSize: 12, color: AppColors.textSecondary),
              ),
            ],
          ),
        ),
      ),
    );
  }

  // ── 空态 ────────────────────────────────────────────────────
  Widget _empty() {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            Icons.insights_outlined,
            size: 46,
            color: AppColors.textMuted.withValues(alpha: 0.5),
          ),
          const SizedBox(height: AppSpacing.s4),
          const Text(
            '选择一个连接以查看实时监控',
            style: TextStyle(fontSize: 15, color: AppColors.textSecondary),
          ),
          const SizedBox(height: AppSpacing.s2),
          const Text(
            '在左侧连接树点选，或到「连接」页选择主机',
            style: TextStyle(fontSize: 12, color: AppColors.textMuted),
          ),
          if (onPickConnection != null) ...[
            const SizedBox(height: AppSpacing.s4),
            _ghostButton(Icons.dns_outlined, '前往连接', onPickConnection!),
          ],
        ],
      ),
    );
  }

  // ── 仪表盘 ──────────────────────────────────────────────────
  Widget _dashboard() {
    final s = controller.latest;
    return LayoutBuilder(
      builder: (context, constraints) {
        final w = constraints.maxWidth - AppSpacing.s5 * 2;
        const gap = AppSpacing.s4;
        final cols = w >= 1080 ? 3 : (w >= 680 ? 2 : 1);
        final statW = (w - gap * (cols - 1)) / cols;
        final halfW = w >= 760 ? (w - gap) / 2 : w;
        return ListView(
          padding: const EdgeInsets.all(AppSpacing.s5),
          children: [
            if (_banner(s) case final b?) ...[b, const SizedBox(height: gap)],
            Wrap(
              spacing: gap,
              runSpacing: gap,
              children: [
                SizedBox(width: statW, child: _cpuCard(s)),
                SizedBox(width: statW, child: _memCard(s)),
                SizedBox(width: statW, child: _diskHeroCard(s)),
              ],
            ),
            const SizedBox(height: gap),
            _netCard(s),
            const SizedBox(height: gap),
            Wrap(
              spacing: gap,
              runSpacing: gap,
              children: [
                SizedBox(width: halfW, child: _systemCard(s)),
                SizedBox(width: halfW, child: _volumesCard(s)),
              ],
            ),
          ],
        );
      },
    );
  }

  Widget? _banner(rust.SystemStatsDto? s) {
    switch (controller.status) {
      case MonitorStatus.connecting:
        if (s != null) return null;
        final host = controller.connection?.host ?? '';
        return _infoBanner(Icons.sync, AppColors.warning, '正在采集 $host 指标…');
      case MonitorStatus.error:
        return _errorBanner();
      case MonitorStatus.idle:
      case MonitorStatus.live:
        return null;
    }
  }

  Widget _infoBanner(IconData icon, Color color, String text) {
    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.s4,
        vertical: AppSpacing.s3,
      ),
      decoration: BoxDecoration(
        color: AppColors.surface3,
        borderRadius: BorderRadius.circular(AppRadius.md),
        border: Border.all(color: AppColors.borderSubtle),
      ),
      child: Row(
        children: [
          Icon(icon, size: 15, color: color),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              text,
              style: const TextStyle(fontSize: 13, color: AppColors.textSecondary),
            ),
          ),
        ],
      ),
    );
  }

  Widget _errorBanner() {
    return Container(
      padding: const EdgeInsets.all(AppSpacing.s4),
      decoration: BoxDecoration(
        color: AppColors.danger.withValues(alpha: 0.10),
        borderRadius: BorderRadius.circular(AppRadius.md),
        border: Border.all(color: AppColors.danger.withValues(alpha: 0.35)),
      ),
      child: Row(
        children: [
          const Icon(Icons.error_outline, size: 16, color: AppColors.danger),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text(
                  '监控已断开',
                  style: TextStyle(
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    color: AppColors.textPrimary,
                  ),
                ),
                if (controller.error != null) ...[
                  const SizedBox(height: 4),
                  Text(
                    controller.error!,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(fontSize: 12, color: AppColors.textMuted),
                  ),
                ],
              ],
            ),
          ),
          const SizedBox(width: AppSpacing.s3),
          _ghostButton(Icons.refresh, '重试', controller.retryNow),
        ],
      ),
    );
  }

  // ── CPU ─────────────────────────────────────────────────────
  Widget _cpuCard(rust.SystemStatsDto? s) {
    final percent = s?.cpuPercent;
    final color = percent == null ? AppColors.accent : monitorThresholdColor(percent);
    return _card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _cardTitle(Icons.memory_outlined, 'CPU 使用率'),
          const SizedBox(height: 10),
          _bigValue(percent == null ? '—' : formatPercent(percent), color),
          const SizedBox(height: 12),
          MetricSparkline(
            series: [MetricSeries(controller.cpuHistory, color)],
            minY: 0,
            maxY: 100,
            height: 64,
          ),
          const SizedBox(height: 10),
          _cardSub(
            s == null
                ? '等待数据…'
                : '${s.cpuCores} 核 · 负载 ${s.load1.toStringAsFixed(2)}',
          ),
        ],
      ),
    );
  }

  // ── 内存 ────────────────────────────────────────────────────
  Widget _memCard(rust.SystemStatsDto? s) {
    final percent = s?.memPercent;
    final color = percent == null ? AppColors.accent : monitorThresholdColor(percent);
    return _card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _cardTitle(Icons.developer_board_outlined, '内存使用率'),
          const SizedBox(height: 10),
          _bigValue(percent == null ? '—' : formatPercent(percent), color),
          const SizedBox(height: 14),
          _bar(percent),
          const SizedBox(height: 12),
          _cardSub(
            s == null
                ? '等待数据…'
                : formatKbPair(s.memUsedKb.toInt(), s.memTotalKb.toInt()),
          ),
          const SizedBox(height: 6),
          MetricSparkline(
            series: [MetricSeries(controller.memHistory, color)],
            minY: 0,
            maxY: 100,
            height: 36,
          ),
        ],
      ),
    );
  }

  // ── 磁盘（主盘 hero）──────────────────────────────────────────
  Widget _diskHeroCard(rust.SystemStatsDto? s) {
    final percent = s?.diskPercent;
    final color = percent == null ? AppColors.accent : monitorThresholdColor(percent);
    final disks = s?.disks ?? const <rust.DiskUsageDto>[];
    final mount = disks.isNotEmpty ? disks.first.mount : null;
    return _card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(child: _cardTitle(Icons.storage_outlined, '磁盘使用率')),
              if (disks.length > 1)
                Text(
                  '${disks.length} 个卷',
                  style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
                ),
            ],
          ),
          const SizedBox(height: 10),
          _bigValue(percent == null ? '—' : formatPercent(percent), color),
          const SizedBox(height: 14),
          _bar(percent),
          const SizedBox(height: 12),
          if (s != null)
            _cardSub(
              mount == null
                  ? formatKbPair(s.diskUsedKb.toInt(), s.diskTotalKb.toInt())
                  : '$mount   ${formatKbPair(s.diskUsedKb.toInt(), s.diskTotalKb.toInt())}',
            )
          else
            _cardSub('等待数据…'),
        ],
      ),
    );
  }

  // ── 网络（双线大图）──────────────────────────────────────────
  Widget _netCard(rust.SystemStatsDto? s) {
    final rx = s == null ? '—' : formatRate(s.netRxPerSec);
    final tx = s == null ? '—' : formatRate(s.netTxPerSec);
    return _card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              _cardTitle(Icons.swap_vert, '网络吞吐'),
              const Spacer(),
              _netStat(Icons.south, AppColors.accentTeal, '接收', rx),
              const SizedBox(width: AppSpacing.s6),
              _netStat(Icons.north, AppColors.accentIndigo, '发送', tx),
            ],
          ),
          const SizedBox(height: 14),
          MetricSparkline(
            series: [
              MetricSeries(controller.netRxHistory, AppColors.accentTeal),
              MetricSeries(controller.netTxHistory, AppColors.accentIndigo),
            ],
            minY: 0,
            height: 96,
          ),
        ],
      ),
    );
  }

  Widget _netStat(IconData icon, Color color, String label, String value) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 15, color: color),
        const SizedBox(width: 5),
        Text(
          label,
          style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
        ),
        const SizedBox(width: 6),
        Text(
          value,
          style: const TextStyle(
            fontSize: 16,
            fontWeight: FontWeight.w700,
            color: AppColors.textPrimary,
            fontFamily: AppFonts.mono,
            fontFeatures: [FontFeature.tabularFigures()],
          ),
        ),
      ],
    );
  }

  // ── 系统信息 ────────────────────────────────────────────────
  Widget _systemCard(rust.SystemStatsDto? s) {
    return _card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _cardTitle(Icons.dns_outlined, '系统信息'),
          const SizedBox(height: AppSpacing.s3),
          _infoRow(Icons.computer_outlined, '操作系统', s == null || s.os.isEmpty ? '—' : s.os),
          const Divider(height: AppSpacing.s5, color: AppColors.borderSubtle),
          _infoRow(Icons.schedule_outlined, '运行时长', s == null ? '—' : formatUptime(s.uptimeSecs)),
          const Divider(height: AppSpacing.s5, color: AppColors.borderSubtle),
          _infoRow(Icons.memory_outlined, 'CPU 核心', s == null ? '—' : '${s.cpuCores} 核'),
          const Divider(height: AppSpacing.s5, color: AppColors.borderSubtle),
          _infoRow(
            Icons.speed_outlined,
            '平均负载',
            s == null ? '—' : s.load1.toStringAsFixed(2),
          ),
        ],
      ),
    );
  }

  Widget _infoRow(IconData icon, String label, String value) {
    return Row(
      children: [
        Icon(icon, size: 14, color: AppColors.textMuted),
        const SizedBox(width: 8),
        Text(label, style: const TextStyle(fontSize: 12, color: AppColors.textSecondary)),
        const SizedBox(width: AppSpacing.s4),
        Expanded(
          child: Text(
            value,
            textAlign: TextAlign.right,
            overflow: TextOverflow.ellipsis,
            style: const TextStyle(
              fontSize: 12.5,
              color: AppColors.textPrimary,
              fontFamily: AppFonts.mono,
            ),
          ),
        ),
      ],
    );
  }

  // ── 磁盘明细（多盘）──────────────────────────────────────────
  Widget _volumesCard(rust.SystemStatsDto? s) {
    final disks = s?.disks ?? const <rust.DiskUsageDto>[];
    return _card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _cardTitle(Icons.dataset_outlined, '磁盘明细'),
          const SizedBox(height: AppSpacing.s3),
          if (disks.isEmpty)
            _cardSub('等待数据…')
          else
            for (var i = 0; i < disks.length; i++) ...[
              if (i > 0) const SizedBox(height: AppSpacing.s4),
              _volumeRow(disks[i]),
            ],
        ],
      ),
    );
  }

  Widget _volumeRow(rust.DiskUsageDto d) {
    final color = monitorThresholdColor(d.percent);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                d.mount,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(
                  fontSize: 12.5,
                  color: AppColors.textSecondary,
                  fontFamily: AppFonts.mono,
                ),
              ),
            ),
            const SizedBox(width: 8),
            Text(
              formatKbPair(d.usedKb.toInt(), d.totalKb.toInt()),
              style: const TextStyle(
                fontSize: 11,
                color: AppColors.textMuted,
                fontFamily: AppFonts.mono,
              ),
            ),
            const SizedBox(width: 10),
            Text(
              formatPercent(d.percent),
              style: TextStyle(
                fontSize: 12,
                fontWeight: FontWeight.w600,
                color: color,
                fontFamily: AppFonts.mono,
                fontFeatures: const [FontFeature.tabularFigures()],
              ),
            ),
          ],
        ),
        const SizedBox(height: 6),
        ClipRRect(
          borderRadius: BorderRadius.circular(99),
          child: LinearProgressIndicator(
            value: (d.percent / 100).clamp(0.0, 1.0),
            minHeight: 5,
            backgroundColor: AppColors.surfaceActive,
            color: color,
          ),
        ),
      ],
    );
  }

  // ── 公共片段 ────────────────────────────────────────────────
  Widget _card({required Widget child}) {
    return Container(
      padding: const EdgeInsets.all(AppSpacing.s4),
      decoration: BoxDecoration(
        color: AppColors.surface3,
        borderRadius: BorderRadius.circular(AppRadius.lg),
        border: Border.all(color: AppColors.borderSubtle),
      ),
      child: child,
    );
  }

  Widget _cardTitle(IconData icon, String title) => Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 14, color: AppColors.textMuted),
          const SizedBox(width: 6),
          Text(
            title,
            style: const TextStyle(fontSize: 12.5, color: AppColors.textSecondary),
          ),
        ],
      );

  Widget _bigValue(String value, Color color) => Text(
        value,
        style: TextStyle(
          fontSize: 34,
          fontWeight: FontWeight.w700,
          color: color,
          fontFamily: AppFonts.mono,
          fontFeatures: const [FontFeature.tabularFigures()],
        ),
      );

  Widget _cardSub(String sub) => Text(
        sub,
        style: const TextStyle(fontSize: 11.5, color: AppColors.textMuted),
      );

  Widget _bar(double? percent) {
    final color = percent == null ? AppColors.accent : monitorThresholdColor(percent);
    return ClipRRect(
      borderRadius: BorderRadius.circular(99),
      child: LinearProgressIndicator(
        value: percent == null ? 0 : (percent / 100).clamp(0.0, 1.0),
        minHeight: 8,
        backgroundColor: AppColors.surfaceActive,
        color: color,
      ),
    );
  }
}
