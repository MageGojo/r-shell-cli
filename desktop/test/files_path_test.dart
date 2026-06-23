// 远程路径拼接的纯逻辑单测（不触发 Rust / SFTP 调用）。
import 'package:flutter_test/flutter_test.dart';
import 'package:r_shell_desktop/features/files/files_controller.dart';

void main() {
  group('FilesController.remoteJoin', () {
    test('在无尾斜杠的目录后拼接', () {
      expect(FilesController.remoteJoin('/home/deploy', 'app.zip'),
          '/home/deploy/app.zip');
    });

    test('在有尾斜杠的目录后拼接（不重复斜杠）', () {
      expect(FilesController.remoteJoin('/', 'etc'), '/etc');
      expect(FilesController.remoteJoin('/var/', 'log'), '/var/log');
    });

    test('空基路径返回名称本身', () {
      expect(FilesController.remoteJoin('', 'name'), 'name');
    });

    test('拼接 .. 用于返回上级（交给服务端 realpath 规范化）', () {
      expect(FilesController.remoteJoin('/a/b', '..'), '/a/b/..');
    });

    test('Windows OpenSSH 风格的盘符路径仍用正斜杠拼接', () {
      expect(FilesController.remoteJoin('/C:/Users/shanh', 'file.txt'),
          '/C:/Users/shanh/file.txt');
    });
  });
}
