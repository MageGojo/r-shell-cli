import 'package:flutter/widgets.dart';

/// Midnight Ops 调色板 —— 唯一颜色来源，组件禁止写死 hex。
/// 详见 docs/gui/03-UI设计规范.md
abstract final class AppColors {
  // 背景 / 表面（由深到浅表达 elevation）
  static const canvas = Color(0xFF0B1220);
  static const surface1 = Color(0xFF0F1A30);
  static const surface2 = Color(0xFF111A2E);
  static const surface3 = Color(0xFF16213B);
  static const surfaceHover = Color(0xFF1B2742);
  static const surfaceActive = Color(0xFF22324F);

  // 边框
  static const borderSubtle = Color(0xFF1C2740);
  static const borderDefault = Color(0xFF243149);
  static const borderStrong = Color(0xFF33425F);

  // 文本
  static const textPrimary = Color(0xFFE6EDF7);
  static const textSecondary = Color(0xFF93A4C3);
  static const textMuted = Color(0xFF5D6B86);
  static const textInverse = Color(0xFF0B1220);

  // 强调
  static const accent = Color(0xFF38BDF8);
  static const accentHover = Color(0xFF5CCBFA);
  static const accentPressed = Color(0xFF0EA5E9);
  static const accentTeal = Color(0xFF22D3EE);
  static const accentIndigo = Color(0xFF818CF8);

  // 状态
  static const online = Color(0xFF34D399);
  static const warning = Color(0xFFFBBF24);
  static const danger = Color(0xFFF87171);
  static const offline = Color(0xFF5D6B86);

  static const accentGradient = LinearGradient(
    begin: Alignment.topLeft,
    end: Alignment.bottomRight,
    colors: [accentTeal, accent],
  );

  /// 应用底色微渐变（深蓝，符合「深色渐变背景」质感）。
  static const canvasGradient = LinearGradient(
    begin: Alignment.topLeft,
    end: Alignment.bottomRight,
    colors: [Color(0xFF0B1220), Color(0xFF0A1526)],
  );
}
