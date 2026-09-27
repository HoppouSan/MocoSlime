import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';

import 'package:mocoslime/models/app_state.dart';
import 'package:mocoslime/screens/settings_screen.dart';
import 'package:mocoslime/services/native_bridge.dart';
import 'package:mocoslime/state/app_state.dart';

/// Regression test: the settings screen must build without throwing for
/// default settings AND for values round-tripped through the Rust
/// config (a `..sort()` cascade once crashed every build → white
/// screen in release).
void main() {
  Future<void> pumpSettings(WidgetTester tester, AppState appState) async {
    await tester.pumpWidget(
      ChangeNotifierProvider.value(
        value: appState,
        child: const MaterialApp(home: Scaffold(body: SettingsScreen())),
      ),
    );
    await tester.pump();
  }

  testWidgets('builds with defaults', (WidgetTester tester) async {
    await pumpSettings(tester, AppState(NativeBridge()));
    expect(find.text('Bluetooth'), findsOneWidget);
    expect(find.text('SlimeVR'), findsWidgets);
  });

  testWidgets('builds with Rust round-tripped values',
      (WidgetTester tester) async {
    const raw = '''
{"general":{"start_with_windows":false,"minimize_to_tray":true,"start_minimized":false,"language":"en","log_level":"INFO"},
"bluetooth":{"auto_scan":true,"auto_connect":true,"reconnect_delay_ms":5000,"connection_timeout_ms":10000,"max_trackers":12,"scan_duration_ms":10000},
"slimevr":{"server_ip":"127.0.0.1","server_port":6969,"enabled":true,"packet_rate":128,"auto_discover":true},
"tracking":{"coordinate_system":"SlimeVR","enable_filtering":true,"filter_slerp_factor":0.1,"filter_accel_alpha":0.2},
"trackers":{},"version":1}''';
    final decoded = Map<String, dynamic>.from(jsonDecode(raw) as Map);
    final appState = AppState(NativeBridge());
    appState.settings = AppSettings.fromRustJson(decoded);
    await pumpSettings(tester, appState);
    expect(find.text('Bluetooth'), findsOneWidget);
  });
}
