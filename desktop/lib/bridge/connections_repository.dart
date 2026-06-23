import '../src/rust/api/connections.dart';

/// Repository 层：封装 flutter_rust_bridge 的连接相关调用，UI 不直接接触 frb。
///
/// 全部写操作都委托给 `r_shell_core::connections`（与 CLI 同源），失败时底层会
/// 抛出字符串异常，由调用方捕获并提示。
class ConnectionsRepository {
  const ConnectionsRepository();

  /// 读取脱敏后的连接列表。
  List<ConnectionDto> list() => listConnections();

  /// 新建连接，返回脱敏后的结果（含新生成 id）。
  ConnectionDto create(ConnectionInput input) => createConnection(input: input);

  /// 按 id 更新连接（提交整张表单），返回脱敏后的最新值。
  ConnectionDto update(String id, ConnectionInput input) =>
      updateConnection(id: id, input: input);

  /// 按 id 删除连接。
  void delete(String id) => deleteConnection(id: id);
}
