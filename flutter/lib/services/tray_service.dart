import 'package:flutter/foundation.dart';
import 'package:system_tray/system_tray.dart';
import 'dart:io';
import '../widgets/localized_text.dart';

/// Windows system tray.
///
/// The Rust core runs in-process. [onClose] shuts it down before destroying
/// the window.
class TrayService {
  TrayService({
    required Future<void> Function() onShow,
    required Future<void> Function() onStart,
    required Future<void> Function() onStop,
    required Future<void> Function() onClose,
    required String language,
  })  : _onShow = onShow,
        _onStart = onStart,
        _onStop = onStop,
        _onClose = onClose,
        _language = language;

  final Future<void> Function() _onShow;
  final Future<void> Function() _onStart;
  final Future<void> Function() _onStop;
  final Future<void> Function() _onClose;
  String _language;

  final SystemTray _tray = SystemTray();
  final Menu _menu = Menu();
  bool _ready = false;

  /// Get the icon path - try Windows .ico resource first, fallback to asset
  String get _iconPath {
    if (Platform.isWindows) {
      // In release, the icon should be next to the exe in data/
      final exeDir = File(Platform.resolvedExecutable).parent.path;
      final icoPath = '$exeDir/data/app_icon.ico';
      if (File(icoPath).existsSync()) return icoPath;

      // Also try the build directory for debug
      final buildIcoPath =
          '$exeDir/../../../../../windows/runner/resources/app_icon.ico';
      if (File(buildIcoPath).existsSync()) return buildIcoPath;
    }
    // Fallback to asset (debug mode)
    return 'assets/images/tray_icon.png';
  }

  Future<void> init() async {
    if (_ready) return;
    try {
      final iconPath = _iconPath;
      debugPrint('Tray using icon: $iconPath');
      await _tray.initSystemTray(
        title: 'Mocoslime',
        iconPath: iconPath,
        toolTip: 'Mocoslime – ${localizeUiString(_language, 'Starting…')}',
      );
      await _rebuildMenu(
          isStreaming: false,
          summary: localizeUiString(_language, 'Starting…'));
      _tray.registerSystemTrayEventHandler((eventName) {
        if (eventName == 'left-click') {
          _onShow();
        } else if (eventName == 'right-click') {
          _tray.popUpContextMenu();
        }
      });
      _ready = true;
    } catch (e) {
      debugPrint('Tray init failed (continuing without tray): $e');
      // Don't rethrow - app works without tray
    }
  }

  /// Refresh tooltip + menu state. Cheap; call on AppState changes.
  Future<void> update({
    required bool isStreaming,
    required String summary,
    required String language,
  }) async {
    if (!_ready) return;
    _language = language;
    final localizedSummary = localizeUiString(language, summary);
    await _tray.setToolTip('Mocoslime – $localizedSummary');
    await _rebuildMenu(isStreaming: isStreaming, summary: localizedSummary);
  }

  Future<void> _rebuildMenu({
    required bool isStreaming,
    required String summary,
  }) async {
    await _menu.buildFrom([
      MenuItemLabel(
        label: 'Mocoslime – $summary',
        enabled: false,
      ),
      MenuSeparator(),
      MenuItemLabel(
        label: localizeUiString(_language, 'Show window'),
        onClicked: (_) => _onShow(),
      ),
      MenuItemLabel(
        label: localizeUiString(_language, 'Start tracking'),
        enabled: !isStreaming,
        onClicked: (_) => _onStart(),
      ),
      MenuItemLabel(
        label: localizeUiString(_language, 'Stop tracking'),
        enabled: isStreaming,
        onClicked: (_) => _onStop(),
      ),
      MenuSeparator(),
      MenuItemLabel(
        label: localizeUiString(_language, 'Close application'),
        onClicked: (_) => _onClose(),
      ),
    ]);
    await _tray.setContextMenu(_menu);
  }

  Future<void> dispose() async {
    if (_ready) {
      await _tray.destroy();
      _ready = false;
    }
  }
}

