import 'dart:io';
import 'dart:typed_data';

/// 一条本地文件 / 目录项。
class LocalEntry {
  final String name;
  final String path;
  final bool isDir;
  final int size;
  final DateTime? modified;

  const LocalEntry({
    required this.name,
    required this.path,
    required this.isDir,
    required this.size,
    this.modified,
  });
}

/// 本地文件系统访问（与远程连接解耦，供 SFTP 双栏的「本地」一侧使用）。
class LocalFs {
  const LocalFs();

  /// 用户主目录（跨平台），取不到时退回当前工作目录。
  String homePath() {
    final env = Platform.environment;
    return env['HOME'] ?? env['USERPROFILE'] ?? Directory.current.path;
  }

  /// 列出目录内容（目录在前，按名排序）。无法 stat 的项跳过。
  Future<List<LocalEntry>> list(String path) async {
    final dir = Directory(path);
    final entries = <LocalEntry>[];
    await for (final entity in dir.list(followLinks: false)) {
      try {
        final stat = await entity.stat();
        entries.add(
          LocalEntry(
            name: _basename(entity.path),
            path: entity.path,
            isDir: stat.type == FileSystemEntityType.directory,
            size: stat.size,
            modified: stat.modified,
          ),
        );
      } catch (_) {
        // 权限不足 / 失效链接等：跳过该项。
      }
    }
    entries.sort((a, b) {
      if (a.isDir != b.isDir) return a.isDir ? -1 : 1;
      return a.name.toLowerCase().compareTo(b.name.toLowerCase());
    });
    return entries;
  }

  /// 删除本地文件 / 目录(目录递归)。
  Future<void> delete(String path, {required bool isDir}) async {
    if (isDir) {
      await Directory(path).delete(recursive: true);
    } else {
      await File(path).delete();
    }
  }

  /// 在同目录内重命名本地文件 / 目录,返回新路径。
  Future<String> rename(String path, String newName, {required bool isDir}) async {
    final parent = parentOf(path);
    final target = join(parent, newName);
    if (isDir) {
      await Directory(path).rename(target);
    } else {
      await File(path).rename(target);
    }
    return target;
  }

  Future<Uint8List> readFile(String path) => File(path).readAsBytes();

  Future<void> writeFile(String path, Uint8List data) =>
      File(path).writeAsBytes(data, flush: true);

  /// 临时目录(供远程文件「打开编辑 / 复制」先落地用)。
  String tempDir() => Directory.systemTemp.path;

  /// 把一个本地文件放进系统剪贴板(可在访达 / 资源管理器里粘贴出文件本体)。
  /// macOS 用 osascript、Windows 用 PowerShell;Linux 无统一接口,返回 false 由调用方退化。
  Future<bool> copyFileToClipboard(String path) async {
    try {
      if (Platform.isMacOS) {
        final r = await Process.run('osascript', [
          '-e',
          'set the clipboard to (POSIX file "${path.replaceAll('"', '\\"')}")',
        ]);
        return r.exitCode == 0;
      }
      if (Platform.isWindows) {
        final r = await Process.run('powershell', [
          '-NoProfile',
          '-Command',
          "Set-Clipboard -LiteralPath '${path.replaceAll("'", "''")}'",
        ]);
        return r.exitCode == 0;
      }
      return false;
    } catch (_) {
      return false;
    }
  }

  String parentOf(String path) => Directory(path).parent.path;

  String join(String dir, String name) {
    final sep = Platform.pathSeparator;
    return dir.endsWith(sep) ? '$dir$name' : '$dir$sep$name';
  }

  String _basename(String path) {
    final normalized = path.endsWith(Platform.pathSeparator)
        ? path.substring(0, path.length - 1)
        : path;
    final idx = normalized.lastIndexOf(Platform.pathSeparator);
    return idx < 0 ? normalized : normalized.substring(idx + 1);
  }
}
