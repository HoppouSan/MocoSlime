import 'dart:convert';
import 'dart:developer' show log;
import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';
import '../models/app_state.dart';

typedef _InitNative = Int32 Function(Pointer<Utf8>);
typedef _InitDart = int Function(Pointer<Utf8>);
typedef _NoArgNative = Int32 Function();
typedef _NoArgDart = int Function();
typedef _StrArgNative = Int32 Function(Pointer<Utf8>);
typedef _StrArgDart = int Function(Pointer<Utf8>);
typedef _AssignNative = Int32 Function(Pointer<Utf8>, Int32);
typedef _AssignDart = int Function(Pointer<Utf8>, int);
typedef _SlimeNative = Int32 Function(Pointer<Utf8>, Int32);
typedef _SlimeDart = int Function(Pointer<Utf8>, int);
typedef _GetStrNative = Pointer<Utf8> Function();
typedef _GetStrDart = Pointer<Utf8> Function();
typedef _GetDevNative = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _GetDevDart = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _MountNative = Int32 Function(Pointer<Utf8>, Int32);
typedef _MountDart = int Function(Pointer<Utf8>, int);
typedef _FreeNative = Void Function(Pointer<Utf8>);
typedef _FreeDart = void Function(Pointer<Utf8>);
typedef _ApiNative = Int32 Function();
typedef _ApiDart = int Function();
typedef _AutoNative = Int32 Function(Int32);
typedef _AutoDart = int Function(int);

/// FFI bindings for `mocoslime_ffi` (Rust `crates/ffi`).
///
/// No mock data lives here: if the native library cannot be loaded,
/// every call returns an error/empty result and the UI shows the real
/// disconnected state instead of fabricated trackers.
class NativeBridge {
  static bool _initialized = false;
  static DynamicLibrary? _lib;
  static String? _lastInitError;

  static _NoArgDart? _shutdown;
  static _StrArgDart? _init;
  static _NoArgDart? _scan;
  static _StrArgDart? _connect;
  static _StrArgDart? _disconnect;
  static _AssignDart? _assign;
  static _NoArgDart? _startStreaming;
  static _NoArgDart? _stopStreaming;
  static _SlimeDart? _setSlime;
  static _StrArgDart? _setConfig;
  static _StrArgDart? _setLanguage;
  static _NoArgDart? _loadConfig;
  static _GetStrDart? _getConfig;
  static _GetStrDart? _getDevices;
  static _GetDevDart? _getDevice;
  static _GetStrDart? _pollEvent;
  static _GetStrDart? _pollLogs;
  static _GetStrDart? _getDiagnostics;
  static _StrArgDart? _setBluetoothConfig;
  static _GetStrDart? _getVersion;
  static _FreeDart? _freeString;
  static _ApiDart? _apiVersion;
  static _StrArgDart? _setLogLevel;
  static _AutoDart? _setAutostart;
  static _ApiDart? _isAutostart;
  // P4 Pose-Profile (optional: ältere DLLs ohne diese Symbole laufen weiter).
  static _StrArgDart? _setActivePose;
  static _AutoDart? _setAutoPose;
  static _StrArgDart? _setSkeleton;
  static _GetStrDart? _getPoseStatus;
  // P1 Event-Stats (API v3): optional.
  static _GetStrDart? _getEventStats;
  // P2 Adapter-Status (leichtgewichtig): optional.
  static _GetStrDart? _getAdapterStatus;
  // P3 SlimeVR-Status: optional.
  static _GetStrDart? _getSlimevrStatus;
  // T3 Mount-Flip: optional (alte DLL -> No-op Erfolg).
  static _MountDart? _setMountFlip;
  // Dead-Code-Ersatz: Cancel/Reset-All/IsScanning (alte DLL -> No-op).
  static _ApiDart? _isScanning;
  static _NoArgDart? _resendHandshake;

  static bool get isNativeAvailable => _lib != null;
  static String? get lastInitError => _lastInitError;

  static String _libFileName() {
    if (Platform.isWindows) return 'mocoslime_ffi.dll';
    if (Platform.isLinux) return 'libmocoslime_ffi.so';
    if (Platform.isMacOS) return 'libmocoslime_ffi.dylib';
    throw UnsupportedError('Unsupported platform for mocoslime_ffi');
  }

  /// Search order: exe dir, exe dir/data, cwd, flutter build outputs.
  /// The old code only checked the CWD, so the installed app reported
  /// "Native library not found" even though the DLL sat next to the exe.
  static List<String> _candidateLibPaths() {
    final name = _libFileName();
    final candidates = <String>[name];
    try {
      final exeDir = File(Platform.resolvedExecutable).parent.path;
      candidates.add('$exeDir${Platform.pathSeparator}$name');
      candidates.add(
          '$exeDir${Platform.pathSeparator}data${Platform.pathSeparator}$name');
    } catch (_) {}
    return candidates;
  }

  static Future<void> initialize() async {
    if (_initialized) return;
    _lastInitError = null;
    try {
      final candidates = _candidateLibPaths();
      String? found;
      for (final p in candidates) {
        if (File(p).existsSync()) {
          found = p;
          break;
        }
      }
      if (found == null) {
        _lastInitError =
            'Native library not found. Searched: ${candidates.join(', ')}';
        _lib = null;
        _initialized = true;
        return;
      }
      _lib = DynamicLibrary.open(found);
      final lib = _lib!;
      _init = lib.lookupFunction<_InitNative, _InitDart>('mocoslime_init');
      _shutdown =
          lib.lookupFunction<_NoArgNative, _NoArgDart>('mocoslime_shutdown');
      _scan =
          lib.lookupFunction<_NoArgNative, _NoArgDart>('mocoslime_scan_devices');
      _connect = lib
          .lookupFunction<_StrArgNative, _StrArgDart>('mocoslime_connect_device');
      _disconnect = lib.lookupFunction<_StrArgNative, _StrArgDart>(
          'mocoslime_disconnect_device');
      _assign = lib
          .lookupFunction<_AssignNative, _AssignDart>('mocoslime_assign_tracker');
      _startStreaming = lib
          .lookupFunction<_NoArgNative, _NoArgDart>('mocoslime_start_streaming');
      _stopStreaming = lib
          .lookupFunction<_NoArgNative, _NoArgDart>('mocoslime_stop_streaming');
      _setSlime = lib.lookupFunction<_SlimeNative, _SlimeDart>(
          'mocoslime_set_slimevr_address');
      _setConfig =
          lib.lookupFunction<_StrArgNative, _StrArgDart>('mocoslime_set_config');
      try {
        _setLanguage = lib.lookupFunction<_StrArgNative, _StrArgDart>(
            'mocoslime_set_language');
      } catch (_) {
        _setLanguage = null;
      }
      // Config persistence (added later than v1 API): optional, the app
      // works without it if the loaded DLL is older.
      try {
        _loadConfig =
            lib.lookupFunction<_NoArgNative, _NoArgDart>('mocoslime_load_config');
      } catch (_) {
        _loadConfig = null;
      }
      _getConfig =
          lib.lookupFunction<_GetStrNative, _GetStrDart>('mocoslime_get_config');
      _getDevices =
          lib.lookupFunction<_GetStrNative, _GetStrDart>('mocoslime_get_devices');
      _getDevice =
          lib.lookupFunction<_GetDevNative, _GetDevDart>('mocoslime_get_device');
      _pollEvent =
          lib.lookupFunction<_GetStrNative, _GetStrDart>('mocoslime_poll_event');
      _setBluetoothConfig = lib.lookupFunction<_StrArgNative, _StrArgDart>(
          'mocoslime_set_bluetooth_config');
      _pollLogs =
          lib.lookupFunction<_GetStrNative, _GetStrDart>('mocoslime_poll_logs');
      // Diagnostics bundle (added later than v1 API): optional, the app
      // works without it if the loaded DLL is older.
      try {
        _getDiagnostics = lib.lookupFunction<_GetStrNative, _GetStrDart>(
            'mocoslime_get_diagnostics');
      } catch (_) {
        _getDiagnostics = null;
      }
      _getVersion =
          lib.lookupFunction<_GetStrNative, _GetStrDart>('mocoslime_get_version');
      _freeString =
          lib.lookupFunction<_FreeNative, _FreeDart>('mocoslime_free_string');
      _apiVersion =
          lib.lookupFunction<_ApiNative, _ApiDart>('mocoslime_api_version');
      _setLogLevel = lib
          .lookupFunction<_StrArgNative, _StrArgDart>('mocoslime_set_log_level');
      _setAutostart =
          lib.lookupFunction<_AutoNative, _AutoDart>('mocoslime_set_autostart');
      _isAutostart =
          lib.lookupFunction<_ApiNative, _ApiDart>('mocoslime_is_autostart');
      try {
        _setActivePose = lib.lookupFunction<_StrArgNative, _StrArgDart>(
            'mocoslime_set_active_pose');
      } catch (_) {
        _setActivePose = null;
      }
      try {
        _setAutoPose =
            lib.lookupFunction<_AutoNative, _AutoDart>('mocoslime_set_auto_pose');
      } catch (_) {
        _setAutoPose = null;
      }
      try {
        _setSkeleton = lib
            .lookupFunction<_StrArgNative, _StrArgDart>('mocoslime_set_skeleton');
      } catch (_) {
        _setSkeleton = null;
      }
      try {
        _getPoseStatus = lib.lookupFunction<_GetStrNative, _GetStrDart>(
            'mocoslime_get_pose_status');
      } catch (_) {
        _getPoseStatus = null;
      }
      // P1 Event-Stats (API v3): optional.
      try {
        _getEventStats = lib.lookupFunction<_GetStrNative, _GetStrDart>(
            'mocoslime_get_event_stats');
      } catch (_) {
        _getEventStats = null;
      }
      // P2 Adapter-Status: optional (alte DLL -> null -> Unknown).
      try {
        _getAdapterStatus = lib.lookupFunction<_GetStrNative, _GetStrDart>(
            'mocoslime_get_adapter_status');
      } catch (_) {
        _getAdapterStatus = null;
      }
      // P3 SlimeVR-Status: optional.
      try {
        _getSlimevrStatus = lib.lookupFunction<_GetStrNative, _GetStrDart>(
            'mocoslime_get_slimevr_status');
      } catch (_) {
        _getSlimevrStatus = null;
      }
      // T3 Mount-Flip: optional.
      try {
        _setMountFlip = lib
            .lookupFunction<_MountNative, _MountDart>('mocoslime_set_mount_flip');
      } catch (_) {
        _setMountFlip = null;
      }
      try {
        _isScanning =
            lib.lookupFunction<_ApiNative, _ApiDart>('mocoslime_is_scanning');
      } catch (_) {
        _isScanning = null;
      }
      try {
        _resendHandshake = lib.lookupFunction<_NoArgNative, _NoArgDart>(
            'mocoslime_resend_handshake');
      } catch (_) {
        _resendHandshake = null;
      }

      // Verify API version (v1 Basis, v2 Pose, v3 Re-Init+Stats).
      // Neue GUI läuft mit allen >=1; fehlende v2/v3-Symbole sind No-ops.
      final apiVer = _apiVersion?.call() ?? -1;
      if (apiVer < 1) {
        _lastInitError =
            'Incompatible native API version: $apiVer (expected >= 1)';
        _lib = null;
        _initialized = true;
        return;
      }
      if (apiVer < 3) {
        log('NativeBridge: old API v$apiVer (pose/event-stats limited)');
      }
      _lastInitError = null;
    } catch (e) {
      _lastInitError = 'Failed to load native bridge: $e';
      _lib = null;
    }
    _initialized = true;
  }

  String? _takeString(Pointer<Utf8> ptr) {
    if (ptr == nullptr || _freeString == null) return null;
    try {
      return ptr.toDartString();
    } finally {
      _freeString!(ptr);
    }
  }

  Pointer<Utf8> _toNative(String value) {
    return value.toNativeUtf8();
  }

  static bool _checkNative() {
    if (!isNativeAvailable) {
      log('NativeBridge: native library not available (${_lastInitError ?? "unknown"})');
      return false;
    }
    return true;
  }

  /// Retry bei `ChannelFull` (Code 4): der Rust-Channel ist 256 tief,
  /// aber Full-Body-Kalibrierung feuert Bursts. 3 Versuche mit Backoff.
  static Future<int> _sendWithRetry(int Function() call) async {
    int code = call();
    for (int i = 0; i < 2 && code == 4; i++) {
      await Future.delayed(Duration(milliseconds: 50 * (i + 1)));
      code = call();
    }
    if (code == 4) {
      log('NativeBridge: command channel full after retry');
    }
    return code;
  }

  /// Feature-Flags aus der API-Version (v1 Basis, v2 Pose, v3 Re-Init+Stats).
  bool get isPoseSupported => apiVersion() >= 2;
  bool get isEventStatsSupported => apiVersion() >= 3;

  Map<String, dynamic>? getEventStatsRaw() {
    if (!_checkNative()) return null;
    final fn = _getEventStats;
    if (fn == null) return null;
    final raw = _takeString(fn());
    if (raw == null || raw.isEmpty) return null;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map) return Map<String, dynamic>.from(decoded);
      return null;
    } catch (_) {
      return null;
    }
  }

  /// Leichter Bluetooth-Adapter-Check `{ok, error}` oder null (unbekannt).
  Map<String, dynamic>? getAdapterStatusRaw() {
    if (!_checkNative()) return null;
    final fn = _getAdapterStatus;
    if (fn == null) return null;
    final raw = _takeString(fn());
    if (raw == null || raw.isEmpty) return null;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map) return Map<String, dynamic>.from(decoded);
      return null;
    } catch (_) {
      return null;
    }
  }

  Map<String, dynamic>? getSlimevrStatusRaw() {
    if (!_checkNative()) return null;
    final fn = _getSlimevrStatus;
    if (fn == null) return null;
    final raw = _takeString(fn());
    if (raw == null || raw.isEmpty) return null;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map) return Map<String, dynamic>.from(decoded);
      return null;
    } catch (_) {
      return null;
    }
  }

  Future<int> setMountFlip(String deviceId, bool flip) async {
    if (!_checkNative()) return 1;
    final fn = _setMountFlip;
    if (fn == null) return 0;
    final ptr = _toNative(deviceId);
    try {
      return await _sendWithRetry(() => fn(ptr, flip ? 1 : 0));
    } finally {
      calloc.free(ptr);
    }
  }

  Future<int> resendHandshake() async {
    if (!_checkNative()) return 1;
    final fn = _resendHandshake;
    if (fn == null) return 0;
    return _sendWithRetry(() => fn());
  }

  /// 1 = Scan läuft, 0 = idle, -1 = unbekannt (alte DLL/ohne Core).
  int isScanningNative() {
    if (!_checkNative()) return -1;
    return _isScanning?.call() ?? -1;
  }

  Future<int> init(String configJson) async {
    if (!_checkNative()) return 1; // NotInitialized
    final fn = _init;
    if (fn == null) return 1;
    final ptr = _toNative(configJson);
    try {
      return fn(ptr);
    } finally {
      calloc.free(ptr);
    }
  }

  Future<int> shutdown() async {
    if (!_checkNative()) return 1;
    return _shutdown?.call() ?? 1;
  }

  Future<int> scanDevices() async {
    if (!_checkNative()) return 1;
    final fn = _scan;
    if (fn == null) return 1;
    return _sendWithRetry(() => fn());
  }

  Future<int> connectDevice(String deviceId) async {
    if (!_checkNative()) return 1;
    final fn = _connect;
    if (fn == null) return 1;
    final ptr = _toNative(deviceId);
    try {
      return await _sendWithRetry(() => fn(ptr));
    } finally {
      calloc.free(ptr);
    }
  }

  Future<int> disconnectDevice(String deviceId) async {
    if (!_checkNative()) return 1;
    final fn = _disconnect;
    if (fn == null) return 1;
    final ptr = _toNative(deviceId);
    try {
      return fn(ptr);
    } finally {
      calloc.free(ptr);
    }
  }

  Future<int> assignTracker(String deviceId, int role) async {
    if (!_checkNative()) return 1;
    final fn = _assign;
    if (fn == null) return 1;
    final ptr = _toNative(deviceId);
    try {
      return await _sendWithRetry(() => fn(ptr, role));
    } finally {
      calloc.free(ptr);
    }
  }

  Future<int> setActivePose(String pose) async {
    if (!_checkNative()) return 1;
    final fn = _setActivePose;
    // Ohne v2-DLL: stiller No-op (Erfolg), damit alte Builds nicht brechen.
    if (fn == null) return 0;
    final ptr = _toNative(pose);
    try {
      return fn(ptr);
    } finally {
      calloc.free(ptr);
    }
  }

  Future<int> setAutoPose(bool enabled) async {
    if (!_checkNative()) return 1;
    final fn = _setAutoPose;
    if (fn == null) return 0;
    return fn(enabled ? 1 : 0);
  }

  Future<int> setSkeleton(String skeletonJson) async {
    if (!_checkNative()) return 1;
    final fn = _setSkeleton;
    if (fn == null) return 0;
    final ptr = _toNative(skeletonJson);
    try {
      return fn(ptr);
    } finally {
      calloc.free(ptr);
    }
  }

  Map<String, dynamic>? getPoseStatusRaw() {
    if (!_checkNative()) return null;
    final fn = _getPoseStatus;
    if (fn == null) return null;
    final raw = _takeString(fn());
    if (raw == null || raw.isEmpty) return null;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map) return Map<String, dynamic>.from(decoded);
      return null;
    } catch (_) {
      return null;
    }
  }

  Future<int> startStreaming() async {
    if (!_checkNative()) return 1;
    final fn = _startStreaming;
    if (fn == null) return 1;
    return _sendWithRetry(() => fn());
  }

  Future<int> stopStreaming() async {
    if (!_checkNative()) return 1;
    return _stopStreaming?.call() ?? 1;
  }

  Future<int> setSlimevrAddress(String ip, int port) async {
    if (!_checkNative()) return 1;
    final fn = _setSlime;
    if (fn == null) return 1;
    final ptr = _toNative(ip);
    try {
      return fn(ptr, port);
    } finally {
      calloc.free(ptr);
    }
  }

  Future<int> setConfig(String configJson) async {
    if (!_checkNative()) return 1;
    final fn = _setConfig;
    if (fn == null) return 1;
    final ptr = _toNative(configJson);
    try {
      return fn(ptr);
    } finally {
      calloc.free(ptr);
    }
  }

  Future<int> setLanguage(String language) async {
    if (!_checkNative()) return 1;
    final fn = _setLanguage;
    // Keep older FFI DLLs usable; the UI still changes, but they cannot
    // persist this setting independently.
    if (fn == null) return 0;
    final ptr = _toNative(language);
    try {
      return fn(ptr);
    } finally {
      calloc.free(ptr);
    }
  }

  /// Load the on-disk config into the native core (settings + role
  /// assignments survive restarts). Returns 0 on success; missing
  /// symbol on older DLLs counts as success (nothing to load).
  Future<int> loadConfig() async {
    if (!_checkNative()) return 1;
    final fn = _loadConfig;
    if (fn == null) return 0;
    return fn();
  }

  /// Raw Rust AppConfig JSON (nested shape) or null when unavailable.
  String? getConfigRaw() {
    if (!_checkNative()) return null;
    final fn = _getConfig;
    if (fn == null) return null;
    return _takeString(fn());
  }

  /// Raw Rust TrackerStatus list JSON or null when unavailable.
  String? getDevicesRaw() {
    if (!_checkNative()) return null;
    final fn = _getDevices;
    if (fn == null) return null;
    return _takeString(fn());
  }

  /// Parsed tracker list. Returns empty (never fabricated entries) when
  /// the native library is missing or returns invalid JSON.
  List<TrackerStatus> getDevices() {
    final raw = getDevicesRaw();
    if (raw == null || raw.isEmpty) return [];
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! List) return [];
      return decoded
          .whereType<Map>()
          .map((e) => TrackerStatus.fromJson(Map<String, dynamic>.from(e)))
          .toList();
    } catch (_) {
      return [];
    }
  }

  Map<String, dynamic>? getDeviceRaw(String deviceId) {
    if (!_checkNative()) return null;
    final fn = _getDevice;
    if (fn == null) return null;
    final ptr = _toNative(deviceId);
    try {
      final raw = _takeString(fn(ptr));
      if (raw == null || raw.isEmpty || raw == 'null') return null;
      final decoded = jsonDecode(raw);
      if (decoded is Map) return Map<String, dynamic>.from(decoded);
      return null;
    } catch (_) {
      return null;
    } finally {
      calloc.free(ptr);
    }
  }

  /// Next pending Rust CoreEvent, or null when the queue is empty.
  CoreEvent? pollEvent() {
    if (!_checkNative()) return null;
    final fn = _pollEvent;
    if (fn == null) return null;
    final raw = _takeString(fn());
    if (raw == null || raw.isEmpty) return null;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map) {
        return CoreEvent.fromJson(Map<String, dynamic>.from(decoded));
      }
      return null;
    } catch (_) {
      return null;
    }
  }

  String getVersion() {
    if (!_checkNative()) return 'unknown';
    final fn = _getVersion;
    if (fn == null) return 'unknown';
    return _takeString(fn()) ?? 'unknown';
  }

  Future<int> setBluetoothConfig(String configJson) async {
    if (!_checkNative()) return 1;
    final fn = _setBluetoothConfig;
    if (fn == null) return 1;
    final ptr = _toNative(configJson);
    try {
      return fn(ptr);
    } finally {
      calloc.free(ptr);
    }
  }

  int apiVersion() {
    if (!_checkNative()) return -1;
    return _apiVersion?.call() ?? -1;
  }

  Future<int> setLogLevel(String level) async {
    if (!_checkNative()) return 1;
    final fn = _setLogLevel;
    if (fn == null) return 1;
    final ptr = _toNative(level);
    try {
      return fn(ptr);
    } finally {
      calloc.free(ptr);
    }
  }

  /// Drain buffered Rust log entries (decoded JSON array).
  List<LogEntry> pollLogs() {
    if (!_checkNative()) return [];
    final fn = _pollLogs;
    if (fn == null) return [];
    final raw = _takeString(fn());
    if (raw == null || raw.isEmpty) return [];
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! List) return [];
      return decoded
          .whereType<Map>()
          .map((e) => LogEntry.fromJson(Map<String, dynamic>.from(e)))
          .toList();
    } catch (_) {
      return [];
    }
  }

  /// Support bundle from Rust (`mocoslime_get_diagnostics`): version, OS,
  /// Bluetooth adapter state, config, trackers, recent logs. Null when
  /// the native library (or an older DLL) does not provide it.
  Map<String, dynamic>? getDiagnosticsRaw() {
    if (!_checkNative()) return null;
    final fn = _getDiagnostics;
    if (fn == null) return null;
    final raw = _takeString(fn());
    if (raw == null || raw.isEmpty) return null;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is Map) return Map<String, dynamic>.from(decoded);
      return null;
    } catch (_) {
      return null;
    }
  }

  Future<int> setAutostart(bool enabled) async {
    if (!_checkNative()) return 1;
    return _setAutostart?.call(enabled ? 1 : 0) ?? 1;
  }

  /// 1 = enabled, 0 = disabled, -1 = unknown/error.
  int isAutostart() {
    if (!_checkNative()) return -1;
    return _isAutostart?.call() ?? -1;
  }
}
