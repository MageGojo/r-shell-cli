import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../shared/theme/app_colors.dart';
import '../../shared/theme/app_dimens.dart';
import '../../src/rust/api/adb.dart' as rust;

/// 安卓 11+「无线调试」首次连接的**配对码**弹窗。
///
/// 手机：设置 → 开发者选项 → 无线调试 → **使用配对码配对设备**，会显示一个
/// 配对地址（IP:随机端口）与 6 位配对码。在此输入后调 `adb pair` 让设备信任本机。
/// 配对成功后返回设备 IP（供「新建连接」预填，连接端口仍用 5555）。
class AdbPairDialog extends StatefulWidget {
  const AdbPairDialog({super.key});

  @override
  State<AdbPairDialog> createState() => _AdbPairDialogState();
}

class _AdbPairDialogState extends State<AdbPairDialog> {
  final _addr = TextEditingController();
  final _code = TextEditingController();
  bool _busy = false;
  String? _error;
  String? _success;

  @override
  void dispose() {
    _addr.dispose();
    _code.dispose();
    super.dispose();
  }

  Future<void> _pair() async {
    final addr = _addr.text.trim();
    final code = _code.text.trim();
    if (!addr.contains(':')) {
      setState(() => _error = '配对地址应为「IP:端口」，例如 192.168.0.101:37123');
      return;
    }
    if (code.length < 6) {
      setState(() => _error = '请输入 6 位配对码');
      return;
    }
    setState(() {
      _busy = true;
      _error = null;
      _success = null;
    });
    try {
      final msg = await rust.adbPair(hostPort: addr, code: code);
      if (!mounted) return;
      setState(() {
        _busy = false;
        _success = msg;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _busy = false;
        _error = '$e';
      });
    }
  }

  /// 从配对地址里取 IP 部分（去掉端口），用于预填连接主机。
  String get _deviceIp {
    final addr = _addr.text.trim();
    final i = addr.indexOf(':');
    return i > 0 ? addr.substring(0, i) : addr;
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: Colors.transparent,
      child: Container(
        width: 460,
        padding: const EdgeInsets.all(AppSpacing.s5),
        decoration: BoxDecoration(
          color: AppColors.surface2,
          borderRadius: BorderRadius.circular(AppRadius.xl),
          border: Border.all(color: AppColors.borderDefault),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Container(
                  width: 30,
                  height: 30,
                  decoration: BoxDecoration(
                    color: AppColors.online.withValues(alpha: 0.15),
                    borderRadius: BorderRadius.circular(AppRadius.md),
                  ),
                  child: const Icon(Icons.phonelink_ring_outlined,
                      size: 17, color: AppColors.online),
                ),
                const SizedBox(width: AppSpacing.s3),
                const Expanded(
                  child: Text(
                    '配对安卓设备（首次连接）',
                    style: TextStyle(
                      fontSize: 16,
                      fontWeight: FontWeight.w700,
                      color: AppColors.textPrimary,
                    ),
                  ),
                ),
                IconButton(
                  tooltip: '关闭',
                  icon: const Icon(Icons.close, size: 18),
                  color: AppColors.textMuted,
                  onPressed: () => Navigator.of(context).pop(),
                ),
              ],
            ),
            const SizedBox(height: AppSpacing.s3),
            _steps(),
            const SizedBox(height: AppSpacing.s4),
            _label('配对地址（IP:端口）'),
            const SizedBox(height: 6),
            _input(
              _addr,
              hint: '例如 192.168.0.101:37123',
              enabled: !_busy && _success == null,
            ),
            const SizedBox(height: AppSpacing.s4),
            _label('配对码（6 位）'),
            const SizedBox(height: 6),
            _input(
              _code,
              hint: '手机上显示的 6 位数字',
              enabled: !_busy && _success == null,
              keyboardType: TextInputType.number,
              inputFormatters: [
                FilteringTextInputFormatter.digitsOnly,
                LengthLimitingTextInputFormatter(6),
              ],
            ),
            if (_error != null) ...[
              const SizedBox(height: AppSpacing.s3),
              _banner(Icons.error_outline, AppColors.danger, _error!),
            ],
            if (_success != null) ...[
              const SizedBox(height: AppSpacing.s3),
              _banner(Icons.check_circle_outline, AppColors.online,
                  '配对成功！设备已信任本机。可用 IP $_deviceIp + 端口 5555 连接。'),
            ],
            const SizedBox(height: AppSpacing.s5),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                if (_success == null)
                  _button('取消', primary: false, onTap: () => Navigator.of(context).pop())
                else
                  _button('完成', primary: true,
                      onTap: () => Navigator.of(context).pop(_deviceIp)),
                const SizedBox(width: AppSpacing.s2),
                if (_success == null)
                  _button(
                    _busy ? '配对中…' : '配对',
                    primary: true,
                    onTap: _busy ? null : _pair,
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _steps() {
    return Container(
      padding: const EdgeInsets.all(AppSpacing.s3),
      decoration: BoxDecoration(
        color: AppColors.surface3,
        borderRadius: BorderRadius.circular(AppRadius.md),
        border: Border.all(color: AppColors.borderSubtle),
      ),
      child: const Text(
        '手机：设置 → 开发者选项 → 无线调试 → 「使用配对码配对设备」，\n'
        '把弹出的「IP:端口」和 6 位配对码填到下面。',
        style: TextStyle(fontSize: 11.5, color: AppColors.textMuted, height: 1.5),
      ),
    );
  }

  Widget _label(String text) => Text(
        text,
        style: const TextStyle(
          fontSize: 12,
          color: AppColors.textSecondary,
          fontWeight: FontWeight.w500,
        ),
      );

  Widget _input(
    TextEditingController controller, {
    String? hint,
    bool enabled = true,
    TextInputType? keyboardType,
    List<TextInputFormatter>? inputFormatters,
  }) {
    return TextField(
      controller: controller,
      enabled: enabled,
      keyboardType: keyboardType,
      inputFormatters: inputFormatters,
      style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
      cursorColor: AppColors.accent,
      decoration: InputDecoration(
        hintText: hint,
        hintStyle: const TextStyle(fontSize: 13, color: AppColors.textMuted),
        isDense: true,
        contentPadding: const EdgeInsets.symmetric(
          horizontal: AppSpacing.s3,
          vertical: AppSpacing.s3,
        ),
        filled: true,
        fillColor: AppColors.surface3,
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(AppRadius.md),
          borderSide: const BorderSide(color: AppColors.borderDefault),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(AppRadius.md),
          borderSide: const BorderSide(color: AppColors.accent),
        ),
      ),
    );
  }

  Widget _banner(IconData icon, Color color, String text) {
    return Container(
      padding: const EdgeInsets.all(AppSpacing.s3),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.10),
        borderRadius: BorderRadius.circular(AppRadius.md),
        border: Border.all(color: color.withValues(alpha: 0.35)),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, size: 15, color: color),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              text,
              style: const TextStyle(fontSize: 12, color: AppColors.textSecondary),
            ),
          ),
        ],
      ),
    );
  }

  Widget _button(String label, {required bool primary, VoidCallback? onTap}) {
    return Material(
      color: primary ? AppColors.accent : AppColors.surface3,
      borderRadius: BorderRadius.circular(AppRadius.md),
      child: InkWell(
        borderRadius: BorderRadius.circular(AppRadius.md),
        hoverColor: primary ? AppColors.accentHover : AppColors.surfaceHover,
        onTap: onTap,
        child: Container(
          height: 36,
          padding: const EdgeInsets.symmetric(horizontal: AppSpacing.s5),
          alignment: Alignment.center,
          child: Text(
            label,
            style: TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w600,
              color: primary ? AppColors.textInverse : AppColors.textSecondary,
            ),
          ),
        ),
      ),
    );
  }
}

/// 打开配对弹窗，返回配对成功后的设备 IP（取消 / 失败返回 null）。
Future<String?> showAdbPairDialog(BuildContext context) {
  return showDialog<String>(
    context: context,
    barrierColor: Colors.black.withValues(alpha: 0.55),
    builder: (_) => const AdbPairDialog(),
  );
}
