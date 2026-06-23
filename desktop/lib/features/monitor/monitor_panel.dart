import 'package:flutter/material.dart';

import '../../shared/theme/app_colors.dart';
import '../../shared/theme/app_dimens.dart';
import '../../src/rust/api/monitor.dart' as rust;
import 'monitor_controller.dart';
import 'monitor_format.dart';
import 'monitor_widgets.dart';

/// 监控侧栏:跟随当前选中连接,实时渲染 CPU / 内存 / 磁盘 / 网络指标 + 走势图。
///
/// 数据由 [MonitorController] 驱动(订阅 Rust `stats_stream`);本组件只负责呈现。
class MonitorPanel extends StatelessWidget {
  final MonitorController controller;
  const MonitorPanel({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return Container(
      width: AppLayout.monitorWidth,
      decoration: const BoxDecoration(
        color: AppColors.surface1,
        border: Border(left: BorderSide(color: AppColors.borderSubtle)),
      ),
      child: ListenableBuilder(
        listenable: controller,
        builder: (context, _) => Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            _header(),
            Expanded(child: _body()),
          ],
        ),
      ),
    );
  }

  // ── 头部 ────────────────────────────────────────────────────
  Widget _header() {
    return Padding(
      padding: const EdgeInsets.fromLTRB(
        AppSpacing.s4,
        AppSpacing.s4,
        AppSpacing.s3,
        AppSpacing.s2,
      ),
      child: Row(
        children: [
          const Icon(
            Icons.monitor_heart_outlined,
            size: 16,
            color: AppColors.accent,
          ),
          const SizedBox(width: 6),
          const Expanded(
            child: Text(
              '服务器监控',
              style: TextStyle(
                fontSize: 14,
                fontWeight: FontWeight.w600,
                color: AppColors.textPrimary,
              ),
            ),
          ),
          _statusPill(),
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
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
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

  // ── 主体 ────────────────────────────────────────────────────
  Widget _body() {
    final s = controller.latest;
    return ListView(
      padding: const EdgeInsets.fromLTRB(
        AppSpacing.s4,
        AppSpacing.s2,
        AppSpacing.s4,
        AppSpacing.s4,
      ),
      children: [
        _contextBanner(s),
        _cpuCard(s),
        _gaugeCard(
          '内存使用率',
          s?.memPercent,
          s == null ? null : formatKbPair(s.memUsedKb.toInt(), s.memTotalKb.toInt()),
        ),
        _diskCard(s),
        _netCard(s),
        if (s != null) _infoFooter(s),
      ],
    );
  }

  /// 顶部状态提示:空闲提示 / 采集中 / 出错重试。实时有数据时不显示。
  Widget _contextBanner(rust.SystemStatsDto? s) {
    switch (controller.status) {
      case MonitorStatus.idle:
        return _banner(
          icon: Icons.touch_app_outlined,
          color: AppColors.textMuted,
          text: '选择左侧连接以开始监控',
        );
      case MonitorStatus.connecting:
        if (s != null) return const SizedBox.shrink();
        final host = controller.connection?.host ?? '';
        return _banner(
          icon: Icons.sync,
          color: AppColors.warning,
          text: '正在采集 $host 指标…',
        );
      case MonitorStatus.error:
        return _errorBanner();
      case MonitorStatus.live:
        return const SizedBox.shrink();
    }
  }

  Widget _banner({
    required IconData icon,
    required Color color,
    required String text,
  }) {
    return Container(
      margin: const EdgeInsets.only(bottom: AppSpacing.s3),
      padding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.s3,
        vertical: AppSpacing.s2,
      ),
      decoration: BoxDecoration(
        color: AppColors.surface3,
        borderRadius: BorderRadius.circular(AppRadius.md),
        border: Border.all(color: AppColors.borderSubtle),
      ),
      child: Row(
        children: [
          Icon(icon, size: 14, color: color),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              text,
              style: const TextStyle(fontSize: 12, color: AppColors.textSecondary),
            ),
          ),
        ],
      ),
    );
  }

  Widget _errorBanner() {
    return Container(
      margin: const EdgeInsets.only(bottom: AppSpacing.s3),
      padding: const EdgeInsets.all(AppSpacing.s3),
      decoration: BoxDecoration(
        color: AppColors.danger.withValues(alpha: 0.10),
        borderRadius: BorderRadius.circular(AppRadius.md),
        border: Border.all(color: AppColors.danger.withValues(alpha: 0.35)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(Icons.error_outline, size: 14, color: AppColors.danger),
              const SizedBox(width: 8),
              const Expanded(
                child: Text(
                  '监控已断开',
                  style: TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                    color: AppColors.textPrimary,
                  ),
                ),
              ),
              InkWell(
                borderRadius: BorderRadius.circular(AppRadius.sm),
                onTap: controller.retryNow,
                child: const Padding(
                  padding: EdgeInsets.symmetric(horizontal: 8, vertical: 3),
                  child: Text(
                    '重试',
                    style: TextStyle(
                      fontSize: 12,
                      color: AppColors.accent,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
              ),
            ],
          ),
          if (controller.error != null) ...[
            const SizedBox(height: 6),
            Text(
              controller.error!,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
            ),
          ],
        ],
      ),
    );
  }

  // ── CPU 卡(hero:大数值 + 面积走势图)──────────────────────
  Widget _cpuCard(rust.SystemStatsDto? s) {
    final percent = s?.cpuPercent;
    final color = percent == null ? AppColors.accent : monitorThresholdColor(percent);
    return _card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _cardTitle('CPU 使用率'),
          const SizedBox(height: 6),
          _bigValue(percent == null ? '—' : formatPercent(percent), color),
          const SizedBox(height: 8),
          MetricSparkline(
            series: [MetricSeries(controller.cpuHistory, color)],
            minY: 0,
            maxY: 100,
          ),
          const SizedBox(height: 8),
          _cardSub(
            s == null
                ? '实时数据'
                : '${s.cpuCores} 核 · 负载 ${s.load1.toStringAsFixed(2)}',
          ),
        ],
      ),
    );
  }

  // ── 内存 / 磁盘卡(数值 + 进度条 + used/total)────────────────
  Widget _gaugeCard(String title, double? percent, String? detail) {
    final color = percent == null ? AppColors.accent : monitorThresholdColor(percent);
    return _card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _cardTitle(title),
          const SizedBox(height: 6),
          _bigValue(percent == null ? '—' : formatPercent(percent), color),
          const SizedBox(height: 10),
          ClipRRect(
            borderRadius: BorderRadius.circular(99),
            child: LinearProgressIndicator(
              value: percent == null ? 0 : (percent / 100).clamp(0.0, 1.0),
              minHeight: 6,
              backgroundColor: AppColors.surfaceActive,
              color: color,
            ),
          ),
          if (detail != null) ...[
            const SizedBox(height: 8),
            _cardSub(detail),
          ],
        ],
      ),
    );
  }

  // ── 磁盘卡(主盘 hero + 多盘明细)────────────────────────────
  /// headline = 主盘(最大的真实卷,安卓上即 `/data` 内置存储);其余卷在分隔线
  /// 下逐条列出(挂载点 + 进度条 + used/total),满足「多盘」展示。
  Widget _diskCard(rust.SystemStatsDto? s) {
    final percent = s?.diskPercent;
    final color = percent == null ? AppColors.accent : monitorThresholdColor(percent);
    final disks = s?.disks ?? const <rust.DiskUsageDto>[];
    final primaryMount = disks.isNotEmpty ? disks.first.mount : null;
    final extra = disks.length > 1
        ? disks.sublist(1)
        : const <rust.DiskUsageDto>[];
    return _card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(child: _cardTitle('磁盘使用率')),
              if (disks.length > 1)
                Text(
                  '${disks.length} 个卷',
                  style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
                ),
            ],
          ),
          const SizedBox(height: 6),
          _bigValue(percent == null ? '—' : formatPercent(percent), color),
          const SizedBox(height: 10),
          ClipRRect(
            borderRadius: BorderRadius.circular(99),
            child: LinearProgressIndicator(
              value: percent == null ? 0 : (percent / 100).clamp(0.0, 1.0),
              minHeight: 6,
              backgroundColor: AppColors.surfaceActive,
              color: color,
            ),
          ),
          if (s != null) ...[
            const SizedBox(height: 8),
            _cardSub(
              primaryMount == null
                  ? formatKbPair(s.diskUsedKb.toInt(), s.diskTotalKb.toInt())
                  : '$primaryMount   '
                      '${formatKbPair(s.diskUsedKb.toInt(), s.diskTotalKb.toInt())}',
            ),
          ],
          if (extra.isNotEmpty) ...[
            const SizedBox(height: 12),
            const Divider(height: 1, color: AppColors.borderSubtle),
            const SizedBox(height: 12),
            for (var i = 0; i < extra.length; i++) ...[
              if (i > 0) const SizedBox(height: 12),
              _diskRow(extra[i]),
            ],
          ],
        ],
      ),
    );
  }

  /// 单条磁盘明细:挂载点 + 百分比 + 细进度条 + used/total。
  Widget _diskRow(rust.DiskUsageDto d) {
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
                  fontSize: 12,
                  color: AppColors.textSecondary,
                  fontFamily: AppFonts.mono,
                ),
              ),
            ),
            const SizedBox(width: 8),
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
        const SizedBox(height: 5),
        ClipRRect(
          borderRadius: BorderRadius.circular(99),
          child: LinearProgressIndicator(
            value: (d.percent / 100).clamp(0.0, 1.0),
            minHeight: 5,
            backgroundColor: AppColors.surfaceActive,
            color: color,
          ),
        ),
        const SizedBox(height: 4),
        _cardSub(formatKbPair(d.usedKb.toInt(), d.totalKb.toInt())),
      ],
    );
  }

  // ── 网络卡(↓/↑ 速率 + 双线走势图)──────────────────────────
  Widget _netCard(rust.SystemStatsDto? s) {
    final rx = s == null ? '—' : formatRate(s.netRxPerSec);
    final tx = s == null ? '—' : formatRate(s.netTxPerSec);
    return _card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _cardTitle('网络吞吐'),
          const SizedBox(height: 6),
          Row(
            children: [
              Expanded(child: _netStat(Icons.south, AppColors.accentTeal, rx)),
              Expanded(child: _netStat(Icons.north, AppColors.accentIndigo, tx)),
            ],
          ),
          const SizedBox(height: 8),
          MetricSparkline(
            series: [
              MetricSeries(controller.netRxHistory, AppColors.accentTeal),
              MetricSeries(controller.netTxHistory, AppColors.accentIndigo),
            ],
            minY: 0,
          ),
          const SizedBox(height: 8),
          _cardSub('↓ 接收      ↑ 发送'),
        ],
      ),
    );
  }

  Widget _netStat(IconData icon, Color color, String value) {
    return Row(
      children: [
        Icon(icon, size: 13, color: color),
        const SizedBox(width: 4),
        Flexible(
          child: Text(
            value,
            overflow: TextOverflow.ellipsis,
            style: const TextStyle(
              fontSize: 15,
              fontWeight: FontWeight.w700,
              color: AppColors.textPrimary,
              fontFamily: AppFonts.mono,
              fontFeatures: [FontFeature.tabularFigures()],
            ),
          ),
        ),
      ],
    );
  }

  // ── 系统信息页脚(OS + 运行时长)────────────────────────────
  Widget _infoFooter(rust.SystemStatsDto s) {
    return _card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _infoRow(Icons.dns_outlined, '系统', s.os.isEmpty ? '未知' : s.os),
          const SizedBox(height: 8),
          _infoRow(Icons.schedule_outlined, '运行', formatUptime(s.uptimeSecs)),
        ],
      ),
    );
  }

  Widget _infoRow(IconData icon, String label, String value) {
    return Row(
      children: [
        Icon(icon, size: 13, color: AppColors.textMuted),
        const SizedBox(width: 6),
        Text(
          label,
          style: const TextStyle(fontSize: 12, color: AppColors.textSecondary),
        ),
        const SizedBox(width: AppSpacing.s3),
        Expanded(
          child: Text(
            value,
            textAlign: TextAlign.right,
            overflow: TextOverflow.ellipsis,
            style: const TextStyle(
              fontSize: 12,
              color: AppColors.textPrimary,
              fontFamily: AppFonts.mono,
            ),
          ),
        ),
      ],
    );
  }

  // ── 公共片段 ────────────────────────────────────────────────
  Widget _card({required Widget child}) {
    return Container(
      margin: const EdgeInsets.only(bottom: AppSpacing.s3),
      padding: const EdgeInsets.all(AppSpacing.s3),
      decoration: BoxDecoration(
        color: AppColors.surface3,
        borderRadius: BorderRadius.circular(AppRadius.lg),
        border: Border.all(color: AppColors.borderSubtle),
      ),
      child: child,
    );
  }

  Widget _cardTitle(String title) => Text(
        title,
        style: const TextStyle(fontSize: 12, color: AppColors.textSecondary),
      );

  Widget _bigValue(String value, Color color) => Text(
        value,
        style: TextStyle(
          fontSize: 24,
          fontWeight: FontWeight.w700,
          color: color,
          fontFamily: AppFonts.mono,
          fontFeatures: const [FontFeature.tabularFigures()],
        ),
      );

  Widget _cardSub(String sub) => Text(
        sub,
        style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
      );
}
