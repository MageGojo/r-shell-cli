// 间距 / 圆角 / 字体 token。详见 docs/gui/03-UI设计规范.md

abstract final class AppSpacing {
  static const s1 = 4.0;
  static const s2 = 8.0;
  static const s3 = 12.0;
  static const s4 = 16.0;
  static const s5 = 20.0;
  static const s6 = 24.0;
  static const s8 = 32.0;
  static const s10 = 40.0;
}

abstract final class AppRadius {
  static const sm = 6.0;
  static const md = 8.0;
  static const lg = 10.0;
  static const xl = 14.0;
}

abstract final class AppFonts {
  /// UI 字体：未内置 Inter 前用系统默认（置 null）。Stage 7 内置 Inter。
  static const String? ui = null;

  /// 等宽字体：'monospace' 是 Flutter 跨平台通用别名，映射系统等宽，无需内置。
  static const String mono = 'monospace';
}

abstract final class AppLayout {
  static const navRailWidth = 56.0;
  static const sidebarWidth = 264.0;
  static const monitorWidth = 304.0;
  static const tabBarHeight = 40.0;
  static const toolbarHeight = 38.0;
  static const statusBarHeight = 28.0;
}
