// 连接树的纯组件冒烟测试（不触发 Rust 原生调用）。
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:r_shell_desktop/features/connections/connection_tree.dart';

void main() {
  testWidgets('ConnectionTree 在无连接时显示空状态提示', (WidgetTester tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(body: ConnectionTree(connections: [])),
      ),
    );

    expect(find.textContaining('暂无连接'), findsOneWidget);
  });
}
