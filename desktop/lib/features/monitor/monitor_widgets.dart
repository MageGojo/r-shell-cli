import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';

import '../../shared/theme/app_colors.dart';

/// 监控阈值着色：>92% 危险、>80% 警告、其余强调色。监控侧栏与整页共用。
Color monitorThresholdColor(double percent) {
  if (percent >= 92) return AppColors.danger;
  if (percent >= 80) return AppColors.warning;
  return AppColors.accent;
}

/// 一条走势序列（数据 + 颜色）。
class MetricSeries {
  final List<double> data;
  final Color color;
  const MetricSeries(this.data, this.color);
}

/// 极简走势图（无网格 / 坐标轴 / 触摸）：平滑折线 + 顶部渐变填充。
/// 支持多序列共享 Y 轴（网络的 ↓/↑ 双线）。监控侧栏与整页共用，高度可配。
class MetricSparkline extends StatelessWidget {
  final List<MetricSeries> series;
  final double minY;

  /// 固定上限（如 CPU 100）；为 null 则按数据自动缩放。
  final double? maxY;
  final double height;

  const MetricSparkline({
    super.key,
    required this.series,
    this.minY = 0,
    this.maxY,
    this.height = 44,
  });

  @override
  Widget build(BuildContext context) {
    final hasData = series.any((s) => s.data.isNotEmpty);
    if (!hasData) {
      return SizedBox(
        height: height,
        child: Center(child: Container(height: 1, color: AppColors.borderSubtle)),
      );
    }

    final maxLen = series.fold<int>(0, (m, s) => s.data.length > m ? s.data.length : m);
    final computedMax = maxY ?? _autoMax();

    return SizedBox(
      height: height,
      child: LineChart(
        LineChartData(
          minX: 0,
          maxX: (maxLen - 1).clamp(1, double.infinity).toDouble(),
          minY: minY,
          maxY: computedMax,
          gridData: const FlGridData(show: false),
          titlesData: const FlTitlesData(show: false),
          borderData: FlBorderData(show: false),
          lineTouchData: const LineTouchData(enabled: false),
          lineBarsData: [
            for (final s in series)
              if (s.data.isNotEmpty)
                LineChartBarData(
                  spots: [
                    for (var i = 0; i < s.data.length; i++)
                      FlSpot(i.toDouble(), s.data[i]),
                  ],
                  isCurved: true,
                  curveSmoothness: 0.2,
                  color: s.color,
                  barWidth: 1.8,
                  dotData: const FlDotData(show: false),
                  belowBarData: BarAreaData(
                    show: true,
                    gradient: LinearGradient(
                      begin: Alignment.topCenter,
                      end: Alignment.bottomCenter,
                      colors: [
                        s.color.withValues(alpha: 0.26),
                        s.color.withValues(alpha: 0.0),
                      ],
                    ),
                  ),
                ),
          ],
        ),
      ),
    );
  }

  double _autoMax() {
    var peak = 0.0;
    for (final s in series) {
      for (final v in s.data) {
        if (v > peak) peak = v;
      }
    }
    // 留 25% 顶部余量；避免全 0 时塌成一条线。
    return (peak * 1.25).clamp(1.0, double.infinity);
  }
}
