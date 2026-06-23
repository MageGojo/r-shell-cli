import 'package:flutter/foundation.dart';

import '../../src/rust/api/connections.dart';
import '../../src/rust/api/sftp.dart' as rust;
import 'local_fs.dart';
import 'sftp_repository.dart';
import 'transfer_queue.dart';

/// SFTP 文件视图的状态：远程一侧（绑定当前连接）+ 本地一侧 + 传输队列。
///
/// 与 `BlockTerminalController` 一样由 `AppScaffold` 持有，切换视图不丢状态。
class FilesController extends ChangeNotifier {
  final SftpRepository repo;
  final LocalFs localFs;
  final TransferQueue queue;

  // ── 远程一侧 ──
  ConnectionDto? connection;
  String remotePath = '';
  List<rust.SftpEntryDto> remoteEntries = const [];
  bool remoteLoading = false;
  String? remoteError;
  String? selectedRemote; // 选中项的名称

  // ── 本地一侧 ──
  String localPath = '';
  List<LocalEntry> localEntries = const [];
  bool localLoading = false;
  String? localError;
  String? selectedLocal; // 选中项的绝对路径

  FilesController({
    this.repo = const SftpRepository(),
    this.localFs = const LocalFs(),
    TransferQueue? queue,
  }) : queue = queue ?? TransferQueue() {
    this.queue.addListener(notifyListeners);
    localPath = localFs.homePath();
    _loadLocal();
  }

  // ── 远程导航 ──────────────────────────────────────────────

  /// 绑定（或切换）当前连接，载入其远程 home 目录。重复同一连接不重载。
  Future<void> setConnection(ConnectionDto? c) async {
    if (c?.id == connection?.id) return;
    connection = c;
    remoteEntries = const [];
    remoteError = null;
    selectedRemote = null;
    remotePath = '';
    if (c == null) {
      notifyListeners();
      return;
    }
    remoteLoading = true;
    notifyListeners();
    try {
      final home = await repo.home(c.id);
      remotePath = home;
      remoteEntries = await repo.list(c.id, home);
      remoteError = null;
    } catch (e) {
      remoteError = '$e';
    } finally {
      remoteLoading = false;
      notifyListeners();
    }
  }

  Future<void> refreshRemote() async {
    if (connection != null) await _loadRemote(remotePath);
  }

  Future<void> remoteUp() async {
    final c = connection;
    if (c == null) return;
    try {
      final parent = await repo.realpath(c.id, remoteJoin(remotePath, '..'));
      await _loadRemote(parent);
    } catch (e) {
      remoteError = '$e';
      notifyListeners();
    }
  }

  void enterRemote(rust.SftpEntryDto entry) {
    if (entry.kind == 'dir' || entry.kind == 'symlink') {
      _loadRemote(remoteJoin(remotePath, entry.name));
    }
  }

  void selectRemote(String name) {
    selectedRemote = name;
    notifyListeners();
  }

  Future<void> _loadRemote(String path) async {
    final c = connection;
    if (c == null) return;
    remoteLoading = true;
    remoteError = null;
    notifyListeners();
    try {
      final entries = await repo.list(c.id, path);
      remotePath = path;
      remoteEntries = entries;
      selectedRemote = null;
    } catch (e) {
      remoteError = '$e';
    } finally {
      remoteLoading = false;
      notifyListeners();
    }
  }

  // ── 本地导航 ──────────────────────────────────────────────

  Future<void> refreshLocal() => _loadLocal();

  Future<void> navigateLocal(String path) async {
    localPath = path;
    selectedLocal = null;
    await _loadLocal();
  }

  Future<void> localUp() => navigateLocal(localFs.parentOf(localPath));

  void enterLocal(LocalEntry entry) {
    if (entry.isDir) navigateLocal(entry.path);
  }

  void selectLocal(String path) {
    selectedLocal = path;
    notifyListeners();
  }

  Future<void> _loadLocal() async {
    localLoading = true;
    localError = null;
    notifyListeners();
    try {
      localEntries = await localFs.list(localPath);
    } catch (e) {
      localError = '$e';
    } finally {
      localLoading = false;
      notifyListeners();
    }
  }

  // ── 传输 ──────────────────────────────────────────────────

  /// 上传一个本地项（目录暂不支持）。
  void uploadLocalEntry(LocalEntry entry) {
    final c = connection;
    if (c == null || entry.isDir) return;
    queue.upload(
      connectionId: c.id,
      localPath: entry.path,
      remotePath: remoteJoin(remotePath, entry.name),
      name: entry.name,
      onComplete: refreshRemote,
    );
  }

  /// 上传任意本地路径（拖拽进来的文件）。
  void uploadFile({required String localFilePath, required String name}) {
    final c = connection;
    if (c == null) return;
    queue.upload(
      connectionId: c.id,
      localPath: localFilePath,
      remotePath: remoteJoin(remotePath, name),
      name: name,
      onComplete: refreshRemote,
    );
  }

  /// 下载一个远程项（目录暂不支持）。
  void downloadRemoteEntry(rust.SftpEntryDto entry) {
    final c = connection;
    if (c == null || entry.kind == 'dir') return;
    queue.download(
      connectionId: c.id,
      remotePath: remoteJoin(remotePath, entry.name),
      localPath: localFs.join(localPath, entry.name),
      name: entry.name,
      onComplete: refreshLocal,
    );
  }

  // ── 远程文件操作（删除 / 重命名 / 编辑 / 复制）──────────────
  // 约定：动作方法返回 `String?`，null 表示成功，否则为错误信息（UI 弹 SnackBar）。

  Future<String?> deleteRemote(rust.SftpEntryDto e) async {
    final c = connection;
    if (c == null) return '未连接';
    try {
      await repo.delete(c.id, remoteJoin(remotePath, e.name));
      if (selectedRemote == e.name) selectedRemote = null;
      await refreshRemote();
      return null;
    } catch (err) {
      return '$err';
    }
  }

  Future<String?> renameRemote(rust.SftpEntryDto e, String newName) async {
    final c = connection;
    if (c == null) return '未连接';
    final nn = newName.trim();
    if (nn.isEmpty || nn == e.name) return null;
    try {
      await repo.rename(
        c.id,
        remoteJoin(remotePath, e.name),
        remoteJoin(remotePath, nn),
      );
      await refreshRemote();
      return null;
    } catch (err) {
      return '$err';
    }
  }

  Future<Uint8List> readRemoteBytes(rust.SftpEntryDto e) =>
      repo.readFile(connection!.id, remoteJoin(remotePath, e.name));

  Future<String?> writeRemoteBytes(String name, Uint8List data) async {
    final c = connection;
    if (c == null) return '未连接';
    try {
      await repo.writeFile(c.id, remoteJoin(remotePath, name), data);
      await refreshRemote();
      return null;
    } catch (err) {
      return '$err';
    }
  }

  /// 复制远程文件本体到系统剪贴板：先下到临时文件，再放剪贴板。
  Future<String?> copyRemoteToClipboard(rust.SftpEntryDto e) async {
    final c = connection;
    if (c == null) return '未连接';
    if (e.kind == 'dir') return '暂不支持复制目录';
    try {
      final bytes = await repo.readFile(c.id, remoteJoin(remotePath, e.name));
      final tmp = localFs.join(localFs.tempDir(), e.name);
      await localFs.writeFile(tmp, bytes);
      final ok = await localFs.copyFileToClipboard(tmp);
      return ok ? null : '本系统暂不支持复制文件到剪贴板';
    } catch (err) {
      return '$err';
    }
  }

  // ── 本地文件操作 ──────────────────────────────────────────

  Future<String?> deleteLocal(LocalEntry e) async {
    try {
      await localFs.delete(e.path, isDir: e.isDir);
      if (selectedLocal == e.path) selectedLocal = null;
      await refreshLocal();
      return null;
    } catch (err) {
      return '$err';
    }
  }

  Future<String?> renameLocal(LocalEntry e, String newName) async {
    final nn = newName.trim();
    if (nn.isEmpty || nn == e.name) return null;
    try {
      await localFs.rename(e.path, nn, isDir: e.isDir);
      await refreshLocal();
      return null;
    } catch (err) {
      return '$err';
    }
  }

  Future<Uint8List> readLocalBytes(LocalEntry e) => localFs.readFile(e.path);

  Future<String?> writeLocalBytes(String path, Uint8List data) async {
    try {
      await localFs.writeFile(path, data);
      await refreshLocal();
      return null;
    } catch (err) {
      return '$err';
    }
  }

  Future<String?> copyLocalToClipboard(LocalEntry e) async {
    if (e.isDir) return '暂不支持复制目录';
    try {
      final ok = await localFs.copyFileToClipboard(e.path);
      return ok ? null : '本系统暂不支持复制文件到剪贴板';
    } catch (err) {
      return '$err';
    }
  }

  bool get canUploadSelection {
    final sel = selectedLocal;
    if (connection == null || sel == null) return false;
    final entry = _localByPath(sel);
    return entry != null && !entry.isDir;
  }

  bool get canDownloadSelection {
    final sel = selectedRemote;
    if (connection == null || sel == null) return false;
    final entry = _remoteByName(sel);
    return entry != null && entry.kind != 'dir';
  }

  void uploadSelected() {
    final sel = selectedLocal;
    if (sel == null) return;
    final entry = _localByPath(sel);
    if (entry != null) uploadLocalEntry(entry);
  }

  void downloadSelected() {
    final sel = selectedRemote;
    if (sel == null) return;
    final entry = _remoteByName(sel);
    if (entry != null) downloadRemoteEntry(entry);
  }

  LocalEntry? _localByPath(String path) {
    for (final e in localEntries) {
      if (e.path == path) return e;
    }
    return null;
  }

  rust.SftpEntryDto? _remoteByName(String name) {
    for (final e in remoteEntries) {
      if (e.name == name) return e;
    }
    return null;
  }

  /// 拼接远程路径（POSIX 风格，SFTP 服务端对 Windows 同样接受 `/`）。
  static String remoteJoin(String base, String name) {
    if (base.isEmpty) return name;
    return base.endsWith('/') ? '$base$name' : '$base/$name';
  }

  @override
  void dispose() {
    queue.removeListener(notifyListeners);
    queue.dispose();
    super.dispose();
  }
}
