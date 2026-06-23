import 'package:flutter/foundation.dart';

import '../../src/rust/api/sftp.dart' as rust;
import 'sftp_repository.dart';

enum TransferDirection { upload, download }

enum TransferState { running, completed, failed }

/// 一个传输任务的可观察状态。
class TransferItem {
  final String id;
  final TransferDirection direction;
  final String name;
  final String localPath;
  final String remotePath;
  int transferred;
  int total;
  TransferState state;
  String? error;

  TransferItem({
    required this.id,
    required this.direction,
    required this.name,
    required this.localPath,
    required this.remotePath,
    this.transferred = 0,
    this.total = 0,
    this.state = TransferState.running,
    this.error,
  });

  double get progress =>
      total <= 0 ? 0 : (transferred / total).clamp(0.0, 1.0).toDouble();
}

/// 传输队列：发起上传 / 下载，订阅进度流并更新任务状态。
///
/// 进度走 onData、失败走 onError、完成走 onDone（与 frb 流范式一致）。成功时回调
/// `onComplete`（用于刷新对侧目录）。
class TransferQueue extends ChangeNotifier {
  final SftpRepository repo;
  final List<TransferItem> items = [];
  int _seq = 0;

  TransferQueue({this.repo = const SftpRepository()});

  int get activeCount =>
      items.where((i) => i.state == TransferState.running).length;

  bool get hasFinished =>
      items.any((i) => i.state != TransferState.running);

  TransferItem upload({
    required String connectionId,
    required String localPath,
    required String remotePath,
    required String name,
    VoidCallback? onComplete,
  }) {
    final item = TransferItem(
      id: 'tx-${_seq++}',
      direction: TransferDirection.upload,
      name: name,
      localPath: localPath,
      remotePath: remotePath,
    );
    items.insert(0, item);
    notifyListeners();
    _drive(item, repo.upload(connectionId, localPath, remotePath), onComplete);
    return item;
  }

  TransferItem download({
    required String connectionId,
    required String remotePath,
    required String localPath,
    required String name,
    VoidCallback? onComplete,
  }) {
    final item = TransferItem(
      id: 'tx-${_seq++}',
      direction: TransferDirection.download,
      name: name,
      localPath: localPath,
      remotePath: remotePath,
    );
    items.insert(0, item);
    notifyListeners();
    _drive(
      item,
      repo.download(connectionId, remotePath, localPath),
      onComplete,
    );
    return item;
  }

  void _drive(
    TransferItem item,
    Stream<rust.TransferProgress> stream,
    VoidCallback? onComplete,
  ) {
    stream.listen(
      (p) {
        item.transferred = p.transferred.toInt();
        item.total = p.total.toInt();
        notifyListeners();
      },
      onError: (Object e) {
        item.state = TransferState.failed;
        item.error = '$e';
        notifyListeners();
      },
      onDone: () {
        if (item.state == TransferState.running) {
          item.state = TransferState.completed;
          if (item.total > 0) item.transferred = item.total;
          notifyListeners();
          onComplete?.call();
        }
      },
      cancelOnError: true,
    );
  }

  void clearFinished() {
    items.removeWhere((i) => i.state != TransferState.running);
    notifyListeners();
  }
}
