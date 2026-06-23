import 'dart:typed_data';

import '../../src/rust/api/sftp.dart' as rust;

/// Repository 层：封装 frb 的 SFTP 调用，文件管理 UI 不直接接触 frb。
///
/// 列目录走结构化 [rust.SftpEntryDto]；上传 / 下载返回 `Stream<TransferProgress>`
/// （进度 onData、失败 onError、完成 onDone）。会话由 Rust 侧按需复用，凭据不回传。
class SftpRepository {
  const SftpRepository();

  /// 列出远程目录（结构化元数据，跨平台）。
  Future<List<rust.SftpEntryDto>> list(String connectionId, String path) =>
      rust.sftpList(connectionId: connectionId, path: path);

  /// 远程默认目录（home）的绝对路径。
  Future<String> home(String connectionId) =>
      rust.sftpHome(connectionId: connectionId);

  /// 规范化远程路径（解析 `..`，用于返回上级）。
  Future<String> realpath(String connectionId, String path) =>
      rust.sftpRealpath(connectionId: connectionId, path: path);

  /// 上传本地文件到远程，进度经流推送。
  Stream<rust.TransferProgress> upload(
    String connectionId,
    String localPath,
    String remotePath,
  ) => rust.sftpUpload(
    connectionId: connectionId,
    localPath: localPath,
    remotePath: remotePath,
  );

  /// 下载远程文件到本地，进度经流推送。
  Stream<rust.TransferProgress> download(
    String connectionId,
    String remotePath,
    String localPath,
  ) => rust.sftpDownload(
    connectionId: connectionId,
    remotePath: remotePath,
    localPath: localPath,
  );

  /// 递归删除远程文件 / 目录。
  Future<void> delete(String connectionId, String path) =>
      rust.sftpDelete(connectionId: connectionId, path: path);

  /// 重命名 / 移动远程路径（绝对路径）。
  Future<void> rename(String connectionId, String from, String to) =>
      rust.sftpRename(connectionId: connectionId, from: from, to: to);

  /// 读取远程文件全部字节（打开编辑）。
  Future<Uint8List> readFile(String connectionId, String path) =>
      rust.sftpReadFile(connectionId: connectionId, path: path);

  /// 写回远程文件字节（编辑后保存）。
  Future<void> writeFile(String connectionId, String path, Uint8List data) =>
      rust.sftpWriteFile(connectionId: connectionId, path: path, data: data);
}
