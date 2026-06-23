// 监控数值的展示格式化。与 core `monitor.rs` 的 Rust 版一一对应,但按项目约定
// (同 SFTP)把格式化放在 Dart 侧;Rust 只回传干净的数值。

/// 人类可读的字节速率,如 `1.5 MB/s`。
String formatRate(double bytesPerSec) => '${formatBytes(bytesPerSec)}/s';

/// 人类可读的字节大小(浮点),如 `512 B` / `1.5 MB`。
String formatBytes(double bytes) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  var value = bytes < 0 ? 0.0 : bytes;
  var unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return unit == 0
      ? '${value.toStringAsFixed(0)} ${units[unit]}'
      : '${value.toStringAsFixed(1)} ${units[unit]}';
}

/// 共享一个单位(按 total 缩放)的 `used/total`,如 `4.9/7.8 GB`。
String formatKbPair(int usedKb, int totalKb) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  var total = totalKb * 1024.0;
  var used = usedKb * 1024.0;
  var unit = 0;
  while (total >= 1024 && unit < units.length - 1) {
    total /= 1024;
    used /= 1024;
    unit++;
  }
  final digits = unit == 0 ? 0 : 1;
  return '${used.toStringAsFixed(digits)}/${total.toStringAsFixed(digits)} ${units[unit]}';
}

/// 把运行时长(秒)格式化为 `Xd Yh Zm`。
String formatUptime(double secs) {
  final total = secs.toInt();
  final days = total ~/ 86400;
  final hours = (total % 86400) ~/ 3600;
  final minutes = (total % 3600) ~/ 60;
  if (days > 0) return '${days}d ${hours}h ${minutes}m';
  if (hours > 0) return '${hours}h ${minutes}m';
  return '${minutes}m';
}

/// 百分比headline(整数,等宽展示),如 `42%`。
String formatPercent(double percent) => '${percent.clamp(0, 100).toStringAsFixed(0)}%';
