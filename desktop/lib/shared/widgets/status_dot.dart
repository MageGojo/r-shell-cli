import 'package:flutter/material.dart';

import '../theme/app_colors.dart';

/// 连接状态，映射 core 的 `ConnectionStatus`。
enum ConnStatus { connected, connecting, disconnected }

ConnStatus connStatusFrom(String label) {
  switch (label) {
    case 'Connected':
      return ConnStatus.connected;
    case 'Connecting':
      return ConnStatus.connecting;
    default:
      return ConnStatus.disconnected;
  }
}

extension ConnStatusColor on ConnStatus {
  Color get color => switch (this) {
    ConnStatus.connected => AppColors.online,
    ConnStatus.connecting => AppColors.warning,
    ConnStatus.disconnected => AppColors.offline,
  };

  String get label => switch (this) {
    ConnStatus.connected => '在线',
    ConnStatus.connecting => '连接中',
    ConnStatus.disconnected => '未连接',
  };
}

/// 8px 状态点；在线态带柔和辉光（颜色不作唯一信息载体，旁边总有文字）。
class StatusDot extends StatelessWidget {
  final ConnStatus status;
  final double size;
  const StatusDot(this.status, {super.key, this.size = 8});

  @override
  Widget build(BuildContext context) {
    final color = status.color;
    return Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        color: color,
        shape: BoxShape.circle,
        boxShadow: status == ConnStatus.connected
            ? [BoxShadow(color: color.withValues(alpha: 0.55), blurRadius: 6)]
            : null,
      ),
    );
  }
}
