import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:flutter/foundation.dart';
import '../models/app_state.dart';
import '../services/native_bridge.dart';

class AppState extends ChangeNotifier {
  final NativeBridge _bridge;

  List<TrackerStatus> _trackers = [];
  bool _isStreaming = false;
  String _bleState = 'Ready';
  bool _slimevrConnected = false;
  AppSettings _settings = AppSettings();
  Map<String, dynamic>? _fullConfig;
  String? _lastError;
  Timer? _eventTimer;
  bool _isNativeReady = false;
  // P4 Pose-Profile.
  String _activePose = 'standing';
  bool _autoPoseEnabled = true;
  double _autoPoseConfidence = 1.0;
  Map<String, dynamic>? _poseStatus;
  // P2 BLE.
  bool? _adapterOk;
  String? _adapterError;
  bool _hideWeakDevices = false;
  Timer? _scanTimeout;
  // P1 Event-Stats (Lag-Warnung).
  int _eventLagged = 0;

  AppState(this._bridge);

  double _dataRate = 0.0;
  final Map<String, int> _lastTrackingCounters = {};
  final Map<String, DateTime> _lastTrackingTimes = {};
  final Map<String, double> _trackerRates = {};
  final Map<String, Map<String, dynamic>> _trackingSamples = {};
  final Map<String, List<double>> _trackingPositions = {};

  List<TrackerStatus> get trackers => _trackers;

  /// Gefilterte Liste für die UI (schwache Geräte optional ausblenden).
  List<TrackerStatus> get visibleTrackers {
    if (!_hideWeakDevices) return _trackers;
    final min = _settings.bluetoothRssiMinDbm;
    return _trackers.where((t) => t.rssi == 0 || t.rssi >= min).toList();
  }

  double get dataRate => _dataRate;
  Map<String, dynamic>? trackingSampleFor(String id) => _trackingSamples[id];
  List<double>? estimatedPositionFor(String id) => _trackingPositions[id];
  double trackerDataRate(String id) => _trackerRates[id] ?? 0;
  int get errorCount => _trackers.where((t) => t.hasError).length;
  int get reconnectingCount =>
      _trackers.where((t) => t.state == 'Reconnecting').length;
  bool get isStreaming => _isStreaming;
  String get bleState => _bleState;
  bool get slimevrConnected => _slimevrConnected;
  AppSettings get settings => _settings;
  String get activePose => _activePose;
  bool get autoPoseEnabled => _autoPoseEnabled;
  double get autoPoseConfidence => _autoPoseConfidence;
  Map<String, dynamic>? get poseStatus => _poseStatus;
  bool? get adapterOk => _adapterOk;
  String? get adapterError => _adapterError;
  bool get hideWeakDevices => _hideWeakDevices;
  int get eventLagged => _eventLagged;

  /// Test-only settings injection (widget/regression tests).
  @visibleForTesting
  set settings(AppSettings value) {
    _settings = value;
  }

  String? get lastError => _lastError;
  List<LogEntry> get logs => _logs;
  NativeBridge get bridge => _bridge;

  SlimevrStatus? get slimevrStatus => _slimevrStatus;
  bool get isNativeReady => _isNativeReady;

  final List<LogEntry> _logs = [];
  Timer? _logTimer;
  Timer? _slimevrTimer;
  SlimevrStatus? _slimevrStatus;

  /// One-line summary for the tray tooltip/menu.
  String get traySummary {
    final connected = _trackers.where((t) => t.isConnected).length;
    final slime = _slimevrConnected ? 'SlimeVR on' : 'SlimeVR off';
    final errors = _trackers.where((t) => t.hasError).length;
    final err = errors > 0 ? ' • $errors error(s)' : '';
    return '$connected/${_trackers.length} tracker(s) • $slime$err';
  }

  Future<void> initialize() async {
    await NativeBridge.initialize();
    if (!NativeBridge.isNativeAvailable) {
      _lastError = NativeBridge.lastInitError ?? 'Native bridge unavailable';
      _isNativeReady = false;
      notifyListeners();
      return;
    }
    _isNativeReady = true;
    final initCode = await _bridge.init(jsonEncode(_settings.toRustJson()));
    if (initCode != 0) {
      _lastError =
          'Native init failed (code $initCode): ${FfiError.fromCode(initCode)}';
      _isNativeReady = false;
      notifyListeners();
      return;
    }
    // Load persisted settings/roles BEFORE reading them back: without
    // this, every restart silently resets to defaults.
    final loadCode = await _bridge.loadConfig();
    if (loadCode != 0) {
      _lastError =
          'Saved settings could not be loaded (code $loadCode): ${FfiError.fromCode(loadCode)}';
    }
    await loadSettings();
    await _bridge.setLogLevel(_settings.logLevel);
    await _syncAutostartFromSystem();
    await refresh();
    if (_settings.bluetoothAutoScan) {
      // Small delay to let event loop start before scanning
      await Future.delayed(const Duration(milliseconds: 500));
      await scanDevices();
    }
    _eventTimer?.cancel();
    _eventTimer =
        Timer.periodic(const Duration(milliseconds: 250), (_) => drainEvents());
    _logTimer?.cancel();
    _logTimer = Timer.periodic(const Duration(seconds: 1), (_) => drainLogs());
    _slimevrTimer?.cancel();
    _slimevrTimer = Timer.periodic(
        const Duration(seconds: 2), (_) => refreshSlimevrStatus());
    refreshAdapterStatus();
    refreshSlimevrStatus();
  }

  @override
  void dispose() {
    _eventTimer?.cancel();
    _logTimer?.cancel();
    _slimevrTimer?.cancel();
    _scanTimeout?.cancel();
    super.dispose();
  }

  /// Clean shutdown: stop timers, stop streaming, shut down Rust core.
  Future<void> shutdown() async {
    _eventTimer?.cancel();
    _logTimer?.cancel();
    _slimevrTimer?.cancel();
    _scanTimeout?.cancel();
    try {
      await _bridge.stopStreaming();
    } catch (_) {}
    try {
      await _bridge.shutdown();
    } catch (_) {}
  }

  /// Drain Rust log ring buffer (1 Hz; cheap, never in the 250 ms event path).
  void drainLogs() {
    final entries = _bridge.pollLogs();
    if (entries.isEmpty) return;
    _appendLogs(entries);
    notifyListeners();
  }

  void clearLogs() {
    _logs.clear();
    notifyListeners();
  }

  void _appendLogs(List<LogEntry> entries) {
    if (entries.isEmpty) return;
    _logs.addAll(entries);
    while (_logs.length > 500) {
      _logs.removeRange(0, _logs.length - 500);
    }
  }

  /// Dart-side errors (Flutter framework, FFI misuse, JSON parsing) land
  /// in the same Logs view as the Rust entries (target `flutter/...`).
  void addFlutterLog(String level, String target, String message) {
    _appendLogs([
      LogEntry(
        timestamp: DateTime.now().toIso8601String(),
        level: level,
        target: target,
        message: message,
      )
    ]);
    notifyListeners();
  }

  /// Support bundle for bug reports: Rust diagnostics JSON plus the
  /// currently visible Flutter log tail. Saved to
  /// `%APPDATA%\Mocoslime\diagnostics-<timestamp>.json`; returns the
  /// file path or null when unavailable.
  Future<String?> exportDiagnostics({bool redactPrivateInfo = true}) async {
    final diag = _bridge.getDiagnosticsRaw();
    if (diag == null) {
      _lastError = 'Diagnostics unavailable (native bridge missing)';
      notifyListeners();
      return null;
    }
    diag['flutter_logs'] = _logs
        .map((e) => {
              'timestamp': e.timestamp,
              'level': e.level,
              'target': e.target,
              'message': e.message,
            })
        .toList();
    diag['exported_at'] = DateTime.now().toIso8601String();
    if (redactPrivateInfo) {
      final sanitized = _redactDiagnosticValue(diag);
      if (sanitized is Map<String, dynamic>) {
        diag
          ..clear()
          ..addAll(sanitized);
      }
    }
    final appData = Platform.environment['APPDATA'];
    if (appData == null || appData.isEmpty) {
      _lastError = 'Diagnostics export failed: %APPDATA% not set';
      notifyListeners();
      return null;
    }
    final stamp =
        DateTime.now().toIso8601String().replaceAll(':', '-').split('.').first;
    final path = '$appData${Platform.pathSeparator}Mocoslime'
        '${Platform.pathSeparator}diagnostics-$stamp.json';
    try {
      final file = File(path);
      await file.parent.create(recursive: true);
      await file
          .writeAsString(const JsonEncoder.withIndent('  ').convert(diag));
      return path;
    } catch (e) {
      _lastError = 'Diagnostics export failed: $e';
      notifyListeners();
      return null;
    }
  }

  Future<void> _syncAutostartFromSystem() async {
    final state = _bridge.isAutostart();
    if (state == 1 || state == 0) {
      _settings.startWithWindows = state == 1;
    }
  }

  Future<void> loadSettings() async {
    final raw = _bridge.getConfigRaw();
    if (raw == null || raw.isEmpty) return;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map) {
        final map = Map<String, dynamic>.from(decoded);
        _fullConfig = map;
        _settings = AppSettings.fromRustJson(map);
        if (map['tracking'] is Map) {
          final t = Map<String, dynamic>.from(map['tracking'] as Map);
          _activePose = (t['active_pose'] ?? 'standing').toString();
          _autoPoseEnabled = t['enable_auto_pose'] != false;
        }
        notifyListeners();
      }
    } catch (e) {
      debugPrint('Error parsing config: $e');
    }
    refreshPoseStatus();
  }

  void refreshPoseStatus() {
    final raw = _bridge.getPoseStatusRaw();
    if (raw == null) return;
    _poseStatus = raw;
    final ap = raw['active_pose']?.toString();
    if (ap != null && ap.isNotEmpty) {
      _activePose = ap;
      _settings.trackingActivePose = ap;
    }
    if (raw['auto_enabled'] is bool) {
      _autoPoseEnabled = raw['auto_enabled'] as bool;
      _settings.trackingEnableAutoPose = _autoPoseEnabled;
    }
    final conf = raw['confidence'];
    if (conf is num) _autoPoseConfidence = conf.toDouble();
    notifyListeners();
  }

  void refreshAdapterStatus() {
    final raw = _bridge.getAdapterStatusRaw();
    if (raw == null) {
      _adapterOk = null;
      _adapterError = null;
      return;
    }
    _adapterOk = raw['ok'] == true;
    _adapterError = raw['error']?.toString();
    notifyListeners();
  }

  void refreshEventStats() {
    final raw = _bridge.getEventStatsRaw();
    if (raw == null) return;
    final lag = raw['lagged'];
    if (lag is num) {
      _eventLagged = lag.toInt();
      notifyListeners();
    }
  }

  void setHideWeakDevices(bool v) {
    _hideWeakDevices = v;
    notifyListeners();
  }

  bool isMountFlip(String bluetoothAddress) {
    final trackers = _fullConfig?['trackers'];
    if (trackers is Map) {
      final entry = trackers[bluetoothAddress];
      if (entry is Map) return entry['mount_flip'] == true;
    }
    return false;
  }

  Future<void> setMountFlip(
      String deviceId, String bluetoothAddress, bool flip) async {
    final code = await _bridge.setMountFlip(deviceId, flip);
    if (code != 0) {
      _lastError = 'Mount-Flip failed (code $code)';
      notifyListeners();
      return;
    }
    await loadSettings();
  }

  Future<void> refreshSlimevrStatus() async {
    final raw = _bridge.getSlimevrStatusRaw();
    if (raw == null) return;
    try {
      _slimevrStatus = SlimevrStatus.fromJson(raw);
      notifyListeners();
    } catch (e) {
      debugPrint('Error parsing SlimeVR status: $e');
    }
  }

  /// SlimeVR-Reconnect: nutzt gespeicherte IP/Port (Auto oder manuell).
  Future<void> reconnectSlimevr() async {
    _lastError = null;
    notifyListeners();
    await startStreaming();
    await refreshSlimevrStatus();
  }

  /// Nur Handshakes nachsenden (z.B. Server wurde neu gestartet).
  Future<void> resendHandshake() async {
    final code = await _bridge.resendHandshake();
    if (code != 0) {
      _lastError = 'Handshake-Resend failed (code $code)';
      notifyListeners();
    }
  }

  Future<void> refresh() async {
    try {
      _trackers = _bridge.getDevices();
      _updateDataRate();
    } catch (e) {
      debugPrint('Error refreshing devices: $e');
      _trackers = [];
    }
    notifyListeners();
  }

  /// Packets/sec across all trackers, derived from monotonic packet
  /// counters (no fabricated traffic measurement).
  void _updateDataRate() {
    final now = DateTime.now();
    _trackerRates.removeWhere((id, _) {
      final last = _lastTrackingTimes[id];
      if (last == null || now.difference(last) > const Duration(seconds: 2)) {
        _lastTrackingCounters.remove(id);
        _lastTrackingTimes.remove(id);
        return true;
      }
      return false;
    });
    _dataRate = _trackerRates.values.fold<double>(0, (a, b) => a + b);
  }

  /// Drain all pending Rust events without blocking the UI.
  void drainEvents() {
    final previousRate = _dataRate;
    for (int i = 0; i < 32; i++) {
      final event = _bridge.pollEvent();
      if (event == null) {
        _updateDataRate();
        if (_dataRate != previousRate) notifyListeners();
        return;
      }
      _handleEvent(event);
    }
    _updateDataRate();
  }

  /// Strip host-specific identifiers and addresses from support bundles by
  /// default. The UI offers an explicit opt-in when the support recipient
  /// needs those values to reproduce a network or device discovery issue.
  dynamic _redactDiagnosticValue(dynamic value, {String key = ''}) {
    final normalizedKey = key.toLowerCase();
    if (normalizedKey == 'log_dir' ||
        normalizedKey == 'device_id' ||
        normalizedKey.contains('uuid') ||
        normalizedKey.contains('mac') ||
        normalizedKey.contains('bluetooth_address') ||
        normalizedKey.contains('mac_address') ||
        normalizedKey == 'server_ip' ||
        normalizedKey == 'ip') {
      return '[redacted]';
    }
    if (value is Map) {
      final result = <String, dynamic>{};
      var trackerIndex = 0;
      for (final entry in value.entries) {
        final entryKey = entry.key.toString();
        if (normalizedKey == 'trackers' &&
            entry.value is Map &&
            RegExp(r'^(?:[0-9a-f]{2}[:-]){5}[0-9a-f]{2}$', caseSensitive: false)
                .hasMatch(entryKey)) {
          trackerIndex++;
          result['tracker_$trackerIndex'] = _redactDiagnosticValue(entry.value);
        } else {
          result[entryKey] = _redactDiagnosticValue(entry.value, key: entryKey);
        }
      }
      return result;
    }
    if (value is List) {
      return value.map((item) => _redactDiagnosticValue(item)).toList();
    }
    if (value is String) {
      return value
          .replaceAll(RegExp(r'\b(?:\d{1,3}\.){3}\d{1,3}\b'), '[redacted-ip]')
          .replaceAll(
              RegExp(r'\b(?:[0-9a-f]{2}[:-]){5}[0-9a-f]{2}\b',
                  caseSensitive: false),
              '[redacted-mac]')
          .replaceAll(
              RegExp(
                  r'\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b',
                  caseSensitive: false),
              '[redacted-id]')
          .replaceAll(RegExp(r'[A-Za-z]:\\[^\r\n" ]+'), '[redacted-path]');
    }
    return value;
  }

  void _handleEvent(CoreEvent event) {
    switch (event.kind) {
      case 'DeviceDiscovered':
      case 'DeviceUpdated':
        if (event.status != null) {
          final status = TrackerStatus.fromJson(event.status!);
          final index =
              _trackers.indexWhere((t) => t.deviceId == status.deviceId);
          if (index >= 0) {
            _trackers[index] = status;
          } else {
            _trackers.add(status);
          }
          notifyListeners();
        } else {
          refresh();
        }
        break;
      case 'DeviceConnected':
      case 'DeviceDisconnected':
        refresh();
        break;
      case 'BatteryUpdated':
        if (event.deviceId != null && event.battery != null) {
          final index =
              _trackers.indexWhere((t) => t.deviceId == event.deviceId);
          if (index >= 0) {
            final old = _trackers[index];
            _trackers[index] = TrackerStatus(
              deviceId: old.deviceId,
              bluetoothAddress: old.bluetoothAddress,
              name: old.name,
              state: old.state,
              assignedRole: old.assignedRole,
              battery: BatteryInfo.fromJson(event.battery!),
              rssi: old.rssi,
              firmwareVersion: old.firmwareVersion,
              packetCount: old.packetCount,
              lastPacketNs: old.lastPacketNs,
              reconnectAttempts: old.reconnectAttempts,
              reconnectInMs: old.reconnectInMs,
              error: old.error,
            );
            notifyListeners();
          }
        }
        break;
      case 'AutoPoseChanged':
        if (event.pose != null) {
          _activePose = event.pose!;
          _settings.trackingActivePose = event.pose!;
          if (event.confidence != null) {
            _autoPoseConfidence = event.confidence!;
          }
          notifyListeners();
          refreshPoseStatus();
        }
        break;
      case 'ActivePoseChanged':
        if (event.pose != null) {
          _activePose = event.pose!;
          _settings.trackingActivePose = event.pose!;
          notifyListeners();
          refreshPoseStatus();
        }
        break;
      case 'SlimeVRConnected':
        _slimevrConnected = true;
        _isStreaming = true;
        notifyListeners();
        refreshSlimevrStatus();
        break;
      case 'SlimeVRDiscovered':
        // Nur Info: Ziel folgt via SlimeVRConnected. Trotzdem Status ziehen.
        refreshSlimevrStatus();
        break;
      case 'SlimeVRDisconnected':
        _slimevrConnected = false;
        _isStreaming = false;
        notifyListeners();
        refreshSlimevrStatus();
        break;
      case 'Error':
        _lastError = event.message;
        // A failed scan must not leave the UI stuck on "Scanning...".
        if (_bleState != 'Ready') {
          _bleState = 'Ready';
          _scanTimeout?.cancel();
        }
        notifyListeners();
        break;
      case 'ScanCompleted':
        _scanTimeout?.cancel();
        unawaited(refresh());
        if ((event.count ?? 0) == 0 && _trackers.isEmpty) {
          _lastError =
              'No Mocopi trackers found. Turn them on, keep them close, and check that Bluetooth is enabled.';
        }
        _bleState = 'Ready';
        notifyListeners();
        break;
      case 'TrackingData':
        final id = event.deviceId;
        final sample = event.trackingSample;
        if (id != null && sample != null) {
          _trackingSamples[id] = sample;
          final counterValue = sample['counter'];
          final counter = counterValue is num
              ? counterValue.toInt()
              : int.tryParse('$counterValue');
          final now = DateTime.now();
          final previous = _lastTrackingCounters[id];
          final previousAt = _lastTrackingTimes[id];
          if (counter != null && previous != null && previousAt != null) {
            final elapsed = now.difference(previousAt).inMicroseconds / 1e6;
            final delta = counter >= previous ? counter - previous : counter;
            if (elapsed > 0 && delta >= 0) _trackerRates[id] = delta / elapsed;
          }
          if (counter != null) _lastTrackingCounters[id] = counter;
          _lastTrackingTimes[id] = now;
          _updateDataRate();
          notifyListeners();
        }
        break;
      case 'TrackingPosition':
        final id = event.deviceId;
        if (id != null) {
          if (event.position == null) {
            _trackingPositions.remove(id);
          } else {
            _trackingPositions[id] = event.position!;
          }
          notifyListeners();
        }
        break;
      default:
        break;
    }
  }

  Future<void> scanDevices() async {
    if (_bleState == 'Scanning...') return;
    // Nativer Doppel-Scan-Schutz (P-C): statt Fehler warten.
    if (_bridge.isScanningNative() == 1) return;
    refreshAdapterStatus();
    if (_adapterOk == false) {
      _lastError =
          'Bluetooth adapter not found. Enable Bluetooth in Windows settings. ${_adapterError ?? ''}'
              .trim();
      notifyListeners();
      return;
    }
    _bleState = 'Scanning...';
    _lastError = null;
    notifyListeners();
    // Harter Timeout als Rückfall: scan_duration (10s) + 8s Budget.
    _scanTimeout?.cancel();
    _scanTimeout = Timer(const Duration(seconds: 20), () async {
      if (_bleState == 'Scanning...') {
        await refresh();
        _bleState = 'Ready';
        notifyListeners();
      }
    });
    final code = await _bridge.scanDevices();
    if (code != 0) {
      _scanTimeout?.cancel();
      // "Scan already in progress" ist kein harter Fehler: einfach auf
      // das laufende ScanCompleted warten statt Ready zu forcieren.
      if (code == 6 || _lastError?.contains('already') == true) {
        return;
      }
      _lastError = 'Scan failed (code $code): ${FfiError.fromCode(code)}';
      _bleState = 'Ready';
      notifyListeners();
      return;
    }
    // The Rust scan runs async (live watcher window ~ scan_duration_ms).
    // Drain events + refresh progressively so found trackers appear
    // immediately instead of only after a fixed delay.
    final deadline = DateTime.now().add(const Duration(seconds: 18));
    while (DateTime.now().isBefore(deadline)) {
      await Future.delayed(const Duration(milliseconds: 500));
      drainEvents();
      await refresh();
      // ScanCompleted event (or an Error) flips the state back to Ready.
      if (_bleState == 'Ready') {
        _scanTimeout?.cancel();
        return;
      }
    }
    // Fallback: never leave the UI stuck in "Scanning...".
    _scanTimeout?.cancel();
    await refresh();
    _bleState = 'Ready';
    notifyListeners();
  }

  Future<void> connectTracker(String deviceId) async {
    // Validate UUID format early
    final uuidRegex = RegExp(
        r'^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$');
    if (!uuidRegex.hasMatch(deviceId)) {
      _lastError = 'Invalid device ID format: $deviceId';
      notifyListeners();
      return;
    }
    final code = await _bridge.connectDevice(deviceId);
    if (code != 0) {
      _lastError = 'Connect failed (code $code): ${FfiError.fromCode(code)}';
      notifyListeners();
    }
    await refresh();
  }

  Future<void> disconnectTracker(String deviceId) async {
    final uuidRegex = RegExp(
        r'^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$');
    if (!uuidRegex.hasMatch(deviceId)) {
      _lastError = 'Invalid device ID format: $deviceId';
      notifyListeners();
      return;
    }
    final code = await _bridge.disconnectDevice(deviceId);
    if (code != 0) {
      _lastError =
          'Tracker could not be stopped (code $code): ${FfiError.fromCode(code)}';
      notifyListeners();
    }
    await refresh();
  }

  /// Force a manual BLE reconnect for one tracker. Both commands go through
  /// the same Rust command queue, so disconnect completes before connect runs.
  Future<void> reconnectTracker(String deviceId) async {
    final uuidRegex = RegExp(
        r'^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$');
    if (!uuidRegex.hasMatch(deviceId)) {
      _lastError = 'Invalid device ID format: $deviceId';
      notifyListeners();
      return;
    }
    _lastError = null;
    notifyListeners();
    final disconnectCode = await _bridge.disconnectDevice(deviceId);
    if (disconnectCode != 0) {
      _lastError =
          'Reconnect could not disconnect tracker (code $disconnectCode): ${FfiError.fromCode(disconnectCode)}';
      notifyListeners();
      await refresh();
      return;
    }
    final connectCode = await _bridge.connectDevice(deviceId);
    if (connectCode != 0) {
      _lastError =
          'Reconnect failed (code $connectCode): ${FfiError.fromCode(connectCode)}';
      notifyListeners();
    }
    await refresh();
  }

  Future<void> assignTracker(String deviceId, String role) async {
    final uuidRegex = RegExp(
        r'^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$');
    if (!uuidRegex.hasMatch(deviceId)) {
      _lastError = 'Invalid device ID format: $deviceId';
      notifyListeners();
      return;
    }
    final roleIndex = TrackerRole.allRoles.indexOf(role);
    if (roleIndex < 0) return;
    final code = await _bridge.assignTracker(deviceId, roleIndex);
    if (code != 0) {
      _lastError =
          'Role assignment failed (code $code): ${FfiError.fromCode(code)}';
      notifyListeners();
      return;
    }
    await _refreshFullConfig();
    await refresh();
  }

  Future<void> setActivePose(String pose) async {
    _activePose = pose;
    _settings.trackingActivePose = pose;
    notifyListeners();
    final code = await _bridge.setActivePose(pose);
    if (code != 0) {
      _lastError = 'Pose switch failed (code $code)';
      notifyListeners();
      return;
    }
    await saveSettings();
    refreshPoseStatus();
  }

  Future<void> setAutoPose(bool enabled) async {
    _autoPoseEnabled = enabled;
    _settings.trackingEnableAutoPose = enabled;
    notifyListeners();
    final code = await _bridge.setAutoPose(enabled);
    if (code != 0) {
      _lastError = 'Auto-pose failed (code $code)';
      notifyListeners();
      return;
    }
    await saveSettings();
    refreshPoseStatus();
  }

  Future<void> saveSkeletonHeight(double heightCm) async {
    _settings.skeletonHeightCm = heightCm;
    notifyListeners();
    final code = await _bridge
        .setSkeleton('{"height_cm":$heightCm,"auto_estimate":true}');
    if (code != 0) {
      _lastError = 'Skeleton failed (code $code)';
      notifyListeners();
      return;
    }
    await saveSettings();
  }

  Future<void> startStreaming() async {
    // Always configure the SlimeVR target before starting tracker streaming.
    final slimeCode = await _bridge.setSlimevrAddress(
        _settings.slimevrServerIp, _settings.slimevrServerPort);
    if (slimeCode != 0) {
      _lastError = 'SlimeVR address rejected (code $slimeCode)';
      notifyListeners();
      return;
    }
    final code = await _bridge.startStreaming();
    if (code != 0) {
      _lastError = 'Start streaming failed (code $code)';
      notifyListeners();
      return;
    }
    _isStreaming = true;
    notifyListeners();
  }

  Future<void> stopStreaming() async {
    await _bridge.stopStreaming();
    _isStreaming = false;
    _slimevrConnected = false;
    notifyListeners();
  }

  Future<void> updateSetting(String key, dynamic value) async {
    switch (key) {
      case 'startWithWindows':
        final autostart = value as bool;
        _settings.startWithWindows = autostart;
        final code = await _bridge.setAutostart(autostart);
        if (code != 0) {
          _lastError = 'Autostart konnte nicht gesetzt werden (Code $code)';
        }
        break;
      case 'minimizeToTray':
        _settings.minimizeToTray = value as bool;
        break;
      case 'startMinimized':
        _settings.startMinimized = value as bool;
        break;
      case 'language':
        _settings.language = AppSettings.normalizeLanguage(value);
        // Language changes must survive a restart just like the other
        // persisted settings. Notify immediately so the UI switches without
        // waiting for the native config round-trip.
        notifyListeners();
        await saveSettings();
        break;
      case 'logLevel':
        final level = value as String;
        _settings.logLevel = level;
        await _bridge.setLogLevel(level);
        break;
      case 'bluetoothAutoScan':
        _settings.bluetoothAutoScan = value as bool;
        break;
      case 'bluetoothAutoConnect':
        _settings.bluetoothAutoConnect = value as bool;
        break;
      case 'bluetoothReconnectDelayMs':
        _settings.bluetoothReconnectDelayMs = value as int;
        break;
      case 'bluetoothConnectionTimeoutMs':
        _settings.bluetoothConnectionTimeoutMs = value as int;
        break;
      case 'bluetoothMaxTrackers':
        _settings.bluetoothMaxTrackers = value as int;
        break;
      case 'bluetoothRssiMinDbm':
        _settings.bluetoothRssiMinDbm = value as int;
        break;
      case 'slimevrServerIp':
        final ip = (value as String).trim();
        if (!_isValidIp(ip)) {
          _lastError = 'Ungültige SlimeVR-IP: $ip';
          notifyListeners();
          return;
        }
        _settings.slimevrServerIp = ip;
        await saveSettings();
        break;
      case 'slimevrServerPort':
        final port = value as int;
        if (port < 1 || port > 65535) {
          _lastError = 'Ungültiger SlimeVR-Port: $port (1..65535)';
          notifyListeners();
          return;
        }
        _settings.slimevrServerPort = port;
        await saveSettings();
        break;
      case 'slimevrEnabled':
        _settings.slimevrEnabled = value as bool;
        await saveSettings();
        break;
      case 'slimevrPacketRate':
        final rate = value as int;
        if (rate < 0 || rate > 200) {
          _lastError = 'Ungültige Paketrate: $rate (0..200, 0=unlimitiert)';
          notifyListeners();
          return;
        }
        _settings.slimevrPacketRate = rate;
        break;
      case 'trackingCoordinateSystem':
        _settings.trackingCoordinateSystem = value as String;
        break;
      case 'trackingEnableFiltering':
        _settings.trackingEnableFiltering = value as bool;
        break;
      case 'trackingFilterSlerpFactor':
        _settings.trackingFilterSlerpFactor = value as double;
        break;
      case 'trackingFilterAccelAlpha':
        _settings.trackingFilterAccelAlpha = value as double;
        break;
      default:
        return;
    }
    // Apply Bluetooth config to native bridge when any Bluetooth setting changes
    if (key.startsWith('bluetooth')) {
      await _applyBluetoothConfig();
    }
    notifyListeners();
  }

  Future<void> _applyBluetoothConfig() async {
    final json = jsonEncode({
      'auto_scan': _settings.bluetoothAutoScan,
      'auto_connect': _settings.bluetoothAutoConnect,
      'reconnect_delay_ms': _settings.bluetoothReconnectDelayMs,
      'connection_timeout_ms': _settings.bluetoothConnectionTimeoutMs,
      'max_trackers': _settings.bluetoothMaxTrackers,
      'scan_duration_ms': 10000,
      'rssi_min_dbm': _settings.bluetoothRssiMinDbm,
    });
    final code = await _bridge.setBluetoothConfig(json);
    if (code != 0) {
      _lastError = 'Bluetooth config konnte nicht gesetzt werden (Code $code)';
    }
  }

  Future<void> saveSettings() async {
    // Trackers/Version immer frisch aus dem Core lesen: _fullConfig kann
    // seit loadSettings() veraltet sein (Rolle/Offset/Mount-Flip wurden
    // direkt im Core persistiert) – sonst löscht ein Settings-Save sie.
    await _refreshFullConfig();
    final payload = jsonEncode(_settings.toRustJson(_fullConfig));
    final code = await _bridge.setConfig(payload);
    if (code != 0) {
      _lastError = 'Save settings failed (code $code)';
      notifyListeners();
      return;
    }
    await loadSettings();
  }

  /// _fullConfig ohne UI-Seiteneffekte nachziehen (kein notify nötig,
  /// ruft nur notifyListeners wenn sich etwas geändert hat – hier bewusst
  /// ohne, Caller entscheidet).
  Future<void> _refreshFullConfig() async {
    final raw = _bridge.getConfigRaw();
    if (raw == null || raw.isEmpty) return;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map) {
        _fullConfig = Map<String, dynamic>.from(decoded);
      }
    } catch (e) {
      debugPrint('Error refreshing full config: $e');
    }
  }

  static bool _isValidIp(String ip) {
    // IPv4 strikt, Hostnamen pragmatisch (SlimeVR-Server im LAN).
    final v4 = RegExp(r'^(\d{1,3}\.){3}\d{1,3}$');
    if (v4.hasMatch(ip)) {
      return ip.split('.').every((o) {
        final n = int.tryParse(o);
        return n != null && n >= 0 && n <= 255;
      });
    }
    final host = RegExp(r'^[A-Za-z0-9]([A-Za-z0-9\-.]{0,61}[A-Za-z0-9])?$');
    return ip == 'localhost' || host.hasMatch(ip);
  }
}
