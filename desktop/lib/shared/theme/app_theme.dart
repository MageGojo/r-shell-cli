import 'package:flutter/material.dart';

import 'app_colors.dart';
import 'app_dimens.dart';

/// 深色「Midnight Ops」主题。组装 design tokens 为 [ThemeData]。
abstract final class AppTheme {
  static ThemeData dark() {
    final base = ThemeData.dark(useMaterial3: true);

    final scheme = base.colorScheme.copyWith(
      surface: AppColors.surface2,
      primary: AppColors.accent,
      secondary: AppColors.accentTeal,
      error: AppColors.danger,
      onPrimary: AppColors.textInverse,
      onSurface: AppColors.textPrimary,
      outline: AppColors.borderDefault,
    );

    return base.copyWith(
      colorScheme: scheme,
      scaffoldBackgroundColor: AppColors.canvas,
      canvasColor: AppColors.surface1,
      dividerColor: AppColors.borderDefault,
      splashFactory: InkSparkle.splashFactory,
      textTheme: base.textTheme.apply(
        bodyColor: AppColors.textPrimary,
        displayColor: AppColors.textPrimary,
      ),
      tooltipTheme: const TooltipThemeData(
        decoration: BoxDecoration(
          color: AppColors.surface3,
          borderRadius: BorderRadius.all(Radius.circular(AppRadius.md)),
          border: Border.fromBorderSide(
            BorderSide(color: AppColors.borderStrong),
          ),
        ),
        textStyle: TextStyle(color: AppColors.textPrimary, fontSize: 12),
        waitDuration: Duration(milliseconds: 400),
      ),
      scrollbarTheme: ScrollbarThemeData(
        thumbColor: WidgetStatePropertyAll(AppColors.borderStrong),
        thickness: const WidgetStatePropertyAll(6),
        radius: const Radius.circular(AppRadius.sm),
      ),
    );
  }
}
