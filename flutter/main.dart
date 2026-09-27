import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:provider/provider.dart';
import 'package:window_manager/window_manager.dart';
import 'services/native_bridge.dart';
import 'services/tray_service.dart';
import 'state/app_state.dart';
import 'screens/dashboard_screen.dart';
import 'screens/trackers_screen.dart';
import 'screens/settings_screen.dart';
import 'screens/calibration_screen.dart';
import 'screens/logs_screen.dart';
import 'widgets/localized_text.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await windowManager.ensureInitialized();

  // NOTE: intentionally no runZonedGuarded here: several Windows plugins
  // (window_manager, system_tray) misbehave when initialized inside a
  // custom zone. Async Dart errors are still captured via
  // FlutterError.onError below and land in the Logs screen.
  await _startup();
}

Future<void> _startup() async {
  WindowOptions windowOptions = const WindowOptions(
    size: Size(1000, 700),
    center: true,
    backgroundColor: Colors.transparent,
    skipTaskbar: false,
    titleBarStyle: TitleBarStyle.normal,
  );
  windowManager.waitUntilReadyToShow(windowOptions, () async {
    await windowManager.show();
    await windowManager.focus();
  });

  final nativeBridge = NativeBridge();
  final appState = AppState(nativeBridge);

  // Route Flutter framework errors into the Logs screen (target
  // `flutter/framework`) so UI crashes are diagnosable like Rust errors.
  final previousOnError = FlutterError.onError;
  FlutterError.onError = (details) {
    try {
      appState.addFlutterLog(
        'ERROR',
        'flutter/framework',
        '${details.exceptionAsString()}\n${details.stack ?? ''}'.trim(),
      );
    } catch (_) {}
    if (previousOnError != null) {
      previousOnError(details);
    } else {
      FlutterError.presentError(details);
    }
  };

  await appState.initialize();

  runApp(
    MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: appState),
      ],
      child: const MocoslimeApp(),
    ),
  );
}

class MocoslimeApp extends StatelessWidget {
  const MocoslimeApp({super.key});

  @override
  Widget build(BuildContext context) {
    return Consumer<AppState>(
      builder: (context, appState, _) {
        final code = appState.settings.language;
        final locale = code == 'zh-CN'
            ? const Locale.fromSubtags(languageCode: 'zh', countryCode: 'CN')
            : Locale(['en', 'de', 'ja'].contains(code) ? code : 'en');
        return MaterialApp(
          title: 'Mocoslime',
          locale: locale,
          localizationsDelegates: const [
            GlobalMaterialLocalizations.delegate,
            GlobalWidgetsLocalizations.delegate,
            GlobalCupertinoLocalizations.delegate,
          ],
          supportedLocales: const [
            Locale('en'),
            Locale('de'),
            Locale('ja'),
            Locale.fromSubtags(languageCode: 'zh', countryCode: 'CN'),
          ],
          theme: ThemeData(
            useMaterial3: true,
            colorSchemeSeed: const Color(0xFF6750A4),
            brightness: Brightness.dark,
          ),
          darkTheme: ThemeData(
            useMaterial3: true,
            colorSchemeSeed: const Color(0xFF6750A4),
            brightness: Brightness.dark,
          ),
          home: const MainScreen(),
          routes: {
            '/dashboard': (context) => const DashboardScreen(),
            '/trackers': (context) => const TrackersScreen(),
            '/settings': (context) => const SettingsScreen(),
            '/calibration': (context) => const CalibrationScreen(),
            '/logs': (context) => const LogsScreen(),
          },
        );
      },
    );
  }
}

class MainScreen extends StatefulWidget {
  const MainScreen({super.key});

  @override
  State<MainScreen> createState() => _MainScreenState();
}

class _MainScreenState extends State<MainScreen> with WindowListener {
  int _selectedIndex = 0;
  TrayService? _tray;
  bool _quitting = false;
  late AppState _appState;

  static const List<Widget> _screens = [
    DashboardScreen(),
    TrackersScreen(),
    CalibrationScreen(),
    SettingsScreen(),
    LogsScreen(),
  ];

  static const List<String> _titles = [
    'Dashboard',
    'Trackers',
    'Calibration',
    'Settings',
    'Logs',
  ];

  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    windowManager.setPreventClose(true);
    windowManager.setTitle('Mocoslime');
    _appState = context.read<AppState>();
    _tray = TrayService(
      language: _appState.settings.language,
      onShow: () async {
        await windowManager.show();
        await windowManager.focus();
      },
      onStart: () async {
        await _appState.startStreaming();
        await _refreshTray();
      },
      onStop: () async {
        await _appState.stopStreaming();
        await _refreshTray();
      },
      onClose: _shutdownAndClose,
    );
    _initTray();
    _appState.addListener(_refreshTray);
    if (_appState.settings.startMinimized) {
      WidgetsBinding.instance.addPostFrameCallback((_) async {
        await windowManager.hide();
      });
    }
  }

  Future<void> _initTray() async {
    try {
      await _tray!.init();
      _refreshTray();
    } catch (e) {
      debugPrint('Tray init failed (continuing without tray): $e');
      _tray = null;
    }
  }

  Future<void> _refreshTray() async {
    await _tray?.update(
      isStreaming: _appState.isStreaming,
      summary: _appState.traySummary,
      language: _appState.settings.language,
    );
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    _appState.removeListener(_refreshTray);
    _tray?.dispose();
    super.dispose();
  }

  Future<void> _shutdownAndClose() async {
    if (_quitting) return;
    _quitting = true;
    try {
      await _appState.shutdown();
    } finally {
      try {
        await _tray?.dispose();
      } finally {
        await windowManager.destroy();
      }
    }
  }

  /// The window close button exits the whole app after stopping tracking.
  @override
  void onWindowClose() => _shutdownAndClose();

  void _onItemTapped(int index) {
    setState(() {
      _selectedIndex = index;
    });
  }

  @override
  Widget build(BuildContext context) {
    final currentLocale = Localizations.localeOf(context);
    final language = currentLocale.countryCode == 'CN'
        ? 'zh-CN'
        : currentLocale.languageCode;
    return Scaffold(
      appBar: AppBar(
        title: LocalizedText(_titles[_selectedIndex]),
        centerTitle: true,
      ),
      body: _screens[_selectedIndex],
      bottomNavigationBar: NavigationBar(
        selectedIndex: _selectedIndex,
        onDestinationSelected: _onItemTapped,
        destinations: [
          NavigationDestination(
            icon: const Icon(Icons.dashboard_outlined),
            selectedIcon: const Icon(Icons.dashboard),
            label: localizeUiString(language, 'Dashboard'),
          ),
          NavigationDestination(
            icon: const Icon(Icons.devices_outlined),
            selectedIcon: const Icon(Icons.devices),
            label: localizeUiString(language, 'Trackers'),
          ),
          NavigationDestination(
            icon: const Icon(Icons.tune_outlined),
            selectedIcon: const Icon(Icons.tune),
            label: localizeUiString(language, 'Calibration'),
          ),
          NavigationDestination(
            icon: const Icon(Icons.settings_outlined),
            selectedIcon: const Icon(Icons.settings),
            label: localizeUiString(language, 'Settings'),
          ),
          NavigationDestination(
            icon: const Icon(Icons.article_outlined),
            selectedIcon: const Icon(Icons.article),
            label: localizeUiString(language, 'Logs'),
          ),
        ],
      ),
    );
  }
}
