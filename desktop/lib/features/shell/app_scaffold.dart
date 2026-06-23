import 'package:flutter/material.dart';

import '../../bridge/connections_repository.dart';
import '../../shared/theme/app_colors.dart';
import '../../shared/theme/app_dimens.dart';
import '../../src/rust/api/connections.dart';
import '../blockterm/block_terminal_controller.dart';
import '../blockterm/block_terminal_panel.dart';
import '../connections/connection_edit_dialog.dart';
import '../connections/connection_tree.dart';
import '../connections/connections_dashboard.dart';
import '../files/files_controller.dart';
import '../files/files_panel.dart';
import '../mcp/mcp_controller.dart';
import '../mcp/mcp_panel.dart';
import '../monitor/monitor_controller.dart';
import '../monitor/monitor_page.dart';
import '../monitor/monitor_panel.dart';
import '../settings/settings_controller.dart';
import '../settings/settings_panel.dart';

/// 主窗口三栏外壳：图标导航栏 · 连接树侧栏 · 中心工作区 · 监控侧栏 + 底部状态栏。
class AppScaffold extends StatefulWidget {
  const AppScaffold({super.key});

  @override
  State<AppScaffold> createState() => _AppScaffoldState();
}

class _AppScaffoldState extends State<AppScaffold> {
  static const _repo = ConnectionsRepository();
  final FilesController _files = FilesController();
  final MonitorController _monitor = MonitorController();
  final McpController _mcp = McpController();
  final SettingsController _settings = SettingsController();
  final BlockTerminalController _blocks = BlockTerminalController();

  /// 导航索引：0=连接，1=终端，2=文件，3=监控，4=MCP，90=设置。
  static const _navConnections = 0;
  static const _navTerminal = 1;
  static const _navFiles = 2;
  static const _navMonitor = 3;
  static const _navMcp = 4;
  static const _navSettings = 90;

  int _navIndex = 0;

  List<ConnectionDto> _connections = const [];
  String? _loadError;
  String? _selectedId;

  ConnectionDto? get _selected {
    for (final c in _connections) {
      if (c.id == _selectedId) return c;
    }
    return null;
  }

  @override
  void initState() {
    super.initState();
    _reload();
    _settings.addListener(_onSettingsChanged);
    _initSettings();
  }

  /// 读回持久化设置并应用「即时生效」项:采样间隔 → 监控;MCP 随应用自启。
  Future<void> _initSettings() async {
    await _settings.load();
    if (!mounted) return;
    _monitor.setIntervalMs(_settings.monitorIntervalMs);
    if (_settings.mcpAutoStart && !_mcp.running) {
      _mcp.start(); // fire-and-forget:失败由 MCP 面板呈现
    }
    setState(() {}); // 命令块字号读自 _settings,加载后重建一次
  }

  /// 设置变更:采样间隔实时应用;字号等经重建生效(MCP 自启仅启动时处理一次)。
  void _onSettingsChanged() {
    _monitor.setIntervalMs(_settings.monitorIntervalMs);
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    _files.dispose();
    _monitor.dispose();
    _mcp.dispose();
    _settings.removeListener(_onSettingsChanged);
    _settings.dispose();
    _blocks.dispose();
    super.dispose();
  }

  /// 让监控侧栏跟随当前选中连接(幂等:同一连接不重订阅)。
  void _syncMonitor() => _monitor.setConnection(_selected);

  void _openTerminal(ConnectionDto c) {
    setState(() {
      _selectedId = c.id;
      _navIndex = _navTerminal;
    });
    _blocks.openFor(c); // 终端板块 = 命令块,打开/聚焦该连接的标签页
    _syncMonitor();
  }

  void _openFiles(ConnectionDto c) {
    setState(() {
      _selectedId = c.id;
      _navIndex = _navFiles;
    });
    _files.setConnection(c);
    _syncMonitor();
  }

  void _openMonitor(ConnectionDto c) {
    setState(() {
      _selectedId = c.id;
      _navIndex = _navMonitor;
    });
    _syncMonitor();
  }

  void _openLocalMonitor() {
    setState(() => _navIndex = _navMonitor);
    _monitor.setLocal();
  }

  void _onNav(int index) {
    setState(() => _navIndex = index);
    if (index == _navFiles) _files.setConnection(_selected);
    if (index == _navTerminal) _syncMonitor();
    if (index == _navMonitor) _syncMonitor();
    if (index == _navMcp) _mcp.refresh();
  }

  void _onSelect(ConnectionDto c) {
    setState(() => _selectedId = c.id);
    if (_navIndex == _navFiles) _files.setConnection(c);
    _syncMonitor();
  }

  void _reload() {
    try {
      final list = _repo.list();
      setState(() {
        _connections = list;
        _loadError = null;
        if (_selectedId != null && !list.any((c) => c.id == _selectedId)) {
          _selectedId = null;
        }
      });
    } catch (e) {
      setState(() => _loadError = '$e');
    }
    _syncMonitor();
  }

  Future<void> _openEditor({ConnectionDto? existing}) async {
    final saved = await showConnectionEditDialog(
      context,
      repo: _repo,
      existing: existing,
    );
    if (saved == null) return;
    _reload();
    setState(() => _selectedId = saved.id);
    _syncMonitor();
    _toast(existing == null ? '已添加连接「${saved.name}」' : '已保存「${saved.name}」');
  }

  Future<void> _confirmDelete(ConnectionDto c) async {
    final ok = await showDialog<bool>(
      context: context,
      barrierColor: Colors.black.withValues(alpha: 0.55),
      builder: (_) => _DeleteConfirmDialog(connection: c),
    );
    if (ok != true) return;
    try {
      _repo.delete(c.id);
      _reload();
      _toast('已删除「${c.name}」');
    } catch (e) {
      _toast('删除失败：$e', error: true);
    }
  }

  void _toast(String message, {bool error = false}) {
    if (!mounted) return;
    ScaffoldMessenger.of(context)
      ..clearSnackBars()
      ..showSnackBar(
        SnackBar(
          content: Text(message),
          behavior: SnackBarBehavior.floating,
          width: 360,
          backgroundColor: error ? AppColors.danger : AppColors.surface3,
        ),
      );
  }

  /// 监控侧栏在「终端板块」(现为命令块)随手显示——终端旁边永远带服务器监控
  /// (对照主界面设计稿)。「连接」「监控」各有独立整页,不共用页面。
  bool get _showMonitor => _navIndex == _navTerminal;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Column(
        children: [
          Expanded(
            child: Row(
              children: [
                _navRail(),
                _sidebar(),
                _center(),
                // 监控侧栏隔成独立图层:它每秒刷新只重绘自己,不波及终端 / 命令块区域。
                if (_showMonitor)
                  RepaintBoundary(child: MonitorPanel(controller: _monitor)),
              ],
            ),
          ),
          _statusBar(),
        ],
      ),
    );
  }

  // ── 图标导航栏 ───────────────────────────────────────────────
  Widget _navRail() {
    const items = <(IconData, String)>[
      (Icons.dns_outlined, '连接'),
      (Icons.terminal, '终端'),
      (Icons.folder_outlined, '文件'),
      (Icons.insights_outlined, '监控'),
      (Icons.hub_outlined, 'MCP'),
    ];
    return Container(
      width: AppLayout.navRailWidth,
      color: AppColors.surface1,
      child: Column(
        children: [
          const SizedBox(height: AppSpacing.s3),
          Container(
            width: 34,
            height: 34,
            decoration: BoxDecoration(
              gradient: AppColors.accentGradient,
              borderRadius: BorderRadius.circular(AppRadius.md),
            ),
            child: const Icon(
              Icons.terminal,
              size: 19,
              color: AppColors.textInverse,
            ),
          ),
          const SizedBox(height: AppSpacing.s4),
          for (var i = 0; i < items.length; i++)
            _navButton(i, items[i].$1, items[i].$2),
          const Spacer(),
          _navButton(90, Icons.settings_outlined, '设置'),
          const SizedBox(height: AppSpacing.s3),
        ],
      ),
    );
  }

  Widget _navButton(int index, IconData icon, String tip) {
    final active = _navIndex == index;
    return Tooltip(
      message: tip,
      child: SizedBox(
        width: AppLayout.navRailWidth,
        child: Stack(
          alignment: Alignment.center,
          children: [
            if (active)
              const Positioned(
                left: 0,
                child: _ActiveBar(),
              ),
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 4),
              child: Material(
                color: active ? AppColors.surfaceActive : Colors.transparent,
                borderRadius: BorderRadius.circular(AppRadius.md),
                child: InkWell(
                  borderRadius: BorderRadius.circular(AppRadius.md),
                  hoverColor: AppColors.surfaceHover,
                  onTap: () => _onNav(index),
                  child: Padding(
                    padding: const EdgeInsets.all(9),
                    child: Icon(
                      icon,
                      size: 20,
                      color: active ? AppColors.accent : AppColors.textSecondary,
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  // ── 连接树侧栏 ───────────────────────────────────────────────
  Widget _sidebar() {
    return Container(
      width: AppLayout.sidebarWidth,
      decoration: const BoxDecoration(
        color: AppColors.surface1,
        border: Border(right: BorderSide(color: AppColors.borderSubtle)),
      ),
      child: Column(
        children: [
          _sidebarHeader(),
          const Divider(height: 1, color: AppColors.borderSubtle),
          Expanded(
            child: ConnectionTree(
              connections: _connections,
              selectedId: _selectedId,
              loadError: _loadError,
              onSelect: _onSelect,
              onOpen: _openTerminal,
              onOpenFiles: _openFiles,
              onEdit: (c) => _openEditor(existing: c),
              onDelete: _confirmDelete,
            ),
          ),
          const Divider(height: 1, color: AppColors.borderSubtle),
          _sidebarFooter(),
        ],
      ),
    );
  }

  Widget _sidebarHeader() {
    return Padding(
      padding: const EdgeInsets.fromLTRB(
        AppSpacing.s4,
        AppSpacing.s4,
        AppSpacing.s3,
        AppSpacing.s3,
      ),
      child: Row(
        children: [
          Container(
            width: 26,
            height: 26,
            decoration: BoxDecoration(
              gradient: AppColors.accentGradient,
              borderRadius: BorderRadius.circular(AppRadius.sm),
            ),
            child: const Icon(
              Icons.terminal,
              size: 15,
              color: AppColors.textInverse,
            ),
          ),
          const SizedBox(width: AppSpacing.s2),
          const Text(
            'Conch',
            style: TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w700,
              color: AppColors.textPrimary,
            ),
          ),
        ],
      ),
    );
  }

  Widget _sidebarFooter() {
    // 设置入口统一收敛到左侧导航栏底部的齿轮,这里只留「添加连接」(占满整行),
    // 避免侧边栏出现两个设置图标。
    return Padding(
      padding: const EdgeInsets.all(AppSpacing.s3),
      child: Material(
        color: AppColors.accent,
        borderRadius: BorderRadius.circular(AppRadius.md),
        child: InkWell(
          borderRadius: BorderRadius.circular(AppRadius.md),
          hoverColor: AppColors.accentHover,
          onTap: () => _openEditor(),
          child: Container(
            height: 36,
            alignment: Alignment.center,
            child: const Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Icon(Icons.add, size: 16, color: AppColors.textInverse),
                SizedBox(width: 6),
                Text(
                  '添加连接',
                  style: TextStyle(
                    fontSize: 13,
                    color: AppColors.textInverse,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  // ── 中心工作区(连接 / 终端 / 文件 / 监控 / MCP / 设置,按导航切换) ──
  Widget _center() {
    return Expanded(
      child: Container(
        decoration: const BoxDecoration(gradient: AppColors.canvasGradient),
        child: switch (_navIndex) {
          _navConnections => ConnectionsDashboard(
            connections: _connections,
            selectedId: _selectedId,
            loadError: _loadError,
            onSelect: _onSelect,
            onOpenTerminal: _openTerminal,
            onOpenFiles: _openFiles,
            onOpenMonitor: _openMonitor,
            onEdit: (c) => _openEditor(existing: c),
            onDelete: _confirmDelete,
            onAdd: () => _openEditor(),
            onOpenLocalMonitor: _openLocalMonitor,
          ),
          _navFiles => FilesPanel(controller: _files),
          _navMonitor => MonitorPage(
            controller: _monitor,
            onPickConnection: () => _onNav(_navConnections),
          ),
          _navMcp => McpPanel(controller: _mcp),
          _navSettings => SettingsPanel(controller: _settings),
          _ => BlockTerminalPanel(
            controller: _blocks,
            connections: _connections,
            fontSize: _settings.blockFontSize,
          ),
        },
      ),
    );
  }

  // ── 状态栏 ──────────────────────────────────────────────────
  Widget _statusBar() {
    final c = _selected;
    return Container(
      height: AppLayout.statusBarHeight,
      decoration: const BoxDecoration(
        color: AppColors.surface1,
        border: Border(top: BorderSide(color: AppColors.borderSubtle)),
      ),
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.s4),
      child: Row(
        children: [
          _statusItem(Icons.bolt_outlined, c == null ? '未连接' : '延迟 —'),
          const SizedBox(width: AppSpacing.s5),
          ListenableBuilder(
            listenable: _files.queue,
            builder: (context, _) {
              final active = _files.queue.activeCount;
              return _statusItem(
                Icons.folder_shared_outlined,
                active > 0 ? 'SFTP 传输中 · $active' : 'SFTP 待命',
              );
            },
          ),
          const Spacer(),
          Text(
            c == null
                ? 'Conch · v2.1.0'
                : '当前：${c.username}@${c.host}:${c.port}',
            style: const TextStyle(
              fontSize: 11,
              color: AppColors.textMuted,
              fontFamily: AppFonts.mono,
            ),
          ),
        ],
      ),
    );
  }

  Widget _statusItem(IconData icon, String text) {
    return Row(
      children: [
        Icon(icon, size: 13, color: AppColors.textMuted),
        const SizedBox(width: 5),
        Text(
          text,
          style: const TextStyle(fontSize: 11, color: AppColors.textSecondary),
        ),
      ],
    );
  }
}

class _ActiveBar extends StatelessWidget {
  const _ActiveBar();

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 2.5,
      height: 22,
      decoration: const BoxDecoration(
        color: AppColors.accent,
        borderRadius: BorderRadius.horizontal(right: Radius.circular(2)),
      ),
    );
  }
}

/// 删除连接的确认弹窗（含凭据一并移除的提醒）。
class _DeleteConfirmDialog extends StatelessWidget {
  final ConnectionDto connection;
  const _DeleteConfirmDialog({required this.connection});

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: Colors.transparent,
      child: Container(
        width: 420,
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
                    color: AppColors.danger.withValues(alpha: 0.15),
                    borderRadius: BorderRadius.circular(AppRadius.md),
                  ),
                  child: const Icon(
                    Icons.delete_outline,
                    size: 17,
                    color: AppColors.danger,
                  ),
                ),
                const SizedBox(width: AppSpacing.s3),
                const Text(
                  '删除连接',
                  style: TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.w700,
                    color: AppColors.textPrimary,
                  ),
                ),
              ],
            ),
            const SizedBox(height: AppSpacing.s4),
            Text.rich(
              TextSpan(
                style: const TextStyle(
                  fontSize: 13,
                  height: 1.5,
                  color: AppColors.textSecondary,
                ),
                children: [
                  const TextSpan(text: '确定删除 '),
                  TextSpan(
                    text: connection.name,
                    style: const TextStyle(
                      color: AppColors.textPrimary,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  const TextSpan(text: ' 吗？此操作会一并移除已保存的凭据，且不可撤销。'),
                ],
              ),
            ),
            const SizedBox(height: AppSpacing.s3),
            Container(
              padding: const EdgeInsets.symmetric(
                horizontal: AppSpacing.s3,
                vertical: AppSpacing.s2,
              ),
              decoration: BoxDecoration(
                color: AppColors.surface3,
                borderRadius: BorderRadius.circular(AppRadius.md),
              ),
              child: Text(
                '${connection.username}@${connection.host}:${connection.port}',
                style: const TextStyle(
                  fontSize: 12,
                  color: AppColors.textMuted,
                  fontFamily: AppFonts.mono,
                ),
              ),
            ),
            const SizedBox(height: AppSpacing.s5),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                _dialogButton(
                  label: '取消',
                  color: AppColors.surface3,
                  textColor: AppColors.textSecondary,
                  onTap: () => Navigator.of(context).pop(false),
                ),
                const SizedBox(width: AppSpacing.s2),
                _dialogButton(
                  label: '删除',
                  color: AppColors.danger,
                  textColor: AppColors.textPrimary,
                  onTap: () => Navigator.of(context).pop(true),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _dialogButton({
    required String label,
    required Color color,
    required Color textColor,
    required VoidCallback onTap,
  }) {
    return Material(
      color: color,
      borderRadius: BorderRadius.circular(AppRadius.md),
      child: InkWell(
        borderRadius: BorderRadius.circular(AppRadius.md),
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
              color: textColor,
            ),
          ),
        ),
      ),
    );
  }
}
