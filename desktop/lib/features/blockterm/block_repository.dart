import '../../src/rust/api/blockterm.dart' as rust;

/// Repository 层:封装 frb 的命令块执行调用,UI / 控制器不直接接触 frb。
class BlockRepository {
  const BlockRepository();

  /// 在某已保存连接(SSH / ADB)上执行一条命令块。
  Future<rust.BlockResultDto> runRemote(
    String connectionId,
    String command,
    String cwd,
  ) => rust.blockRunRemote(
    connectionId: connectionId,
    command: command,
    cwd: cwd,
  );

  /// 在本机(运行 GUI 的电脑)执行一条命令块。
  Future<rust.BlockResultDto> runLocal(String command, String cwd) =>
      rust.blockRunLocal(command: command, cwd: cwd);
}
