import 'dart:ui';

import 'package:flutter/material.dart';

import '../theme/app_colors.dart';
import '../theme/app_dimens.dart';

/// 液态玻璃容器 —— 项目默认质感，用于顶栏 / 状态栏 / 悬浮层等点缀处。
///
/// 运维主面板（终端 / 表格 / 连接树）为信息密集区，仍用实色高对比表面；
/// 玻璃仅用于 chrome / overlay，兼顾质感与可读性。
class GlassPanel extends StatelessWidget {
  final Widget child;
  final double blur;
  final double radius;
  final Color? tint;
  final double opacity;
  final EdgeInsetsGeometry? padding;
  final BoxBorder? border;

  const GlassPanel({
    super.key,
    required this.child,
    this.blur = 18,
    this.radius = AppRadius.lg,
    this.tint,
    this.opacity = 0.55,
    this.padding,
    this.border,
  });

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(radius),
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: blur, sigmaY: blur),
        child: Container(
          padding: padding,
          decoration: BoxDecoration(
            color: (tint ?? AppColors.surface3).withValues(alpha: opacity),
            borderRadius: BorderRadius.circular(radius),
            border:
                border ??
                Border.all(
                  color: AppColors.textPrimary.withValues(alpha: 0.06),
                  width: 0.5,
                ),
          ),
          child: child,
        ),
      ),
    );
  }
}
