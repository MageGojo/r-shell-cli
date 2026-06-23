import 'package:flutter_test/flutter_test.dart';
import 'package:r_shell_desktop/features/monitor/monitor_format.dart';

/// 监控格式化是 core `monitor.rs` Rust 版的 Dart 对照实现,这里用与
/// `core::monitor::tests::formats_helpers` 相同的样例固定行为,防止两侧漂移。
void main() {
  group('monitor_format', () {
    test('formatBytes 按 1024 进位', () {
      expect(formatBytes(512), '512 B');
      expect(formatBytes(1536), '1.5 KB');
    });

    test('formatRate 追加 /s', () {
      expect(formatRate(1536), '1.5 KB/s');
      expect(formatRate(0), '0 B/s');
    });

    test('formatKbPair 共享单位按 total 缩放', () {
      expect(formatKbPair(5100000, 8192000), '4.9/7.8 GB');
      expect(formatKbPair(512, 1000), '512.0/1000.0 KB');
    });

    test('formatUptime 输出 d/h/m', () {
      expect(formatUptime(90061), '1d 1h 1m');
      expect(formatUptime(3661), '1h 1m');
      expect(formatUptime(61), '1m');
    });

    test('formatPercent 取整并夹取 0..100', () {
      expect(formatPercent(42.4), '42%');
      expect(formatPercent(0), '0%');
      expect(formatPercent(150), '100%');
    });
  });
}
