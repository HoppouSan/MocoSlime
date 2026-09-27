// Models for Mocoslime. JSON shapes match the Rust side:
// - TrackerStatus / BatteryInfo: snake_case fields from
//   mocopi-ble-windows (ConnectionState serializes as "Streaming", ...).
// - TrackerRole: human labels ("Left Foot") via Rust serde renames.
// - AppConfig (Rust): nested {general, bluetooth, slimevr, tracking,
//   trackers, version}. The flat AppSettings below is the UI-friendly view
//   with explicit toRustJson/fromRustJson conversion (no data loss for
//   unknown trackers/version when round-tripping through `existing`).
// - CoreEvent (Rust): externally-tagged enum, e.g.
//   {"DeviceConnected":"<uuid>"} or {"TrackingData":["<uuid>",{...}]}.

/// FFI error codes matching Rust `FfiError` (crates/ffi/src/error.rs).
class FfiError {
  static const int success = 0;
  static const int notInitialized = 1;
  static const int invalidInput = 2;
  static const int serializationError = 3;
  static const int channelFull = 4;
  static const int internalError = 5;
  static const int alreadyInitialized = 6;

  static String fromCode(int code) {
    switch (code) {
      case success:
        return 'Success';
      case notInitialized:
        return 'Not initialized (call mocoslime_init first)';
      case invalidInput:
        return 'Invalid input (bad UUID/JSON)';
      case serializationError:
        return 'Serialization error';
      case channelFull:
        return 'Command channel full';
      case internalError:
        return 'Internal error';
      case alreadyInitialized:
        return 'Already initialized';
      default:
        return 'Unknown error ($code)';
    }
  }
}

class TrackerStatus {
  final String deviceId;
  final String bluetoothAddress;
  final String name;
  final String state;
  final String? assignedRole;
  final BatteryInfo? battery;
  final int rssi;
  final String? firmwareVersion;
  final int packetCount;
  final int? lastPacketNs;
  final int reconnectAttempts;
  final int? reconnectInMs;
  final String? error;

  TrackerStatus({
    required this.deviceId,
    required this.bluetoothAddress,
    required this.name,
    required this.state,
    this.assignedRole,
    this.battery,
    required this.rssi,
    this.firmwareVersion,
    required this.packetCount,
    this.lastPacketNs,
    required this.reconnectAttempts,
    this.reconnectInMs,
    this.error,
  });

  factory TrackerStatus.fromJson(Map<String, dynamic> json) {
    return TrackerStatus(
      deviceId: (json['device_id'] ?? '').toString(),
      bluetoothAddress: (json['bluetooth_address'] ?? '').toString(),
      name: (json['name'] ?? '').toString(),
      state: (json['state'] ?? 'Disconnected').toString(),
      assignedRole: json['assigned_role']?.toString(),
      battery: json['battery'] is Map
          ? BatteryInfo.fromJson(
              Map<String, dynamic>.from(json['battery'] as Map))
          : null,
      rssi: (json['rssi'] is num) ? (json['rssi'] as num).toInt() : 0,
      firmwareVersion: json['firmware_version']?.toString(),
      packetCount: (json['packet_count'] is num)
          ? (json['packet_count'] as num).toInt()
          : 0,
      lastPacketNs: (json['last_packet_ns'] is num)
          ? (json['last_packet_ns'] as num).toInt()
          : null,
      reconnectAttempts: (json['reconnect_attempts'] is num)
          ? (json['reconnect_attempts'] as num).toInt()
          : 0,
      reconnectInMs: (json['reconnect_in_ms'] is num)
          ? (json['reconnect_in_ms'] as num).toInt()
          : null,
      error: json['error']?.toString(),
    );
  }

  bool get isConnected => state == 'Streaming';
  bool get isConnecting => state == 'Connecting' || state == 'Initializing';
  bool get isReconnecting => state == 'Reconnecting';
  bool get hasError => state == 'Error' || error != null;

  Duration? get lastPacketAge {
    final timestamp = lastPacketNs;
    if (timestamp == null) return null;
    final packetAt = DateTime.fromMillisecondsSinceEpoch(
      timestamp ~/ 1000000,
      isUtc: true,
    );
    final age = DateTime.now().toUtc().difference(packetAt);
    return age.isNegative ? Duration.zero : age;
  }

  bool get hasFreshTrackingData {
    final age = lastPacketAge;
    return age != null && age <= const Duration(seconds: 1);
  }
}

class BatteryInfo {
  final double voltage;
  final double percentage;

  BatteryInfo({
    required this.voltage,
    required this.percentage,
  });

  factory BatteryInfo.fromJson(Map<String, dynamic> json) {
    return BatteryInfo(
      voltage:
          (json['voltage'] is num) ? (json['voltage'] as num).toDouble() : 0.0,
      percentage: (json['percentage'] is num)
          ? (json['percentage'] as num).toDouble()
          : 0.0,
    );
  }
}

class TrackerRole {
  static const List<String> allRoles = [
    'Head',
    'Chest',
    'Waist',
    'Left Upper Arm',
    'Right Upper Arm',
    'Left Lower Arm',
    'Right Lower Arm',
    'Left Hand',
    'Right Hand',
    'Left Upper Leg',
    'Right Upper Leg',
    'Left Lower Leg',
    'Right Lower Leg',
    'Left Foot',
    'Right Foot',
  ];

  static int getSensorId(String role) {
    return allRoles.indexOf(role);
  }
}

class AppSettings {
  bool startWithWindows;
  bool minimizeToTray;
  bool startMinimized;
  String language;
  String logLevel;
  bool bluetoothAutoScan;
  bool bluetoothAutoConnect;
  int bluetoothReconnectDelayMs;
  int bluetoothConnectionTimeoutMs;
  int bluetoothMaxTrackers;
  int bluetoothRssiMinDbm;
  String slimevrServerIp;
  int slimevrServerPort;
  bool slimevrEnabled;
  int slimevrPacketRate;
  String trackingCoordinateSystem;
  bool trackingEnableFiltering;
  double trackingFilterSlerpFactor;
  double trackingFilterAccelAlpha;
  String trackingActivePose;
  bool trackingEnableAutoPose;
  double skeletonHeightCm;

  AppSettings({
    this.startWithWindows = false,
    this.minimizeToTray = true,
    this.startMinimized = false,
    this.language = 'en',
    this.logLevel = 'INFO',
    this.bluetoothAutoScan = true,
    this.bluetoothAutoConnect = true,
    this.bluetoothReconnectDelayMs = 5000,
    this.bluetoothConnectionTimeoutMs = 15000,
    this.bluetoothMaxTrackers = 12,
    this.bluetoothRssiMinDbm = -85,
    this.slimevrServerIp = '127.0.0.1',
    this.slimevrServerPort = 6969,
    this.slimevrEnabled = true,
    this.slimevrPacketRate = 128,
    this.trackingCoordinateSystem = 'SlimeVR',
    this.trackingEnableFiltering = true,
    this.trackingFilterSlerpFactor = 0.1,
    this.trackingFilterAccelAlpha = 0.2,
    this.trackingActivePose = 'standing',
    this.trackingEnableAutoPose = true,
    this.skeletonHeightCm = 175.0,
  });

  /// Flat legacy shape (kept for existing UI tests).
  factory AppSettings.fromJson(Map<String, dynamic> json) {
    // Accept both the legacy flat shape and the nested Rust AppConfig.
    if (json.containsKey('general') || json.containsKey('bluetooth')) {
      return AppSettings.fromRustJson(json);
    }
    return AppSettings(
      startWithWindows: json['start_with_windows'] == true,
      minimizeToTray: json['minimize_to_tray'] != false,
      startMinimized: json['start_minimized'] == true,
      language: normalizeLanguage(json['language']),
      logLevel: (json['log_level'] ?? 'INFO').toString(),
      bluetoothAutoScan: json['bluetooth_auto_scan'] != false,
      bluetoothAutoConnect: json['bluetooth_auto_connect'] != false,
      bluetoothReconnectDelayMs:
          _toInt(json['bluetooth_reconnect_delay_ms'], 5000),
      bluetoothConnectionTimeoutMs:
          _toInt(json['bluetooth_connection_timeout_ms'], 15000),
      bluetoothMaxTrackers: _toInt(json['bluetooth_max_trackers'], 12),
      bluetoothRssiMinDbm: _toInt(json['bluetooth_rssi_min_dbm'], -85),
      slimevrServerIp: (json['slimevr_server_ip'] ?? '127.0.0.1').toString(),
      slimevrServerPort: _toInt(json['slimevr_server_port'], 6969),
      slimevrEnabled: json['slimevr_enabled'] != false,
      slimevrPacketRate: _toInt(json['slimevr_packet_rate'], 128),
      trackingCoordinateSystem:
          (json['tracking_coordinate_system'] ?? 'SlimeVR').toString(),
      trackingEnableFiltering: json['tracking_enable_filtering'] != false,
      trackingFilterSlerpFactor:
          _toDouble(json['tracking_filter_slerp_factor'], 0.1),
      trackingFilterAccelAlpha:
          _toDouble(json['tracking_filter_accel_alpha'], 0.2),
    );
  }

  /// Read the nested Rust AppConfig shape.
  factory AppSettings.fromRustJson(Map<String, dynamic> json) {
    final general = json['general'] is Map
        ? Map<String, dynamic>.from(json['general'] as Map)
        : <String, dynamic>{};
    final bluetooth = json['bluetooth'] is Map
        ? Map<String, dynamic>.from(json['bluetooth'] as Map)
        : <String, dynamic>{};
    final slimevr = json['slimevr'] is Map
        ? Map<String, dynamic>.from(json['slimevr'] as Map)
        : <String, dynamic>{};
    final tracking = json['tracking'] is Map
        ? Map<String, dynamic>.from(json['tracking'] as Map)
        : <String, dynamic>{};
    final skeleton = tracking['skeleton'] is Map
        ? Map<String, dynamic>.from(tracking['skeleton'] as Map)
        : <String, dynamic>{};
    return AppSettings(
      startWithWindows: general['start_with_windows'] == true,
      minimizeToTray: general['minimize_to_tray'] != false,
      startMinimized: general['start_minimized'] == true,
      language: normalizeLanguage(general['language']),
      logLevel: (general['log_level'] ?? 'INFO').toString(),
      bluetoothAutoScan: bluetooth['auto_scan'] != false,
      bluetoothAutoConnect: bluetooth['auto_connect'] != false,
      bluetoothReconnectDelayMs: _toInt(bluetooth['reconnect_delay_ms'], 5000),
      bluetoothConnectionTimeoutMs:
          _toInt(bluetooth['connection_timeout_ms'], 15000),
      bluetoothMaxTrackers: _toInt(bluetooth['max_trackers'], 12),
      bluetoothRssiMinDbm: _toInt(bluetooth['rssi_min_dbm'], -85),
      slimevrServerIp: (slimevr['server_ip'] ?? '127.0.0.1').toString(),
      slimevrServerPort: _toInt(slimevr['server_port'], 6969),
      slimevrEnabled: slimevr['enabled'] != false,
      slimevrPacketRate: _toInt(slimevr['packet_rate'], 128),
      trackingCoordinateSystem:
          (tracking['coordinate_system'] ?? 'SlimeVR').toString(),
      trackingEnableFiltering: tracking['enable_filtering'] != false,
      trackingFilterSlerpFactor:
          _toDouble(tracking['filter_slerp_factor'], 0.1),
      trackingFilterAccelAlpha: _toDouble(tracking['filter_accel_alpha'], 0.2),
      trackingActivePose: (tracking['active_pose'] ?? 'standing').toString(),
      trackingEnableAutoPose: tracking['enable_auto_pose'] != false,
      skeletonHeightCm: _toDouble(skeleton['height_cm'], 175.0),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'start_with_windows': startWithWindows,
      'minimize_to_tray': minimizeToTray,
      'start_minimized': startMinimized,
      'language': language,
      'log_level': logLevel,
      'bluetooth_auto_scan': bluetoothAutoScan,
      'bluetooth_auto_connect': bluetoothAutoConnect,
      'bluetooth_reconnect_delay_ms': bluetoothReconnectDelayMs,
      'bluetooth_connection_timeout_ms': bluetoothConnectionTimeoutMs,
      'bluetooth_max_trackers': bluetoothMaxTrackers,
      'bluetooth_rssi_min_dbm': bluetoothRssiMinDbm,
      'slimevr_server_ip': slimevrServerIp,
      'slimevr_server_port': slimevrServerPort,
      'slimevr_enabled': slimevrEnabled,
      'slimevr_packet_rate': slimevrPacketRate,
      'tracking_coordinate_system': trackingCoordinateSystem,
      'tracking_enable_filtering': trackingEnableFiltering,
      'tracking_filter_slerp_factor': trackingFilterSlerpFactor,
      'tracking_filter_accel_alpha': trackingFilterAccelAlpha,
    };
  }

  /// Nested Rust AppConfig shape. Pass the last known full config as
  /// `existing` to preserve `trackers` and `version` across saves.
  Map<String, dynamic> toRustJson([Map<String, dynamic>? existing]) {
    final trackers = existing?['trackers'];
    final version = existing?['version'];
    final existingSlimevr = existing?['slimevr'] is Map
        ? Map<String, dynamic>.from(existing!['slimevr'] as Map)
        : <String, dynamic>{};
    // Skeleton/Auto-Pose aus der bestehenden Config übernehmen, damit
    // saveSettings() sie nicht versehentlich auf Defaults zurücksetzt.
    final existingTracking = existing?['tracking'] is Map
        ? Map<String, dynamic>.from(existing!['tracking'] as Map)
        : <String, dynamic>{};
    final existingSkeleton = existingTracking['skeleton'];
    final existingPoseDetector = existingTracking['pose_detector'];
    return {
      'general': {
        'start_with_windows': startWithWindows,
        'minimize_to_tray': minimizeToTray,
        'start_minimized': startMinimized,
        'language': language,
        'log_level': logLevel,
      },
      'bluetooth': {
        'auto_scan': bluetoothAutoScan,
        'auto_connect': bluetoothAutoConnect,
        'reconnect_delay_ms': bluetoothReconnectDelayMs,
        'connection_timeout_ms': bluetoothConnectionTimeoutMs,
        'max_trackers': bluetoothMaxTrackers,
        'scan_duration_ms': 10000,
        'rssi_min_dbm': bluetoothRssiMinDbm,
      },
      'slimevr': {
        'server_ip': slimevrServerIp,
        'server_port': slimevrServerPort,
        'enabled': slimevrEnabled,
        'packet_rate': slimevrPacketRate,
        'auto_discover':
            existingSlimevr['server_ip']?.toString() == slimevrServerIp &&
                    _toInt(existingSlimevr['server_port'], slimevrServerPort) ==
                        slimevrServerPort
                ? existingSlimevr['auto_discover'] != false
                : false,
      },
      'tracking': {
        'coordinate_system': trackingCoordinateSystem,
        'enable_filtering': trackingEnableFiltering,
        'filter_slerp_factor': trackingFilterSlerpFactor,
        'filter_accel_alpha': trackingFilterAccelAlpha,
        'active_pose': trackingActivePose,
        'enable_auto_pose': trackingEnableAutoPose,
        'skeleton': existingSkeleton is Map
            ? (Map<String, dynamic>.from(existingSkeleton)
              ..['height_cm'] = skeletonHeightCm)
            : {'height_cm': skeletonHeightCm, 'auto_estimate': true},
        if (existingPoseDetector is Map) 'pose_detector': existingPoseDetector,
      },
      'trackers': trackers ?? <String, dynamic>{},
      'version': version ?? 3,
    };
  }

  static String normalizeLanguage(dynamic value) {
    final language = (value?.toString() ?? 'en').trim().replaceAll('_', '-');
    final normalized = language.toLowerCase();
    if (normalized == 'zh' || normalized == 'zh-cn' ||
        normalized == 'zh-hans' || normalized == 'zh-sg') {
      return 'zh-CN';
    }
    final baseLanguage = normalized.split('-').first;
    return const {'en', 'de', 'ja'}.contains(baseLanguage)
        ? baseLanguage
        : 'en';
  }

  static int _toInt(dynamic v, int fallback) {
    if (v is num) return v.toInt();
    return int.tryParse(v?.toString() ?? '') ?? fallback;
  }

  static double _toDouble(dynamic v, double fallback) {
    if (v is num) return v.toDouble();
    return double.tryParse(v?.toString() ?? '') ?? fallback;
  }
}

class SlimevrStatus {
  final bool enabled;
  final String? ip;
  final int? port;
  final bool autoDiscovered;
  final int packetsSent;
  final int errors;

  SlimevrStatus({
    required this.enabled,
    this.ip,
    this.port,
    required this.autoDiscovered,
    required this.packetsSent,
    required this.errors,
  });

  factory SlimevrStatus.fromJson(Map<String, dynamic> json) {
    int? toIntOpt(dynamic v) {
      if (v == null) return null;
      if (v is num) return v.toInt();
      return int.tryParse(v.toString());
    }

    int toInt(dynamic v, int fallback) =>
        v is num ? v.toInt() : int.tryParse(v?.toString() ?? '') ?? fallback;
    final ip = json['ip']?.toString();
    return SlimevrStatus(
      enabled: json['enabled'] != false,
      ip: (ip == null || ip.isEmpty || ip == 'null') ? null : ip,
      port: toIntOpt(json['port']),
      autoDiscovered: json['auto_discovered'] == true,
      packetsSent: toInt(json['packets_sent'], 0),
      errors: toInt(json['errors'], 0),
    );
  }

  String get targetLabel {
    if (ip == null || port == null) return 'No target';
    return '$ip:$port${autoDiscovered ? ' (Auto)' : ''}';
  }
}

/// One Rust log line (see `mocoslime-core::logging::LogEntry`).
class LogEntry {
  final String timestamp;
  final String level;
  final String target;
  final String message;

  LogEntry({
    required this.timestamp,
    required this.level,
    required this.target,
    required this.message,
  });

  factory LogEntry.fromJson(Map<String, dynamic> json) {
    return LogEntry(
      timestamp: (json['timestamp'] ?? '').toString(),
      level: (json['level'] ?? 'INFO').toString(),
      target: (json['target'] ?? '').toString(),
      message: (json['message'] ?? '').toString(),
    );
  }
}

/// Rust `CoreEvent` externally-tagged enum, e.g.
/// `{"DeviceConnected":"<uuid>"}`,
/// `{"BatteryUpdated":["<uuid>",{"voltage":3.7,"percentage":0.8}]}`,
/// `{"ScanCompleted":6}`,
/// `{"Error":"message"}`.
class CoreEvent {
  final String kind;
  final String? deviceId;
  final Map<String, dynamic>? status;
  final Map<String, dynamic>? battery;
  final String? message;
  final int? count;
  final String? pose;
  final double? confidence;
  final Map<String, dynamic>? trackingSample;
  final List<double>? position;

  CoreEvent({
    required this.kind,
    this.deviceId,
    this.status,
    this.battery,
    this.message,
    this.count,
    this.pose,
    this.confidence,
    this.trackingSample,
    this.position,
  });

  static String? _asPose(dynamic v) {
    if (v == null) return null;
    final s = v.toString().toLowerCase();
    if (s == 'standing' || s == 'sitting' || s == 'lying') return s;
    return null;
  }

  static double? _asDouble(dynamic v) {
    if (v is num) return v.toDouble();
    return double.tryParse(v?.toString() ?? '');
  }

  factory CoreEvent.fromJson(Map<String, dynamic> json) {
    if (json.length != 1) {
      return CoreEvent(kind: 'Unknown');
    }
    final kind = json.keys.first;
    final value = json[kind];
    switch (kind) {
      case 'DeviceDiscovered':
      case 'DeviceUpdated':
        if (value is Map) {
          final status = Map<String, dynamic>.from(value);
          final id = (status['device_id'] ?? '').toString();
          return CoreEvent(
              kind: kind, deviceId: id.isEmpty ? null : id, status: status);
        }
        return CoreEvent(kind: kind);
      case 'DeviceConnected':
      case 'DeviceDisconnected':
      case 'ActivePoseChanged':
        // Neu: "sitting" als String.
        return CoreEvent(kind: kind, pose: _asPose(value));
      case 'AutoPoseChanged':
        // Neu: ["sitting", 0.92].
        if (value is List && value.isNotEmpty) {
          return CoreEvent(
            kind: kind,
            pose: _asPose(value[0]),
            confidence: value.length > 1 ? _asDouble(value[1]) : null,
          );
        }
        return CoreEvent(kind: kind, pose: _asPose(value));
      case 'BatteryUpdated':
        if (value is List && value.length == 2) {
          final battery = value[1] is Map
              ? Map<String, dynamic>.from(value[1] as Map)
              : null;
          return CoreEvent(
              kind: kind, deviceId: value[0]?.toString(), battery: battery);
        }
        return CoreEvent(kind: kind);
      case 'TrackingData':
        if (value is List && value.isNotEmpty) {
          final sample = value.length > 1 && value[1] is Map
              ? Map<String, dynamic>.from(value[1] as Map)
              : null;
          return CoreEvent(
              kind: kind,
              deviceId: value[0]?.toString(),
              trackingSample: sample);
        }
        return CoreEvent(kind: kind);
      case 'TrackingPosition':
        if (value is List && value.isNotEmpty) {
          final raw =
              value.length > 1 && value[1] is List ? value[1] as List : null;
          return CoreEvent(
            kind: kind,
            deviceId: value[0]?.toString(),
            position: raw != null && raw.length >= 3
                ? raw.take(3).map((v) => _asDouble(v) ?? 0).toList()
                : null,
          );
        }
        return CoreEvent(kind: kind);
      case 'Error':
        return CoreEvent(kind: kind, message: value?.toString());
      case 'ScanCompleted':
        int? count;
        if (value is num) {
          count = value.toInt();
        } else {
          count = int.tryParse(value?.toString() ?? '');
        }
        return CoreEvent(kind: kind, count: count);
      case 'SlimeVRConnected':
        // Neu v3: ["127.0.0.1", 6969, true] — alt: unit (null).
        if (value is List && value.isNotEmpty) {
          return CoreEvent(
            kind: kind,
            message: value.isNotEmpty ? value[0]?.toString() : null,
            count: value.length > 1
                ? (value[1] is num
                    ? (value[1] as num).toInt()
                    : int.tryParse(value[1]?.toString() ?? ''))
                : null,
            confidence: value.length > 2 && value[2] == true ? 1.0 : 0.0,
          );
        }
        return CoreEvent(kind: kind);
      case 'SlimeVRDiscovered':
        if (value is List && value.isNotEmpty) {
          return CoreEvent(
            kind: kind,
            message: value[0]?.toString(),
            count: value.length > 1
                ? (value[1] is num
                    ? (value[1] as num).toInt()
                    : int.tryParse(value[1]?.toString() ?? ''))
                : null,
          );
        }
        return CoreEvent(kind: kind, message: value?.toString());
      case 'SlimeVRDisconnected':
        return CoreEvent(kind: kind);
      default:
        return CoreEvent(kind: 'Unknown');
    }
  }
}
