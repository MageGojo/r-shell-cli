import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../bridge/connections_repository.dart';
import '../../shared/theme/app_colors.dart';
import '../../shared/theme/app_dimens.dart';
import '../../src/rust/api/connections.dart';
import 'adb_pair_dialog.dart';

/// 打开「连接编辑」弹窗。`existing == null` 为新建，否则为编辑。
///
/// 返回保存后的脱敏连接（新建时含新 id），取消则返回 `null`。
Future<ConnectionDto?> showConnectionEditDialog(
  BuildContext context, {
  required ConnectionsRepository repo,
  ConnectionDto? existing,
}) {
  return showDialog<ConnectionDto>(
    context: context,
    barrierColor: Colors.black.withValues(alpha: 0.55),
    builder: (_) => ConnectionEditDialog(repo: repo, existing: existing),
  );
}

/// 连接编辑弹窗：页签 常规 / SSH / 高级 / 备注，对接 core 的连接 CRUD。
class ConnectionEditDialog extends StatefulWidget {
  final ConnectionsRepository repo;
  final ConnectionDto? existing;
  const ConnectionEditDialog({super.key, required this.repo, this.existing});

  @override
  State<ConnectionEditDialog> createState() => _ConnectionEditDialogState();
}

class _ConnectionEditDialogState extends State<ConnectionEditDialog>
    with SingleTickerProviderStateMixin {
  late final TabController _tabs = TabController(length: 4, vsync: this);

  late final TextEditingController _name;
  late final TextEditingController _folder;
  late final TextEditingController _host;
  late final TextEditingController _port;
  late final TextEditingController _user;
  late final TextEditingController _password;
  late final TextEditingController _keyPath;
  late final TextEditingController _passphrase;
  late final TextEditingController _tagsCtrl;
  late final TextEditingController _desc;

  /// UI 协议:SSH | ADB | iOS。iOS 落盘仍为 SSH + `platform:ios` 标签。
  String _protocol = 'SSH';
  String _auth = 'password';
  /// 提权方式:`none` / `su` / `sudo`(写入 `elevate:…` 标签)。
  String _elevate = 'none';
  bool _clearPassword = false;
  bool _clearKeyPath = false;
  bool _clearPassphrase = false;
  String? _error;

  bool get _isEdit => widget.existing != null;
  bool get _isIos => _protocol == 'iOS';
  bool get _isAdb => _protocol == 'ADB';

  static const _kIosDefaultUser = 'root';
  static const _kIosDefaultPass = 'alpine';
  static const _kIosDefaultPort = '22';
  static const _kIosDefaultName = 'iPhone';

  @override
  void initState() {
    super.initState();
    final c = widget.existing;
    final tags = c?.tags ?? const <String>[];
    final taggedIos = tags.any(
      (t) =>
          t.trim().toLowerCase() == 'platform:ios' ||
          t.trim().toLowerCase() == 'ios',
    );
    if (c?.protocol == 'ADB') {
      _protocol = 'ADB';
    } else if (taggedIos) {
      _protocol = 'iOS';
    } else {
      _protocol = 'SSH';
    }
    _name = TextEditingController(text: c?.name ?? '');
    _folder = TextEditingController(text: c?.folder ?? '');
    _host = TextEditingController(text: c?.host ?? '');
    _port = TextEditingController(
      text: (c?.port ??
              (_protocol == 'ADB'
                  ? 5555
                  : 22))
          .toString(),
    );
    _user = TextEditingController(
      text: c?.username ?? (_protocol == 'iOS' ? _kIosDefaultUser : ''),
    );
    // 新建 iOS:预填越狱默认密码 alpine;编辑时留空=保持已存密码。
    _password = TextEditingController(
      text: (!_isEdit && _protocol == 'iOS') ? _kIosDefaultPass : '',
    );
    _keyPath = TextEditingController();
    _passphrase = TextEditingController();
    _elevate = 'none';
    for (final t in tags) {
      final lower = t.trim().toLowerCase();
      if (lower.startsWith('elevate:')) {
        final v = lower.substring('elevate:'.length);
        if (v == 'su' || v == 'sudo') _elevate = v;
      } else if (lower == 'elevate') {
        _elevate = 'su';
      }
    }
    final visibleTags = tags
        .where((t) {
          final lower = t.trim().toLowerCase();
          return lower != 'ios' &&
              lower != 'platform:ios' &&
              lower != 'elevate' &&
              !lower.startsWith('elevate:');
        })
        .toList();
    _tagsCtrl = TextEditingController(text: visibleTags.join(', '));
    _desc = TextEditingController(text: c?.description ?? '');
    _auth = c?.authMethod == 'publickey' && !_isIos ? 'publickey' : 'password';
  }

  /// 合并用户填写的标签 + iOS/提权控制标签。
  List<String> _buildTags() {
    final tags = _parseTags(_tagsCtrl.text);
    tags.removeWhere((t) {
      final lower = t.toLowerCase();
      return lower == 'ios' ||
          lower == 'platform:ios' ||
          lower == 'elevate' ||
          lower.startsWith('elevate:');
    });
    if (_isIos) {
      tags.add('platform:ios');
    }
    if ((_isIos || _protocol == 'SSH') && _elevate != 'none') {
      tags.add('elevate:$_elevate');
    }
    return tags;
  }

  @override
  void dispose() {
    _tabs.dispose();
    for (final ctrl in [
      _name,
      _folder,
      _host,
      _port,
      _user,
      _password,
      _keyPath,
      _passphrase,
      _tagsCtrl,
      _desc,
    ]) {
      ctrl.dispose();
    }
    super.dispose();
  }

  List<String> _parseTags(String raw) => raw
      .split(',')
      .map((t) => t.trim())
      .where((t) => t.isNotEmpty)
      .toList();

  /// 弹原生文件选择器挑私钥文件，选中即填入路径并取消「清除」。
  Future<void> _pickKeyFile() async {
    // 本应用在 macOS 关闭了沙箱（见 Stage 3.5），跳过 file_picker 的 entitlements 检查。
    await FilePicker.skipEntitlementsChecks();
    final result = await FilePicker.pickFiles(dialogTitle: '选择 SSH 私钥文件');
    final path = result?.files.single.path;
    if (path != null && mounted) {
      setState(() {
        _keyPath.text = path;
        _clearKeyPath = false;
      });
    }
  }

  /// 计算密钥类字段要提交的值（见 ConnectionInput 文档的三态语义）。
  String? _secretValue(
    TextEditingController ctrl, {
    required bool clear,
    required bool hadExisting,
  }) {
    if (!_isEdit) return ctrl.text.isEmpty ? null : ctrl.text;
    if (clear) return '';
    if (ctrl.text.isNotEmpty) return ctrl.text;
    return null; // 留空：保持原值不变
  }

  void _save() {
    final host = _host.text.trim();
    var name = _name.text.trim();
    var user = _user.text.trim();
    var port = int.tryParse(_port.text.trim());

    // iOS:用户通常只填主机;名称/账号/密码/端口用越狱默认值兜底。
    if (_isIos) {
      if (host.isEmpty) {
        _fail('请填写 iPhone 的 IP 或域名', tab: 1);
        return;
      }
      if (name.isEmpty) name = host;
      if (user.isEmpty) user = _kIosDefaultUser;
      port ??= int.parse(_kIosDefaultPort);
      _port.text = port.toString();
      _user.text = user;
      if (name != _name.text.trim()) _name.text = name;
    }

    if (!_isIos && name.isEmpty) {
      _fail('请填写连接名称', tab: 0);
      return;
    }
    if (port == null || port < 1 || port > 65535) {
      _fail('端口需为 1–65535 之间的数字', tab: 1);
      return;
    }

    // ADB：仅需设备地址 + 端口，无用户名 / 认证概念。
    if (_isAdb) {
      if (host.isEmpty) {
        _fail('请填写安卓设备地址', tab: 1);
        return;
      }
      _commit(ConnectionInput(
        protocol: 'ADB',
        name: name,
        host: host,
        username: user,
        port: port,
        authMethod: 'password',
        folder: _folder.text.trim(),
        description: _desc.text,
        tags: _parseTags(_tagsCtrl.text),
      ));
      return;
    }

    if (host.isEmpty || user.isEmpty) {
      _fail('请填写主机和用户名', tab: 1);
      return;
    }

    final usePassword = _isIos || _auth == 'password';
    String? password;
    if (usePassword) {
      if (_isIos && !_isEdit && _password.text.isEmpty) {
        password = _kIosDefaultPass;
      } else if (_isIos && _isEdit && _password.text.isEmpty && !_clearPassword) {
        // 编辑且未改密码:保持原值(None)。
        password = null;
      } else {
        password = _secretValue(
          _password,
          clear: _clearPassword,
          hadExisting: widget.existing?.hasPassword ?? false,
        );
        // 新建 iOS 若用户清空密码字段,仍回落 alpine。
        if (_isIos && !_isEdit && (password == null || password.isEmpty)) {
          password = _kIosDefaultPass;
        }
      }
    }

    _commit(ConnectionInput(
      protocol: 'SSH',
      name: name,
      host: host,
      username: user,
      port: port,
      authMethod: usePassword ? 'password' : _auth,
      folder: _folder.text.trim(),
      description: _desc.text,
      tags: _buildTags(),
      password: usePassword ? password : null,
      privateKeyPath: usePassword
          ? null
          : _secretValue(
              _keyPath,
              clear: _clearKeyPath,
              hadExisting: widget.existing?.hasPrivateKeyPath ?? false,
            ),
      passphrase: usePassword
          ? null
          : _secretValue(
              _passphrase,
              clear: _clearPassphrase,
              hadExisting: widget.existing?.hasPassphrase ?? false,
            ),
    ));
  }

  void _commit(ConnectionInput input) {
    try {
      final saved = _isEdit
          ? widget.repo.update(widget.existing!.id, input)
          : widget.repo.create(input);
      if (mounted) Navigator.of(context).pop(saved);
    } catch (e) {
      _fail('保存失败：$e');
    }
  }

  void _fail(String message, {int? tab}) {
    setState(() => _error = message);
    if (tab != null) _tabs.animateTo(tab);
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: Colors.transparent,
      insetPadding: const EdgeInsets.all(AppSpacing.s6),
      child: Container(
        width: 580,
        decoration: BoxDecoration(
          color: AppColors.surface2,
          borderRadius: BorderRadius.circular(AppRadius.xl),
          border: Border.all(color: AppColors.borderDefault),
          boxShadow: [
            BoxShadow(
              color: Colors.black.withValues(alpha: 0.4),
              blurRadius: 40,
              spreadRadius: -8,
              offset: const Offset(0, 18),
            ),
          ],
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            _header(),
            const Divider(height: 1, color: AppColors.borderSubtle),
            _tabBar(),
            const Divider(height: 1, color: AppColors.borderSubtle),
            SizedBox(
              height: 400,
              child: TabBarView(
                controller: _tabs,
                children: [
                  _generalTab(),
                  _sshTab(),
                  _advancedTab(),
                  _notesTab(),
                ],
              ),
            ),
            const Divider(height: 1, color: AppColors.borderSubtle),
            _footer(),
          ],
        ),
      ),
    );
  }

  Widget _header() {
    return Padding(
      padding: const EdgeInsets.fromLTRB(
        AppSpacing.s5,
        AppSpacing.s4,
        AppSpacing.s3,
        AppSpacing.s4,
      ),
      child: Row(
        children: [
          Container(
            width: 30,
            height: 30,
            decoration: BoxDecoration(
              gradient: AppColors.accentGradient,
              borderRadius: BorderRadius.circular(AppRadius.md),
            ),
            child: Icon(
              _isEdit ? Icons.edit_outlined : Icons.add_link,
              size: 17,
              color: AppColors.textInverse,
            ),
          ),
          const SizedBox(width: AppSpacing.s3),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  _isEdit ? '编辑连接' : '新建连接',
                  style: const TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.w700,
                    color: AppColors.textPrimary,
                  ),
                ),
                Text(
                  _isEdit
                      ? '修改后保存即写入 workspace.json'
                      : '填写连接信息（SSH / 安卓 ADB / iOS 越狱），保存后出现在连接树',
                  style: const TextStyle(
                    fontSize: 11.5,
                    color: AppColors.textMuted,
                  ),
                ),
              ],
            ),
          ),
          IconButton(
            tooltip: '关闭',
            icon: const Icon(Icons.close, size: 18),
            color: AppColors.textMuted,
            onPressed: () => Navigator.of(context).pop(),
          ),
        ],
      ),
    );
  }

  Widget _tabBar() {
    return TabBar(
      controller: _tabs,
      isScrollable: true,
      tabAlignment: TabAlignment.start,
      labelColor: AppColors.accent,
      unselectedLabelColor: AppColors.textSecondary,
      indicatorColor: AppColors.accent,
      indicatorWeight: 2,
      labelStyle: const TextStyle(fontSize: 13, fontWeight: FontWeight.w600),
      unselectedLabelStyle: const TextStyle(fontSize: 13),
      overlayColor: const WidgetStatePropertyAll(Colors.transparent),
      tabs: [
        const Tab(height: 40, text: '常规'),
        Tab(
          height: 40,
          text: _isAdb
              ? 'ADB'
              : _isIos
                  ? 'iOS'
                  : 'SSH',
        ),
        const Tab(height: 40, text: '高级'),
        const Tab(height: 40, text: '备注'),
      ],
    );
  }

  Widget _tabBody(List<Widget> children) {
    return SingleChildScrollView(
      padding: const EdgeInsets.fromLTRB(
        AppSpacing.s5,
        AppSpacing.s5,
        AppSpacing.s5,
        AppSpacing.s5,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: children,
      ),
    );
  }

  Widget _generalTab() {
    return _tabBody([
      _labelText('协议'),
      const SizedBox(height: 6),
      _protocolSelector(),
      const SizedBox(height: AppSpacing.s4),
      _field(
        label: '连接名称',
        controller: _name,
        hint: _isAdb
            ? '例如：我的安卓'
            : _isIos
                ? '留空则用主机名 / IP'
                : '例如：生产 Web 服务器',
      ),
      if (_isIos) ...[
        const SizedBox(height: AppSpacing.s2),
        const Text(
          'iOS 越狱默认 root / alpine / 22，一般只需在「iOS」页填主机。',
          style: TextStyle(fontSize: 11.5, color: AppColors.textMuted),
        ),
      ],
      const SizedBox(height: AppSpacing.s4),
      _field(
        label: '分组',
        controller: _folder,
        hint: '留空归入「All Connections」',
      ),
    ]);
  }

  Widget _sshTab() {
    if (_isAdb) {
      return _tabBody([
        _field(
          label: '设备地址',
          controller: _host,
          hint: '安卓设备 IP，例如 192.168.0.101',
        ),
        const SizedBox(height: AppSpacing.s4),
        _field(
          label: '端口',
          controller: _port,
          hint: '5555',
          keyboardType: TextInputType.number,
          inputFormatters: [FilteringTextInputFormatter.digitsOnly],
        ),
        const SizedBox(height: AppSpacing.s4),
        _pairRow(),
        const SizedBox(height: AppSpacing.s4),
        _adbHint(),
      ]);
    }

    if (_isIos) {
      return _tabBody([
        _field(
          label: '主机（必填）',
          controller: _host,
          hint: '例如 192.168.0.103',
        ),
        const SizedBox(height: AppSpacing.s3),
        const Text(
          '下方账号密码端口已预填越狱 OpenSSH 默认值，通常不用改。',
          style: TextStyle(fontSize: 11.5, color: AppColors.textMuted),
        ),
        const SizedBox(height: AppSpacing.s4),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              flex: 2,
              child: _field(
                label: '用户名',
                controller: _user,
                hint: _kIosDefaultUser,
              ),
            ),
            const SizedBox(width: AppSpacing.s3),
            Expanded(
              child: _field(
                label: '端口',
                controller: _port,
                hint: _kIosDefaultPort,
                keyboardType: TextInputType.number,
                inputFormatters: [FilteringTextInputFormatter.digitsOnly],
              ),
            ),
          ],
        ),
        const SizedBox(height: AppSpacing.s3),
        _iosUserChips(),
        const SizedBox(height: AppSpacing.s4),
        _secretField(
          label: '密码',
          controller: _password,
          clear: _clearPassword,
          hadExisting: widget.existing?.hasPassword ?? false,
          onClearChanged: (v) => setState(() => _clearPassword = v),
          obscure: true,
          hint: _kIosDefaultPass,
        ),
      ]);
    }

    return _tabBody([
      _field(label: '主机', controller: _host, hint: 'IP 或域名'),
      const SizedBox(height: AppSpacing.s4),
      Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            flex: 2,
            child: _field(label: '用户名', controller: _user, hint: 'root'),
          ),
          const SizedBox(width: AppSpacing.s3),
          Expanded(
            child: _field(
              label: '端口',
              controller: _port,
              hint: '22',
              keyboardType: TextInputType.number,
              inputFormatters: [FilteringTextInputFormatter.digitsOnly],
            ),
          ),
        ],
      ),
      const SizedBox(height: AppSpacing.s4),
      _labelText('认证方式'),
      const SizedBox(height: 6),
      _authSelector(),
      const SizedBox(height: AppSpacing.s4),
      if (_auth == 'password')
        _secretField(
          label: '密码',
          controller: _password,
          clear: _clearPassword,
          hadExisting: widget.existing?.hasPassword ?? false,
          onClearChanged: (v) => setState(() => _clearPassword = v),
          obscure: true,
        )
      else ...[
        _secretField(
          label: '私钥文件',
          controller: _keyPath,
          clear: _clearKeyPath,
          hadExisting: widget.existing?.hasPrivateKeyPath ?? false,
          onClearChanged: (v) => setState(() => _clearKeyPath = v),
          obscure: false,
          hint: '~/.ssh/id_ed25519',
          trailing: _browseButton(enabled: !_clearKeyPath),
        ),
        const SizedBox(height: AppSpacing.s4),
        _secretField(
          label: '私钥口令（passphrase，没有可留空）',
          controller: _passphrase,
          clear: _clearPassphrase,
          hadExisting: widget.existing?.hasPassphrase ?? false,
          onClearChanged: (v) => setState(() => _clearPassphrase = v),
          obscure: true,
        ),
      ],
    ]);
  }

  Widget _iosUserChips() {
    Widget chip(String user) {
      final selected = _user.text.trim() == user;
      return Material(
        color: selected ? AppColors.surfaceActive : AppColors.surface3,
        borderRadius: BorderRadius.circular(AppRadius.md),
        child: InkWell(
          borderRadius: BorderRadius.circular(AppRadius.md),
          onTap: () => setState(() {
            _user.text = user;
            if (user == 'mobile' && _elevate == 'none') {
              _elevate = 'su';
            } else if (user == 'root') {
              _elevate = 'none';
            }
          }),
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(AppRadius.md),
              border: Border.all(
                color: selected ? AppColors.accent : AppColors.borderDefault,
              ),
            ),
            child: Text(
              user,
              style: TextStyle(
                fontSize: 12,
                fontFamily: AppFonts.mono,
                color: selected ? AppColors.accent : AppColors.textSecondary,
              ),
            ),
          ),
        ),
      );
    }

    return Row(
      children: [
        const Text(
          '快捷用户',
          style: TextStyle(fontSize: 12, color: AppColors.textMuted),
        ),
        const SizedBox(width: AppSpacing.s2),
        chip('root'),
        const SizedBox(width: 6),
        chip('mobile'),
      ],
    );
  }

  /// 私钥「选择文件」按钮：唤起原生文件选择器。
  Widget _browseButton({required bool enabled}) {
    return Material(
      color: AppColors.surface3,
      borderRadius: BorderRadius.circular(AppRadius.md),
      child: InkWell(
        borderRadius: BorderRadius.circular(AppRadius.md),
        hoverColor: AppColors.surfaceHover,
        onTap: enabled ? _pickKeyFile : null,
        child: Container(
          height: 42,
          padding: const EdgeInsets.symmetric(horizontal: AppSpacing.s3),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(AppRadius.md),
            border: Border.all(color: AppColors.borderDefault),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                Icons.folder_open_outlined,
                size: 15,
                color: enabled ? AppColors.accent : AppColors.textMuted,
              ),
              const SizedBox(width: 6),
              Text(
                '选择',
                style: TextStyle(
                  fontSize: 13,
                  color: enabled ? AppColors.textPrimary : AppColors.textMuted,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _advancedTab() {
    return _tabBody([
      _field(
        label: '标签',
        controller: _tagsCtrl,
        hint: '逗号分隔，例如：prod, web, cn',
      ),
      const SizedBox(height: AppSpacing.s3),
      const Text(
        '标签用于检索与归类，可留空。',
        style: TextStyle(fontSize: 11.5, color: AppColors.textMuted),
      ),
    ]);
  }

  Widget _notesTab() {
    return _tabBody([
      _field(
        label: '备注',
        controller: _desc,
        hint: '关于这台服务器的说明（可留空）',
        maxLines: 6,
      ),
    ]);
  }

  Widget _protocolSelector() {
    Widget option(String value, IconData icon, String text) {
      final selected = _protocol == value;
      return Expanded(
        child: Material(
          color: selected ? AppColors.surfaceActive : AppColors.surface3,
          borderRadius: BorderRadius.circular(AppRadius.md),
          child: InkWell(
            borderRadius: BorderRadius.circular(AppRadius.md),
            hoverColor: AppColors.surfaceHover,
            onTap: () => _setProtocol(value),
            child: Container(
              height: 38,
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(AppRadius.md),
                border: Border.all(
                  color: selected ? AppColors.accent : AppColors.borderDefault,
                ),
              ),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Icon(
                    icon,
                    size: 15,
                    color: selected ? AppColors.accent : AppColors.textSecondary,
                  ),
                  const SizedBox(width: 6),
                  Text(
                    text,
                    style: TextStyle(
                      fontSize: 13,
                      color: selected
                          ? AppColors.textPrimary
                          : AppColors.textSecondary,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      );
    }

    return Row(
      children: [
        option('SSH', Icons.dns_outlined, 'SSH'),
        const SizedBox(width: 6),
        option('ADB', Icons.android, 'ADB'),
        const SizedBox(width: 6),
        option('iOS', Icons.phone_iphone, 'iOS'),
      ],
    );
  }

  /// 切换协议:自动套用该协议的默认端口 / iOS 默认账号密码。
  void _setProtocol(String value) {
    if (_protocol == value) return;
    setState(() {
      final prevDefault = _protocol == 'ADB' ? '5555' : '22';
      final currentPort = _port.text.trim();
      _protocol = value;

      if (value == 'ADB') {
        if (currentPort.isEmpty || currentPort == prevDefault) {
          _port.text = '5555';
        }
        _elevate = 'none';
      } else if (value == 'iOS') {
        if (currentPort.isEmpty ||
            currentPort == prevDefault ||
            currentPort == '5555') {
          _port.text = _kIosDefaultPort;
        }
        _auth = 'password';
        if (_user.text.trim().isEmpty ||
            _user.text.trim() == 'shell' /* adb leftover */) {
          _user.text = _kIosDefaultUser;
        }
        // 新建或密码仍空时预填 alpine。
        if (!_isEdit || _password.text.isEmpty) {
          if (_password.text.isEmpty) {
            _password.text = _kIosDefaultPass;
            _clearPassword = false;
          }
        }
        if (_name.text.trim().isEmpty) {
          _name.text = _kIosDefaultName;
        }
        _elevate = 'none';
      } else {
        // SSH
        if (currentPort.isEmpty || currentPort == '5555') {
          _port.text = '22';
        }
        // 若密码还是 iOS 默认且切回普通 SSH,清空以免误存。
        if (!_isEdit && _password.text == _kIosDefaultPass) {
          _password.clear();
        }
        _elevate = 'none';
      }
    });
    // 选 iOS / ADB 后跳到对应配置页,方便直接填主机。
    if (value == 'iOS' || value == 'ADB') {
      _tabs.animateTo(1);
    }
  }

  /// 首次连接的「用配对码配对」入口（安卓 11+ 无线调试）。
  Widget _pairRow() {
    return Row(
      children: [
        Material(
          color: AppColors.online.withValues(alpha: 0.14),
          borderRadius: BorderRadius.circular(AppRadius.md),
          child: InkWell(
            borderRadius: BorderRadius.circular(AppRadius.md),
            onTap: _pairDevice,
            child: Container(
              height: 36,
              padding: const EdgeInsets.symmetric(horizontal: AppSpacing.s4),
              child: const Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(Icons.phonelink_ring_outlined,
                      size: 15, color: AppColors.online),
                  SizedBox(width: 6),
                  Text(
                    '用配对码配对（首次连接）',
                    style: TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                      color: AppColors.online,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ],
    );
  }

  Future<void> _pairDevice() async {
    final ip = await showAdbPairDialog(context);
    if (ip != null && ip.isNotEmpty && mounted) {
      setState(() {
        if (_host.text.trim().isEmpty) _host.text = ip;
        final p = _port.text.trim();
        if (p.isEmpty || p == '22') _port.text = '5555';
      });
    }
  }

  Widget _adbHint() {
    return Container(
      padding: const EdgeInsets.all(AppSpacing.s3),
      decoration: BoxDecoration(
        color: AppColors.surface3,
        borderRadius: BorderRadius.circular(AppRadius.md),
        border: Border.all(color: AppColors.borderSubtle),
      ),
      child: const Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(Icons.android, size: 15, color: AppColors.online),
          SizedBox(width: 8),
          Expanded(
            child: Text(
              '通过本机 adb 连接安卓设备，无需用户名 / 密码。\n'
              '请先在手机开启「无线调试 / 网络 ADB（5555）」并授权本机；\n'
              '连接后会自动尝试开启开发者模式。',
              style: TextStyle(
                fontSize: 11.5,
                color: AppColors.textMuted,
                height: 1.5,
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _authSelector() {
    Widget option(String value, IconData icon, String text) {
      final selected = _auth == value;
      return Expanded(
        child: Material(
          color: selected ? AppColors.surfaceActive : AppColors.surface3,
          borderRadius: BorderRadius.circular(AppRadius.md),
          child: InkWell(
            borderRadius: BorderRadius.circular(AppRadius.md),
            hoverColor: AppColors.surfaceHover,
            onTap: () => setState(() => _auth = value),
            child: Container(
              height: 38,
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(AppRadius.md),
                border: Border.all(
                  color: selected
                      ? AppColors.accent
                      : AppColors.borderDefault,
                ),
              ),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Icon(
                    icon,
                    size: 15,
                    color: selected
                        ? AppColors.accent
                        : AppColors.textSecondary,
                  ),
                  const SizedBox(width: 6),
                  Text(
                    text,
                    style: TextStyle(
                      fontSize: 13,
                      color: selected
                          ? AppColors.textPrimary
                          : AppColors.textSecondary,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      );
    }

    return Row(
      children: [
        option('password', Icons.password, '密码'),
        const SizedBox(width: AppSpacing.s3),
        option('publickey', Icons.vpn_key_outlined, '公钥'),
      ],
    );
  }

  Widget _labelText(String text) {
    return Text(
      text,
      style: const TextStyle(
        fontSize: 12,
        color: AppColors.textSecondary,
        fontWeight: FontWeight.w500,
      ),
    );
  }

  Widget _field({
    required String label,
    required TextEditingController controller,
    String? hint,
    TextInputType? keyboardType,
    List<TextInputFormatter>? inputFormatters,
    bool obscure = false,
    int maxLines = 1,
  }) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _labelText(label),
        const SizedBox(height: 6),
        TextField(
          controller: controller,
          keyboardType: keyboardType,
          inputFormatters: inputFormatters,
          obscureText: obscure,
          maxLines: obscure ? 1 : maxLines,
          style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
          cursorColor: AppColors.accent,
          decoration: _decoration(hint),
        ),
      ],
    );
  }

  /// 密钥类字段：编辑态下已配置时给出「留空保持不变」提示，并提供「清除」开关。
  /// `trailing` 可在输入框右侧放一个按钮（如私钥「选择文件」）。
  Widget _secretField({
    required String label,
    required TextEditingController controller,
    required bool clear,
    required bool hadExisting,
    required ValueChanged<bool> onClearChanged,
    required bool obscure,
    String? hint,
    Widget? trailing,
  }) {
    final showClear = _isEdit && hadExisting;
    final placeholder = showClear
        ? '已配置 · 留空保持不变'
        : (hint ?? '');
    final field = TextField(
      controller: controller,
      enabled: !clear,
      obscureText: obscure,
      style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
      cursorColor: AppColors.accent,
      decoration: _decoration(clear ? '将被清除' : placeholder),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            _labelText(label),
            const Spacer(),
            if (showClear)
              InkWell(
                borderRadius: BorderRadius.circular(AppRadius.sm),
                onTap: () => onClearChanged(!clear),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 6,
                    vertical: 2,
                  ),
                  child: Row(
                    children: [
                      Icon(
                        clear
                            ? Icons.check_box_outlined
                            : Icons.check_box_outline_blank,
                        size: 14,
                        color: clear ? AppColors.danger : AppColors.textMuted,
                      ),
                      const SizedBox(width: 4),
                      Text(
                        '清除已保存的值',
                        style: TextStyle(
                          fontSize: 11,
                          color: clear
                              ? AppColors.danger
                              : AppColors.textMuted,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
          ],
        ),
        const SizedBox(height: 6),
        if (trailing == null)
          field
        else
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(child: field),
              const SizedBox(width: AppSpacing.s2),
              trailing,
            ],
          ),
      ],
    );
  }

  InputDecoration _decoration(String? hint) {
    return InputDecoration(
      hintText: hint,
      hintStyle: const TextStyle(fontSize: 13, color: AppColors.textMuted),
      isDense: true,
      contentPadding: const EdgeInsets.symmetric(
        horizontal: AppSpacing.s3,
        vertical: AppSpacing.s3,
      ),
      filled: true,
      fillColor: AppColors.surface3,
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(AppRadius.md),
        borderSide: const BorderSide(color: AppColors.borderDefault),
      ),
      disabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(AppRadius.md),
        borderSide: const BorderSide(color: AppColors.borderSubtle),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(AppRadius.md),
        borderSide: const BorderSide(color: AppColors.accent),
      ),
    );
  }

  Widget _footer() {
    return Padding(
      padding: const EdgeInsets.all(AppSpacing.s4),
      child: Row(
        children: [
          if (_error != null)
            Expanded(
              child: Row(
                children: [
                  const Icon(
                    Icons.error_outline,
                    size: 15,
                    color: AppColors.danger,
                  ),
                  const SizedBox(width: 6),
                  Expanded(
                    child: Text(
                      _error!,
                      style: const TextStyle(
                        fontSize: 12,
                        color: AppColors.danger,
                      ),
                    ),
                  ),
                ],
              ),
            )
          else
            const Spacer(),
          const SizedBox(width: AppSpacing.s3),
          _button(
            label: '取消',
            primary: false,
            onTap: () => Navigator.of(context).pop(),
          ),
          const SizedBox(width: AppSpacing.s2),
          _button(label: '保存', primary: true, onTap: _save),
        ],
      ),
    );
  }

  Widget _button({
    required String label,
    required bool primary,
    required VoidCallback onTap,
  }) {
    return Material(
      color: primary ? AppColors.accent : AppColors.surface3,
      borderRadius: BorderRadius.circular(AppRadius.md),
      child: InkWell(
        borderRadius: BorderRadius.circular(AppRadius.md),
        hoverColor: primary ? AppColors.accentHover : AppColors.surfaceHover,
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
              color: primary ? AppColors.textInverse : AppColors.textSecondary,
            ),
          ),
        ),
      ),
    );
  }
}
