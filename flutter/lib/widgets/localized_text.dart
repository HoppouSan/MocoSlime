import 'package:flutter/material.dart';

/// Lightweight UI translation layer. English is the source language and
/// fallback; keep keys as complete phrases so dynamic values remain intact.
class LocalizedText extends StatelessWidget {
  final String data;
  final TextStyle? style;
  final TextAlign? textAlign;
  final TextOverflow? overflow;
  final int? maxLines;
  final bool? softWrap;

  const LocalizedText(
    this.data, {
    super.key,
    this.style,
    this.textAlign,
    this.overflow,
    this.maxLines,
    this.softWrap,
  });

  @override
  Widget build(BuildContext context) {
    final locale = Localizations.localeOf(context);
    final language = locale.countryCode == 'CN' ? 'zh-CN' : locale.languageCode;
    final translated = localizeUiString(language, data);
    return Text(
      translated,
      style: style,
      textAlign: textAlign,
      overflow: overflow,
      maxLines: maxLines,
      softWrap: softWrap,
    );
  }
}

String localizeUiString(String language, String value) {
  final translated = _translations[language]?[value];
  if (translated != null) return translated;
  final trackerCount = RegExp(r'^Trackers \((\d+)/(\d+)\)$').firstMatch(value);
  if (trackerCount != null) {
    final count = '${trackerCount[1]}/${trackerCount[2]}';
    return switch (language) {
      'de' => 'Tracker ($count)',
      'ja' => 'トラッカー ($count)',
      'zh-CN' => '追踪器 ($count)',
      _ => value,
    };
  }
  final signal = RegExp(r'^Min\. Signal: (.+)$').firstMatch(value);
  if (signal != null) {
    return switch (language) {
      'de' => 'Min. Signalstärke: ${signal[1]}',
      'ja' => '最小信号強度: ${signal[1]}',
      'zh-CN' => '最低信号强度：${signal[1]}',
      _ => value,
    };
  }
  final output =
      RegExp(r'^(.*?) • (\d+) packets • (\d+) errors$').firstMatch(value);
  if (output != null && language != 'en') {
    final address = output[1];
    final packets = output[2];
    final errors = output[3];
    return switch (language) {
      'de' => '$address • $packets Pakete • $errors Fehler',
      'ja' => '$address • $packets パケット • $errors エラー',
      'zh-CN' => '$address • $packets 个数据包 • $errors 个错误',
      _ => value,
    };
  }
  final roles = RegExp(
          r'^(\d+) connected • (\d+) without a role • (\d+) duplicate roles$')
      .firstMatch(value);
  if (roles != null && language != 'en') {
    return switch (language) {
      'de' =>
        '${roles[1]} verbunden • ${roles[2]} ohne Rolle • ${roles[3]} doppelte Rollen',
      'ja' => '${roles[1]} 台接続済み • ${roles[2]} 台ロール未設定 • 重複ロール ${roles[3]} 件',
      'zh-CN' => '已连接 ${roles[1]} 个 • 未分配角色 ${roles[2]} 个 • 重复角色 ${roles[3]} 个',
      _ => value,
    };
  }
  final packetAge = RegExp(r'^Live • (\d+) ms ago$').firstMatch(value);
  if (packetAge != null && language != 'en') {
    return switch (language) {
      'de' => 'Live • vor ${packetAge[1]} ms',
      'ja' => 'ライブ • ${packetAge[1]} ms 前',
      'zh-CN' => '实时 • ${packetAge[1]} 毫秒前',
      _ => value,
    };
  }
  final delayedAge = RegExp(r'^Delayed • (\d+)s ago$').firstMatch(value);
  if (delayedAge != null && language != 'en') {
    return switch (language) {
      'de' => 'Verzögert • vor ${delayedAge[1]} s',
      'ja' => '遅延 • ${delayedAge[1]} 秒前',
      'zh-CN' => '延迟 • ${delayedAge[1]} 秒前',
      _ => value,
    };
  }
  final oldAge = RegExp(r'^No recent data • (\d+)m ago$').firstMatch(value);
  if (oldAge != null && language != 'en') {
    return switch (language) {
      'de' => 'Keine aktuellen Daten • vor ${oldAge[1]} Min.',
      'ja' => '最近のデータなし • ${oldAge[1]} 分前',
      'zh-CN' => '暂无最新数据 • ${oldAge[1]} 分钟前',
      _ => value,
    };
  }
  final logCount = RegExp(r'^Logs \((\d+)/(\d+)\)$').firstMatch(value);
  if (logCount != null) {
    final count = '${logCount[1]}/${logCount[2]}';
    return switch (language) {
      'de' => 'Protokolle ($count)',
      'ja' => 'ログ ($count)',
      'zh-CN' => '日志 ($count)',
      _ => value,
    };
  }
  final assignedRole = RegExp(r'^Role: (.*)$').firstMatch(value);
  if (assignedRole != null) {
    final role = _translations[language]?[assignedRole[1]!] ?? assignedRole[1]!;
    final label = _translations[language]?['Role'] ?? 'Role';
    return '$label: $role';
  }
  final address = RegExp(r'^MAC: (.*)$').firstMatch(value);
  if (address != null) {
    final label = _translations[language]?['Bluetooth address'] ?? 'MAC';
    return '$label: ${address[1]}';
  }
  final tray = RegExp(
          r'^(\d+)/(\d+) tracker\(s\) • SlimeVR (on|off)(?: • (\d+) error\(s\))?$')
      .firstMatch(value);
  if (tray != null && language != 'en') {
    final connected = tray[1];
    final total = tray[2];
    final state = tray[3] == 'on';
    final errors = tray[4];
    return switch (language) {
      'de' =>
        '$connected/$total Tracker • SlimeVR ${state ? 'aktiv' : 'inaktiv'}${errors == null ? '' : ' • $errors Fehler'}',
      'ja' =>
        '$connected/$total 台 • SlimeVR ${state ? '接続中' : '未接続'}${errors == null ? '' : ' • エラー $errors 件'}',
      'zh-CN' =>
        '追踪器 $connected/$total • SlimeVR ${state ? '已连接' : '未连接'}${errors == null ? '' : ' • 错误 $errors 个'}',
      _ => value,
    };
  }
  return value;
}

class LocalizedTooltip extends StatelessWidget {
  final String message;
  final Widget child;

  const LocalizedTooltip(
      {super.key, required this.message, required this.child});

  @override
  Widget build(BuildContext context) {
    final locale = Localizations.localeOf(context);
    final language = locale.countryCode == 'CN' ? 'zh-CN' : locale.languageCode;
    return Tooltip(message: localizeUiString(language, message), child: child);
  }
}

const Map<String, Map<String, String>> _translations = {
  'de': {
    'Dashboard': 'Übersicht',
    'Trackers': 'Tracker',
    'Calibration': 'Kalibrierung',
    'Settings': 'Einstellungen',
    'Logs': 'Protokolle',
    'Connected Trackers': 'Verbundene Tracker',
    'BLE Status': 'BLE-Status',
    'BLE Adapter!': 'BLE-Adapter!',
    'No adapter': 'Kein Adapter',
    'Scanning...': 'Suche läuft …',
    'Ready': 'Bereit',
    'Connected': 'Verbunden',
    'Disconnected': 'Getrennt',
    'Disabled': 'Deaktiviert',
    'No target': 'Kein Ziel',
    'Data Rate': 'Datenrate',
    'Errors': 'Fehler',
    'Last error': 'Letzter Fehler',
    'Quick Actions': 'Schnellaktionen',
    'Scan Devices': 'Geräte suchen',
    'Start Tracking': 'Tracking starten',
    'Stop Tracking': 'Tracking stoppen',
    'SlimeVR Reconnect': 'SlimeVR erneut verbinden',
    'Send Handshake': 'Handshake senden',
    'Handshake senden': 'Handshake senden',
    'Trackers Overview': 'Tracker-Übersicht',
    'No trackers found': 'Keine Tracker gefunden',
    'Click "Scan Devices" to discover Mocopi trackers':
        'Wähle „Geräte suchen“, um Mocopi-Tracker zu finden',
    'Unassigned': 'Nicht zugewiesen',
    'General': 'Allgemein',
    'Bluetooth': 'Bluetooth',
    'SlimeVR': 'SlimeVR',
    'Tracking': 'Tracking',
    'Language': 'Sprache',
    'English': 'Englisch',
    'Japanese': 'Japanisch',
    'German': 'Deutsch',
    'Chinese (Simplified)': 'Chinesisch (vereinfacht)',
    'Log Level': 'Protokollstufe',
    'Start with Windows': 'Mit Windows starten',
    'Minimize to Tray': 'In Infobereich minimieren',
    'Start Minimized': 'Minimiert starten',
    'Auto Scan': 'Automatisch suchen',
    'Auto Connect': 'Automatisch verbinden',
    'Reconnect Delay (ms)': 'Wartezeit vor Wiederverbindung (ms)',
    'Connection Timeout (ms)': 'Verbindungszeitlimit (ms)',
    'Max Trackers': 'Maximale Trackerzahl',
    'Min. Signal (dBm)': 'Min. Signalstärke (dBm)',
    'Enabled': 'Aktiviert',
    'Server IP': 'Server-IP',
    'Server Port': 'Server-Port',
    'Packet Rate (Hz)': 'Paketrate (Hz)',
    'Coordinate System': 'Koordinatensystem',
    'Enable Filtering': 'Filter aktivieren',
    'Quaternion Slerp Factor': 'Quaternion-Slerp-Faktor',
    'Accel Low-pass Alpha': 'Beschleunigungs-Tiefpass-Alpha',
    'Save Settings': 'Einstellungen speichern',
    'Retry Initialization': 'Initialisierung wiederholen',
    'Native Bridge Unavailable': 'Native Bridge nicht verfügbar',
    'No Trackers Found': 'Keine Tracker gefunden',
    'Schwache ausblenden': 'Schwache ausblenden',
    'Connect': 'Verbinden',
    'Reconnect': 'Erneut verbinden',
    'Ausschalten': 'Trennen',
    'Role Assignment': 'Rollenzuweisung',
    'Role': 'Rolle',
    'Bluetooth address': 'Bluetooth-Adresse',
    'Actions': 'Aktionen',
    'Mount-Flip (180°)': 'Montagekorrektur (180°)',
    'Unassigned role': 'Keine Rolle zugewiesen',
    'Logs copied': 'Protokolle kopiert',
    'Export support bundle': 'Supportpaket exportieren',
    'IP addresses, device identifiers, and local paths are removed by default.':
        'IP-Adressen, Gerätekennungen und lokale Pfade werden standardmäßig entfernt.',
    'Include IP addresses and device identifiers':
        'IP-Adressen und Gerätekennungen einschließen',
    'Enable this only when the support recipient needs them.':
        'Nur aktivieren, wenn der Support diese Angaben benötigt.',
    'Cancel': 'Abbrechen',
    'Export': 'Exportieren',
    'SlimeVR-Kalibrierung': 'SlimeVR-Kalibrierung',
    'Kalibrierung durchführen': 'Kalibrierung durchführen',
    'SlimeVR verbinden': 'SlimeVR verbinden',
    'Tracker erneut anmelden': 'Tracker erneut anmelden',
    'Role:': 'Rolle:',
    'SlimeVR Output Diagnostics': 'SlimeVR-Ausgabediagnose',
    'Waiting for SlimeVR status': 'Warte auf SlimeVR-Status',
    'Output state': 'Ausgabestatus',
    'Status unavailable': 'Status nicht verfügbar',
    'SlimeVR output is enabled.': 'SlimeVR-Ausgabe ist aktiviert.',
    'SlimeVR output is disabled in Settings.':
        'SlimeVR-Ausgabe ist in den Einstellungen deaktiviert.',
    'No packets are being sent; check the server target and tracker roles.':
        'Es werden keine Pakete gesendet. Prüfe Serverziel und Tracker-Rollen.',
    'Streaming trackers': 'Aktive Tracker',
    'No SlimeVR server connected': 'Kein SlimeVR-Server verbunden',
    'SlimeVR Calibration': 'SlimeVR-Kalibrierung',
    'SlimeVR Server manages body and tracker calibration. Mocoslime forwards tracker orientation after hardware mounting correction, coordinate conversion, and sensor filtering. It does not apply additional pose offsets.':
        'Der SlimeVR-Server verwaltet die Körper- und Tracker-Kalibrierung. Mocoslime leitet die Tracker-Ausrichtung nach Montagekorrektur, Koordinatenumrechnung und Filterung weiter und wendet keine zusätzlichen Pose-Offsets an.',
    'Calibration steps': 'Kalibrierungsschritte',
    '1. Connect trackers here and register them with SlimeVR.':
        '1. Verbinde die Tracker hier und melde sie beim SlimeVR-Server an.',
    '2. Assign trackers to body roles in SlimeVR Server.':
        '2. Weise den Trackern im SlimeVR-Server Körperrollen zu.',
    '3. Follow the reference pose shown by SlimeVR and calibrate there.':
        '3. Nimm die Referenzpose von SlimeVR ein und kalibriere dort.',
    '4. Perform drift resets and re-alignment in SlimeVR Server.':
        '4. Setze Drift zurück und richte die Tracker im SlimeVR-Server neu aus.',
    'Connect SlimeVR': 'SlimeVR verbinden',
    'Re-register trackers': 'Tracker erneut anmelden',
    'Hide weak signals': 'Schwache Signale ausblenden',
    'Scan for Devices': 'Geräte suchen',
    'Connect your Mocopi trackers via Bluetooth':
        'Verbinde deine Mocopi-Tracker per Bluetooth',
    'Device ID': 'Geräte-ID',
    'Firmware': 'Firmware',
    'Packets Received': 'Empfangene Pakete',
    'Live Data Rate': 'Live-Datenrate',
    'Movement (accel)': 'Bewegung (Beschleunigung)',
    'Orientation (quat)': 'Ausrichtung (Quaternion)',
    'Estimated Position (m)': 'Geschätzte Position (m)',
    'Tracking Data': 'Trackingdaten',
    'Reconnect Attempts': 'Wiederverbindungsversuche',
    'Error': 'Fehler',
    'Clear': 'Leeren',
    'Poll now': 'Jetzt aktualisieren',
    'Copy visible logs': 'Sichtbare Protokolle kopieren',
    'Export diagnostics file (support bundle)': 'Diagnosepaket exportieren',
    'Search message or target…': 'Nach Meldung oder Quelle suchen …',
    'No log entries yet': 'Noch keine Protokolleinträge',
    'Waiting for first IMU packet': 'Warte auf erstes IMU-Paket',
    'Waiting for tracking data': 'Warte auf Trackingdaten',
    'No entries match filter/search':
        'Keine Einträge entsprechen Filter oder Suche',
    'Starting…': 'Wird gestartet …',
    'Show window': 'Fenster anzeigen',
    'Start tracking': 'Tracking starten',
    'Stop tracking': 'Tracking stoppen',
    'Close application': 'Anwendung schließen',
    'Export diagnostics': 'Diagnose exportieren',
    'Minimum level': 'Mindeststufe',
    'Bluetooth signal strength': 'Bluetooth-Signalstärke',
    'No RSSI sample yet. Scan again while the tracker is powered on.':
        'Noch kein RSSI-Wert. Starte bei eingeschaltetem Tracker erneut eine Suche.',
    'Disconnecting stops auto-reconnect in Mocoslime; the tracker itself stays powered on.':
        'Trennt den Tracker in Mocoslime und stoppt Auto-Reconnect. Das Gerät bleibt eingeschaltet.',
    'Disconnect and reconnect this tracker manually':
        'Diesen Tracker manuell trennen und erneut verbinden',
    'Correct a tracker mounted with a 180-degree rotation.':
        'Korrigiere einen um 180 Grad verdreht montierten Tracker.',
    'Assign a role to estimate': 'Weise zum Schätzen eine Rolle zu',
    'Waiting for position sample': 'Warte auf Positionsdaten',
    'Left Foot': 'Linker Fuß',
    'Right Foot': 'Rechter Fuß',
    'Left Lower Leg': 'Linker Unterschenkel',
    'Right Lower Leg': 'Rechter Unterschenkel',
    'Left Upper Leg': 'Linker Oberschenkel',
    'Right Upper Leg': 'Rechter Oberschenkel',
    'Left Hand': 'Linke Hand',
    'Right Hand': 'Rechte Hand',
    'Left Lower Arm': 'Linker Unterarm',
    'Right Lower Arm': 'Rechter Unterarm',
    'Left Upper Arm': 'Linker Oberarm',
    'Right Upper Arm': 'Rechter Oberarm',
    'Head': 'Kopf',
    'Setup checklist': 'Einrichtungscheckliste',
    'Follow these steps to send tracker data to SlimeVR.':
        'Folge diesen Schritten, um Tracker-Daten an SlimeVR zu senden.',
    'Bluetooth adapter ready': 'Bluetooth-Adapter bereit',
    'Check Bluetooth adapter': 'Bluetooth-Adapter prüfen',
    'Tracker connected': 'Tracker verbunden',
    'Connect at least one tracker': 'Mindestens einen Tracker verbinden',
    'Body role assigned': 'Körperrolle zugewiesen',
    'Resolve duplicate SlimeVR body roles':
        'Doppelte SlimeVR-Körperrollen auflösen',
    'Assign a body role to a tracker':
        'Einem Tracker eine Körperrolle zuweisen',
    'SlimeVR server target configured': 'SlimeVR-Serverziel eingerichtet',
    'Configure the SlimeVR server target': 'SlimeVR-Serverziel einrichten',
    'Streaming is active': 'Datenübertragung aktiv',
    'Start streaming from Quick Actions':
        'Datenübertragung unter Schnellaktionen starten',
    'Chest': 'Brust',
    'Waist': 'Taille',
  },
  'ja': {
    'Dashboard': 'ダッシュボード',
    'Trackers': 'トラッカー',
    'Calibration': 'キャリブレーション',
    'Settings': '設定',
    'Logs': 'ログ',
    'Connected Trackers': '接続済みトラッカー',
    'BLE Status': 'BLE 状態',
    'BLE Adapter!': 'BLE アダプター!',
    'No adapter': 'アダプターなし',
    'Scanning...': 'スキャン中…',
    'Ready': '準備完了',
    'Connected': '接続済み',
    'Disconnected': '切断',
    'Disabled': '無効',
    'No target': '接続先なし',
    'Data Rate': 'データレート',
    'Errors': 'エラー',
    'Last error': '直近のエラー',
    'Quick Actions': 'クイック操作',
    'Scan Devices': 'デバイスを検索',
    'Start Tracking': 'トラッキング開始',
    'Stop Tracking': 'トラッキング停止',
    'SlimeVR Reconnect': 'SlimeVR 再接続',
    'Send Handshake': 'ハンドシェイクを送信',
    'Handshake senden': 'ハンドシェイクを送信',
    'Trackers Overview': 'トラッカー一覧',
    'No trackers found': 'トラッカーが見つかりません',
    'Click "Scan Devices" to discover Mocopi trackers':
        '「デバイスを検索」を選択して Mocopi を検索します',
    'Unassigned': '未割り当て',
    'General': '一般',
    'Bluetooth': 'Bluetooth',
    'SlimeVR': 'SlimeVR',
    'Tracking': 'トラッキング',
    'Language': '言語',
    'English': '英語',
    'Japanese': '日本語',
    'German': 'ドイツ語',
    'Chinese (Simplified)': '中国語（簡体字）',
    'Log Level': 'ログレベル',
    'Start with Windows': 'Windows 起動時に開始',
    'Minimize to Tray': 'トレイに最小化',
    'Start Minimized': '最小化して起動',
    'Auto Scan': '自動スキャン',
    'Auto Connect': '自動接続',
    'Reconnect Delay (ms)': '再接続までの待機時間 (ms)',
    'Connection Timeout (ms)': '接続タイムアウト (ms)',
    'Max Trackers': '最大トラッカー数',
    'Min. Signal (dBm)': '最小信号強度 (dBm)',
    'Enabled': '有効',
    'Server IP': 'サーバー IP',
    'Server Port': 'サーバーポート',
    'Packet Rate (Hz)': 'パケットレート (Hz)',
    'Coordinate System': '座標系',
    'Enable Filtering': 'フィルターを有効化',
    'Quaternion Slerp Factor': 'Quaternion Slerp 係数',
    'Accel Low-pass Alpha': '加速度ローパス係数',
    'Save Settings': '設定を保存',
    'Retry Initialization': '初期化を再試行',
    'Native Bridge Unavailable': 'ネイティブブリッジを利用できません',
    'Connect': '接続',
    'Reconnect': '再接続',
    'Ausschalten': '切断',
    'Role Assignment': 'ロール割り当て',
    'Role': 'ロール',
    'Bluetooth address': 'Bluetooth アドレス',
    'Actions': '操作',
    'Mount-Flip (180°)': '取り付け補正 (180°)',
    'Logs copied': 'ログをコピーしました',
    'Export support bundle': 'サポート情報をエクスポート',
    'IP addresses, device identifiers, and local paths are removed by default.':
        'IP アドレス、デバイス ID、ローカルパスは既定で削除されます。',
    'Include IP addresses and device identifiers': 'IP アドレスとデバイス ID を含める',
    'Enable this only when the support recipient needs them.':
        'サポート担当者が必要とする場合にのみ有効にしてください。',
    'Cancel': 'キャンセル',
    'Export': 'エクスポート',
    'SlimeVR-Kalibrierung': 'SlimeVR キャリブレーション',
    'Kalibrierung durchführen': 'キャリブレーション手順',
    'SlimeVR verbinden': 'SlimeVR に接続',
    'Tracker erneut anmelden': 'トラッカーを再登録',
    'Schwache ausblenden': '信号の弱いデバイスを隠す',
    'SlimeVR Output Diagnostics': 'SlimeVR 出力診断',
    'Waiting for SlimeVR status': 'SlimeVR の状態を待機中',
    'Output state': '出力状態',
    'Status unavailable': '状態を取得できません',
    'SlimeVR output is enabled.': 'SlimeVR 出力は有効です。',
    'SlimeVR output is disabled in Settings.': '設定で SlimeVR 出力が無効になっています。',
    'No packets are being sent; check the server target and tracker roles.':
        'パケットが送信されていません。サーバー接続先とトラッカーのロールを確認してください。',
    'Streaming trackers': '配信中のトラッカー',
    'No SlimeVR server connected': 'SlimeVR サーバー未接続',
    'SlimeVR Calibration': 'SlimeVR キャリブレーション',
    'SlimeVR Server manages body and tracker calibration. Mocoslime forwards tracker orientation after hardware mounting correction, coordinate conversion, and sensor filtering. It does not apply additional pose offsets.':
        '身体とトラッカーのキャリブレーションは SlimeVR サーバーが管理します。Mocoslime は取り付け補正、座標変換、フィルター処理後の向きを転送し、追加のポーズ補正は行いません。',
    'Calibration steps': 'キャリブレーション手順',
    '1. Connect trackers here and register them with SlimeVR.':
        '1. ここでトラッカーを接続し、SlimeVR に登録します。',
    '2. Assign trackers to body roles in SlimeVR Server.':
        '2. SlimeVR サーバーでトラッカーに身体のロールを割り当てます。',
    '3. Follow the reference pose shown by SlimeVR and calibrate there.':
        '3. SlimeVR の基準ポーズを取り、サーバーでキャリブレーションします。',
    '4. Perform drift resets and re-alignment in SlimeVR Server.':
        '4. ドリフトリセットと再調整は SlimeVR サーバーで行います。',
    'Connect SlimeVR': 'SlimeVR に接続',
    'Re-register trackers': 'トラッカーを再登録',
    'Hide weak signals': '信号の弱いデバイスを隠す',
    'Scan for Devices': 'デバイスを検索',
    'Connect your Mocopi trackers via Bluetooth':
        'Bluetooth で Mocopi トラッカーを接続します',
    'Device ID': 'デバイス ID',
    'Firmware': 'ファームウェア',
    'Packets Received': '受信パケット数',
    'Live Data Rate': 'リアルタイムデータレート',
    'Movement (accel)': '動き（加速度）',
    'Orientation (quat)': '向き（Quaternion）',
    'Estimated Position (m)': '推定位置 (m)',
    'Tracking Data': 'トラッキングデータ',
    'Reconnect Attempts': '再接続の試行回数',
    'Error': 'エラー',
    'Clear': '消去',
    'Poll now': '今すぐ更新',
    'Copy visible logs': '表示中のログをコピー',
    'Export diagnostics file (support bundle)': '診断情報をエクスポート',
    'Minimum level': '最小レベル',
    'Search message or target…': 'メッセージまたは対象を検索…',
    'No log entries yet': 'ログはまだありません',
    'Waiting for first IMU packet': '最初の IMU パケットを待機中',
    'Waiting for tracking data': 'トラッキングデータを待機中',
    'No entries match filter/search': '条件に一致する項目はありません',
    'Starting…': '起動中…',
    'Show window': 'ウィンドウを表示',
    'Start tracking': 'トラッキング開始',
    'Stop tracking': 'トラッキング停止',
    'Close application': 'アプリを終了',
    'Bluetooth signal strength': 'Bluetooth 信号強度',
    'No RSSI sample yet. Scan again while the tracker is powered on.':
        'RSSI 値がありません。トラッカーの電源を入れて再スキャンしてください。',
    'Disconnecting stops auto-reconnect in Mocoslime; the tracker itself stays powered on.':
        '切断すると Mocoslime の自動再接続を停止します。トラッカー本体の電源は切れません。',
    'Disconnect and reconnect this tracker manually': 'このトラッカーを手動で切断して再接続',
    'Correct a tracker mounted with a 180-degree rotation.':
        '180 度回転して取り付けたトラッカーを補正します。',
    'Assign a role to estimate': '推定にはロールを割り当ててください',
    'Waiting for position sample': '位置データを待機中',
    'Left Foot': '左足',
    'Right Foot': '右足',
    'Left Lower Leg': '左下腿',
    'Right Lower Leg': '右下腿',
    'Left Upper Leg': '左大腿',
    'Right Upper Leg': '右大腿',
    'Left Hand': '左手',
    'Right Hand': '右手',
    'Left Lower Arm': '左前腕',
    'Right Lower Arm': '右前腕',
    'Left Upper Arm': '左上腕',
    'Right Upper Arm': '右上腕',
    'Head': '頭',
    'Setup checklist': 'セットアップチェックリスト',
    'Follow these steps to send tracker data to SlimeVR.':
        '次の手順でトラッカーデータを SlimeVR に送信します。',
    'Bluetooth adapter ready': 'Bluetooth アダプターの準備完了',
    'Check Bluetooth adapter': 'Bluetooth アダプターを確認',
    'Tracker connected': 'トラッカー接続済み',
    'Connect at least one tracker': 'トラッカーを 1 台以上接続',
    'Body role assigned': '身体ロール割り当て済み',
    'Resolve duplicate SlimeVR body roles': '重複した SlimeVR 身体ロールを解決',
    'Assign a body role to a tracker': 'トラッカーに身体ロールを割り当て',
    'SlimeVR server target configured': 'SlimeVR サーバー接続先を設定済み',
    'Configure the SlimeVR server target': 'SlimeVR サーバー接続先を設定',
    'Streaming is active': 'データ送信中',
    'Start streaming from Quick Actions': 'クイック操作から送信を開始',
    'Chest': '胸',
    'Waist': '腰',
  },
  'zh-CN': {
    'Dashboard': '仪表板',
    'Trackers': '追踪器',
    'Calibration': '校准',
    'Settings': '设置',
    'Logs': '日志',
    'Connected Trackers': '已连接的追踪器',
    'BLE Status': 'BLE 状态',
    'BLE Adapter!': 'BLE 适配器！',
    'No adapter': '未检测到适配器',
    'Scanning...': '正在扫描…',
    'Ready': '就绪',
    'Connected': '已连接',
    'Disconnected': '已断开',
    'Disabled': '已禁用',
    'No target': '没有目标',
    'Data Rate': '数据速率',
    'Errors': '错误',
    'Last error': '最近的错误',
    'Quick Actions': '快捷操作',
    'Scan Devices': '扫描设备',
    'Start Tracking': '开始追踪',
    'Stop Tracking': '停止追踪',
    'SlimeVR Reconnect': '重新连接 SlimeVR',
    'Send Handshake': '发送握手包',
    'Handshake senden': '发送握手包',
    'Trackers Overview': '追踪器概览',
    'No trackers found': '未找到追踪器',
    'Click "Scan Devices" to discover Mocopi trackers':
        '点击“扫描设备”以查找 Mocopi 追踪器',
    'Unassigned': '未分配',
    'General': '常规',
    'Bluetooth': '蓝牙',
    'SlimeVR': 'SlimeVR',
    'Tracking': '追踪',
    'Language': '语言',
    'English': '英语',
    'Japanese': '日语',
    'German': '德语',
    'Chinese (Simplified)': '简体中文',
    'Log Level': '日志级别',
    'Start with Windows': '随 Windows 启动',
    'Minimize to Tray': '最小化到托盘',
    'Start Minimized': '启动时最小化',
    'Auto Scan': '自动扫描',
    'Auto Connect': '自动连接',
    'Reconnect Delay (ms)': '重连延迟 (ms)',
    'Connection Timeout (ms)': '连接超时 (ms)',
    'Max Trackers': '最大追踪器数量',
    'Min. Signal (dBm)': '最低信号强度 (dBm)',
    'Enabled': '已启用',
    'Server IP': '服务器 IP',
    'Server Port': '服务器端口',
    'Packet Rate (Hz)': '数据包速率 (Hz)',
    'Coordinate System': '坐标系',
    'Enable Filtering': '启用滤波',
    'Quaternion Slerp Factor': '四元数 Slerp 系数',
    'Accel Low-pass Alpha': '加速度低通系数',
    'Save Settings': '保存设置',
    'Retry Initialization': '重试初始化',
    'Native Bridge Unavailable': '本机桥接不可用',
    'Connect': '连接',
    'Reconnect': '重新连接',
    'Ausschalten': '断开连接',
    'Role Assignment': '角色分配',
    'Role': '角色',
    'Bluetooth address': '蓝牙地址',
    'Actions': '操作',
    'Mount-Flip (180°)': '安装方向校正 (180°)',
    'Logs copied': '日志已复制',
    'Export support bundle': '导出支持包',
    'IP addresses, device identifiers, and local paths are removed by default.':
        '默认会移除 IP 地址、设备标识符和本地路径。',
    'Include IP addresses and device identifiers': '包含 IP 地址和设备标识符',
    'Enable this only when the support recipient needs them.':
        '仅在支持人员需要这些信息时启用。',
    'Cancel': '取消',
    'Export': '导出',
    'SlimeVR-Kalibrierung': 'SlimeVR 校准',
    'Kalibrierung durchführen': '校准步骤',
    'SlimeVR verbinden': '连接 SlimeVR',
    'Tracker erneut anmelden': '重新注册追踪器',
    'Schwache ausblenden': '隐藏信号较弱的设备',
    'SlimeVR Output Diagnostics': 'SlimeVR 输出诊断',
    'Waiting for SlimeVR status': '正在等待 SlimeVR 状态',
    'Output state': '输出状态',
    'Status unavailable': '状态不可用',
    'SlimeVR output is enabled.': 'SlimeVR 输出已启用。',
    'SlimeVR output is disabled in Settings.': 'SlimeVR 输出已在设置中禁用。',
    'No packets are being sent; check the server target and tracker roles.':
        '没有发送数据包，请检查服务器目标和追踪器角色。',
    'Streaming trackers': '正在传输的追踪器',
    'No SlimeVR server connected': '未连接 SlimeVR 服务器',
    'SlimeVR Calibration': 'SlimeVR 校准',
    'SlimeVR Server manages body and tracker calibration. Mocoslime forwards tracker orientation after hardware mounting correction, coordinate conversion, and sensor filtering. It does not apply additional pose offsets.':
        '身体和追踪器校准由 SlimeVR 服务器管理。Mocoslime 在硬件安装校正、坐标转换和滤波后转发追踪器方向，不会额外应用姿态偏移。',
    'Calibration steps': '校准步骤',
    '1. Connect trackers here and register them with SlimeVR.':
        '1. 在此连接追踪器并向 SlimeVR 注册。',
    '2. Assign trackers to body roles in SlimeVR Server.':
        '2. 在 SlimeVR 服务器中为追踪器分配身体角色。',
    '3. Follow the reference pose shown by SlimeVR and calibrate there.':
        '3. 按照 SlimeVR 显示的参考姿势在服务器中校准。',
    '4. Perform drift resets and re-alignment in SlimeVR Server.':
        '4. 在 SlimeVR 服务器中执行漂移重置和重新对齐。',
    'Connect SlimeVR': '连接 SlimeVR',
    'Re-register trackers': '重新注册追踪器',
    'Hide weak signals': '隐藏信号较弱的设备',
    'Scan for Devices': '扫描设备',
    'Connect your Mocopi trackers via Bluetooth': '通过蓝牙连接 Mocopi 追踪器',
    'Device ID': '设备 ID',
    'Firmware': '固件',
    'Packets Received': '已接收数据包',
    'Live Data Rate': '实时数据速率',
    'Movement (accel)': '运动（加速度）',
    'Orientation (quat)': '方向（四元数）',
    'Estimated Position (m)': '估算位置 (m)',
    'Tracking Data': '追踪数据',
    'Reconnect Attempts': '重连次数',
    'Error': '错误',
    'Clear': '清除',
    'Poll now': '立即刷新',
    'Copy visible logs': '复制当前日志',
    'Export diagnostics file (support bundle)': '导出诊断信息',
    'Minimum level': '最低级别',
    'Search message or target…': '搜索消息或目标…',
    'No log entries yet': '暂无日志',
    'Waiting for first IMU packet': '正在等待第一个 IMU 数据包',
    'Waiting for tracking data': '正在等待追踪数据',
    'No entries match filter/search': '没有符合筛选或搜索条件的日志',
    'Starting…': '正在启动…',
    'Show window': '显示窗口',
    'Start tracking': '开始追踪',
    'Stop tracking': '停止追踪',
    'Close application': '关闭应用',
    'Bluetooth signal strength': '蓝牙信号强度',
    'No RSSI sample yet. Scan again while the tracker is powered on.':
        '暂无 RSSI 信号值。请保持追踪器开机并重新扫描。',
    'Disconnecting stops auto-reconnect in Mocoslime; the tracker itself stays powered on.':
        '断开后 Mocoslime 将停止自动重连，追踪器本身仍保持开机。',
    'Disconnect and reconnect this tracker manually': '手动断开并重新连接此追踪器',
    'Correct a tracker mounted with a 180-degree rotation.':
        '校正旋转 180 度安装的追踪器。',
    'Assign a role to estimate': '请分配角色以进行估算',
    'Waiting for position sample': '正在等待位置数据',
    'Left Foot': '左脚',
    'Right Foot': '右脚',
    'Left Lower Leg': '左小腿',
    'Right Lower Leg': '右小腿',
    'Left Upper Leg': '左大腿',
    'Right Upper Leg': '右大腿',
    'Left Hand': '左手',
    'Right Hand': '右手',
    'Left Lower Arm': '左前臂',
    'Right Lower Arm': '右前臂',
    'Left Upper Arm': '左上臂',
    'Right Upper Arm': '右上臂',
    'Head': '头部',
    'Setup checklist': '设置清单',
    'Follow these steps to send tracker data to SlimeVR.':
        '按照以下步骤将追踪器数据发送到 SlimeVR。',
    'Bluetooth adapter ready': '蓝牙适配器已就绪',
    'Check Bluetooth adapter': '检查蓝牙适配器',
    'Tracker connected': '追踪器已连接',
    'Connect at least one tracker': '至少连接一个追踪器',
    'Body role assigned': '身体角色已分配',
    'Resolve duplicate SlimeVR body roles': '解决重复的 SlimeVR 身体角色',
    'Assign a body role to a tracker': '为追踪器分配身体角色',
    'SlimeVR server target configured': 'SlimeVR 服务器目标已设置',
    'Configure the SlimeVR server target': '设置 SlimeVR 服务器目标',
    'Streaming is active': '数据传输已启动',
    'Start streaming from Quick Actions': '从快捷操作启动数据传输',
    'Chest': '胸部',
    'Waist': '腰部',
  },
};
