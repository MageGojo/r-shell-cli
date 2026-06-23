import 'package:flutter/material.dart';

import 'features/shell/app_scaffold.dart';
import 'shared/theme/app_theme.dart';

class RShellApp extends StatelessWidget {
  const RShellApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Conch',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.dark(),
      home: const AppScaffold(),
    );
  }
}
