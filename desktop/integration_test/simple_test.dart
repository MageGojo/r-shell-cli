// 集成冒烟测试:初始化 Rust 绑定并渲染主外壳(在真机/桌面目标上运行)。
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

import 'package:r_shell_desktop/app.dart';
import 'package:r_shell_desktop/src/rust/frb_generated.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async => await RustLib.init());

  testWidgets('主外壳能渲染并显示 R-Shell 标题', (WidgetTester tester) async {
    await tester.pumpWidget(const RShellApp());
    await tester.pumpAndSettle();

    expect(find.text('R-Shell'), findsOneWidget);
  });
}
