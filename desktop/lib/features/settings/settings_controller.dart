import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'autostart_service.dart';

/// 主题模式(深色已落地;浅色 / 跟随系统的实时换肤需整套取色重构,偏好先持久化)。
enum AppThemeMode { dark, system }

/// 设置页状态:进程内承接 + `shared_preferences` 持久化(重启保留)。
///
/// 真正生效的项:命令块字号(接命令块面板)、监控采样间隔(接 `MonitorController`)、
/// MCP 随应用自启(启动时调 `McpController.start`)、开机自启(Login Item / 注册表)。
/// 主题/强调色暂只持久化偏好(实时换肤待 `AppColors` 改为可切换取色后接入)。
class SettingsController extends ChangeNotifier {
  static const _kTheme = 'settings.theme_mode';
  static const _kFont = 'settings.block_font_size';
  static const _kInterval = 'settings.monitor_interval_ms';
  static const _kMcpAuto = 'settings.mcp_auto_start';
  static const _kLaunchAtLogin = 'settings.launch_at_login';

  AppThemeMode _themeMode = AppThemeMode.dark;
  double _blockFontSize = 14;
  int _monitorIntervalMs = 1000;
  bool _mcpAutoStart = true;
  bool _launchAtLogin = false;
  String? _autostartError;

  SharedPreferences? _prefs;
  bool _loaded = false;
  bool get loaded => _loaded;

  AppThemeMode get themeMode => _themeMode;
  double get blockFontSize => _blockFontSize;
  int get monitorIntervalMs => _monitorIntervalMs;
  bool get mcpAutoStart => _mcpAutoStart;
  bool get launchAtLogin => _launchAtLogin;
  String? get autostartError => _autostartError;

  /// 启动时调用:从磁盘读回设置(失败则保持默认),完成后通知一次。
  Future<void> load() async {
    try {
      final p = await SharedPreferences.getInstance();
      _prefs = p;
      final t = p.getString(_kTheme);
      if (t != null) {
        _themeMode = AppThemeMode.values.firstWhere(
          (m) => m.name == t,
          orElse: () => AppThemeMode.dark,
        );
      }
      _blockFontSize =
          (p.getDouble(_kFont) ?? _blockFontSize).clamp(11, 22).toDouble();
      _monitorIntervalMs = p.getInt(_kInterval) ?? _monitorIntervalMs;
      _mcpAutoStart = p.getBool(_kMcpAuto) ?? _mcpAutoStart;
      _launchAtLogin = p.getBool(_kLaunchAtLogin) ?? false;
      // 与系统真实状态对齐(用户可能在系统设置里关过)。
      try {
        final systemOn = await AutostartService.isEnabled();
        if (systemOn != _launchAtLogin) {
          _launchAtLogin = systemOn;
          await p.setBool(_kLaunchAtLogin, systemOn);
        }
      } catch (_) {}
    } catch (_) {
      // 读取失败(如平台无实现)时保持默认值,不阻塞 UI。
    }
    _loaded = true;
    notifyListeners();
  }

  void setThemeMode(AppThemeMode value) {
    if (_themeMode == value) return;
    _themeMode = value;
    _prefs?.setString(_kTheme, value.name);
    notifyListeners();
  }

  void setBlockFontSize(double value) {
    final clamped = value.clamp(11, 22).toDouble();
    if (_blockFontSize == clamped) return;
    _blockFontSize = clamped;
    _prefs?.setDouble(_kFont, clamped);
    notifyListeners();
  }

  void setMonitorIntervalMs(int value) {
    if (_monitorIntervalMs == value) return;
    _monitorIntervalMs = value;
    _prefs?.setInt(_kInterval, value);
    notifyListeners();
  }

  void setMcpAutoStart(bool value) {
    if (_mcpAutoStart == value) return;
    _mcpAutoStart = value;
    _prefs?.setBool(_kMcpAuto, value);
    notifyListeners();
  }

  /// 开机自启:写入系统 Login Item / 注册表,并默认连带打开 MCP 随应用自启。
  Future<void> setLaunchAtLogin(bool value) async {
    _autostartError = null;
    notifyListeners();
    try {
      await AutostartService.setEnabled(value);
      _launchAtLogin = value;
      await _prefs?.setBool(_kLaunchAtLogin, value);
      if (value && !_mcpAutoStart) {
        _mcpAutoStart = true;
        await _prefs?.setBool(_kMcpAuto, true);
      }
    } catch (e) {
      _autostartError = '开机自启设置失败:$e';
    }
    notifyListeners();
  }
}
