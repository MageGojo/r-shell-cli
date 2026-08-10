import 'dart:io';

import 'package:flutter/foundation.dart';

/// 跨平台「登录时启动本应用」——不依赖 Xcode SPM / LaunchAtLogin。
///
/// - macOS: `~/Library/LaunchAgents/<label>.plist` + `launchctl load/unload`
/// - Windows: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
/// - Linux: `~/.config/autostart/<app>.desktop`
class AutostartService {
  static const _label = 'cn.conch.app';
  static const _windowsValueName = 'Conch';

  /// 解析 macOS `.app` 包路径(从可执行文件往上两级)。
  static String? macAppBundlePath() {
    final exe = Platform.resolvedExecutable;
    // …/Conch.app/Contents/MacOS/Conch
    final macosDir = File(exe).parent; // MacOS
    final contentsDir = macosDir.parent; // Contents
    final appDir = contentsDir.parent; // Conch.app
    if (appDir.path.endsWith('.app')) return appDir.path;
    return null;
  }

  static File get _macPlist {
    final home = Platform.environment['HOME'] ?? '';
    return File('$home/Library/LaunchAgents/$_label.plist');
  }

  static File get _linuxDesktop {
    final home = Platform.environment['HOME'] ?? '';
    return File('$home/.config/autostart/conch.desktop');
  }

  static Future<bool> isEnabled() async {
    try {
      if (Platform.isMacOS) {
        return _macPlist.existsSync();
      }
      if (Platform.isWindows) {
        final r = await Process.run('reg', [
          'query',
          r'HKCU\Software\Microsoft\Windows\CurrentVersion\Run',
          '/v',
          _windowsValueName,
        ]);
        return r.exitCode == 0;
      }
      if (Platform.isLinux) {
        return _linuxDesktop.existsSync();
      }
    } catch (e) {
      debugPrint('AutostartService.isEnabled: $e');
    }
    return false;
  }

  static Future<void> setEnabled(bool enabled) async {
    if (Platform.isMacOS) {
      await _setMac(enabled);
    } else if (Platform.isWindows) {
      await _setWindows(enabled);
    } else if (Platform.isLinux) {
      await _setLinux(enabled);
    }
  }

  static Future<void> _setMac(bool enabled) async {
    final plist = _macPlist;
    if (!enabled) {
      if (plist.existsSync()) {
        try {
          await Process.run('launchctl', [
            'bootout',
            'gui/${_uid()}',
            plist.path,
          ]);
        } catch (_) {}
        try {
          await Process.run('launchctl', ['unload', '-w', plist.path]);
        } catch (_) {}
        if (plist.existsSync()) await plist.delete();
      }
      return;
    }

    final app = macAppBundlePath();
    final String program;
    final List<String> programArgs;
    if (app != null) {
      program = '/usr/bin/open';
      programArgs = ['-a', app];
    } else {
      program = Platform.resolvedExecutable;
      programArgs = const [];
    }

    final argsXml = [
      '    <string>${_xmlEscape(program)}</string>',
      for (final a in programArgs) '    <string>${_xmlEscape(a)}</string>',
    ].join('\n');

    final body = '''
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>$_label</string>
  <key>ProgramArguments</key>
  <array>
$argsXml
  </array>
  <key>RunAtLoad</key>
  <true/>
</dict>
</plist>
''';

    await plist.parent.create(recursive: true);
    await plist.writeAsString(body.trimLeft());

    final boot = await Process.run('launchctl', [
      'bootstrap',
      'gui/${_uid()}',
      plist.path,
    ]);
    if (boot.exitCode != 0) {
      await Process.run('launchctl', ['load', '-w', plist.path]);
    }
  }

  static Future<void> _setWindows(bool enabled) async {
    final exe = Platform.resolvedExecutable;
    if (enabled) {
      await Process.run('reg', [
        'add',
        r'HKCU\Software\Microsoft\Windows\CurrentVersion\Run',
        '/v',
        _windowsValueName,
        '/t',
        'REG_SZ',
        '/d',
        '"$exe"',
        '/f',
      ]);
    } else {
      await Process.run('reg', [
        'delete',
        r'HKCU\Software\Microsoft\Windows\CurrentVersion\Run',
        '/v',
        _windowsValueName,
        '/f',
      ]);
    }
  }

  static Future<void> _setLinux(bool enabled) async {
    final desktop = _linuxDesktop;
    if (!enabled) {
      if (desktop.existsSync()) await desktop.delete();
      return;
    }
    final exe = Platform.resolvedExecutable;
    await desktop.parent.create(recursive: true);
    await desktop.writeAsString('''
[Desktop Entry]
Type=Application
Version=1.0
Name=Conch
Comment=Conch SSH workspace
Exec=${_desktopEscape(exe)}
Terminal=false
Categories=Network;Utility;
X-GNOME-Autostart-enabled=true
''');
  }

  static int _uid() {
    try {
      final r = Process.runSync('id', ['-u']);
      return int.tryParse((r.stdout as String).trim()) ?? 501;
    } catch (_) {
      return 501;
    }
  }

  static String _xmlEscape(String s) => s
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;')
      .replaceAll('"', '&quot;')
      .replaceAll("'", '&apos;');

  static String _desktopEscape(String s) =>
      s.contains(' ') ? '"${s.replaceAll('"', r'\"')}"' : s;
}
