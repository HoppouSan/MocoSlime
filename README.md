# Mocoslime — Native Windows Mocopi Bridge for SlimeVR

Mocoslime connects Sony Mocopi trackers to SlimeVR Server over Bluetooth LE on Windows. It is an independent community project and is not affiliated with Sony or SlimeVR.

## Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   Flutter/Dart  │────▶│    Rust Core     │────▶│  Windows BLE    │
│      GUI        │ FFI │  (moslime-core)  │     │  (WinRT APIs)   │
└─────────────────┘     └──────────────────┘     └─────────────────┘
                              │
                              ▼
                        ┌──────────────────┐
                        │  Tracking Core   │
                        │  (coordinate     │
                        │   systems,       │
                        │   calibration,   │
                        │   filtering)     │
                        └──────────────────┘
                              │
                              ▼
                        ┌──────────────────┐
                        │  SlimeVR Output  │
                        │  (UDP protocol)  │
                        └──────────────────┘
```

## Project Structure

```
mocoslime/
├── Cargo.toml                 # Cargo workspace
├── crates/
│   ├── mocopi-protocol/       # Mocopi BLE packet decoding
│   ├── tracking-core/         # Platform-independent tracking logic
│   ├── slimevr-protocol/      # SlimeVR UDP packet serialization
│   ├── configuration/         # Configuration management
│   ├── mocopi-ble-windows/    # Native Windows BLE (WinRT)
│   ├── moslime-core/          # Core application logic (legacy internal crate name)
│   └── ffi/                   # FFI bridge for Flutter
├── flutter/
│   ├── lib/
│   │   ├── models/            # Data models
│   │   ├── services/          # Native bridge, business logic
│   │   ├── state/             # State management
│   │   ├── screens/           # UI screens
│   │   ├── widgets/           # Reusable widgets
│   │   └── main.dart          # App entry point
│   └── pubspec.yaml
├── tests/                     # Integration tests
├── docs/                      # Documentation
└── installer/                 # Windows installer scripts
```

## Features

- **Native Windows BLE**: Direct WinRT API usage via `windows-rs`
- **Multi-tracker Support**: Up to 12 simultaneous Mocopi trackers
- **Robust State Machine**: Disconnected → Discovering → Connecting → Initializing → Streaming
- **Auto-reconnect**: Exponential backoff reconnection handling
- **Tracking Core**: Quaternion math, coordinate system conversion, filtering
- **Calibration**: Per-tracker and full-body calibration
- **SlimeVR Output**: UDP protocol with handshake, role mapping, rotation, acceleration, and battery
- **OSC Output**: independent UDP layer (`/moslime/...` + bundles + VRChat preset), parallel to SlimeVR
- **Modern Flutter UI**: Material 3, system tray, responsive design
- **Persistent Configuration**: JSON-based config with migration

## Building

### Prerequisites

- Rust stable (1.75+)
- Flutter SDK (3.16+)
- Windows 10/11 SDK

### Rust Backend

```powershell
cargo build --release
```

### Flutter Frontend

```powershell
cd flutter
flutter pub get
flutter build windows --release
```

### Zusammenführen (wichtig für FFI)

`moslime_ffi.dll` muss neben `Mocoslime.exe` liegen:

```powershell
powershell -File installer/stage.ps1
```

Danach liegt alles Startfähige in `installer/stage/`. Details + Setup-Erstellung:
`installer/README.md`. Logs: `%APPDATA%\Mocoslime\logs\moslime.log`
(täglich rotiert, 7 Dateien), GUI-Ansicht unter Screens → Logs.

## Development Phases

1. ✅ Repository analysis & protocol isolation
2. ✅ Rust Protocol Decoder (mocopi-protocol)
3. ✅ Native Windows BLE (mocopi-ble-windows)
4. ✅ Single tracker connection
5. ✅ Multi-tracker support
6. ✅ Reconnect system
7. ✅ Tracking Core (coordinate systems, filtering)
8. ✅ SlimeVR UDP Output
9. ✅ Rust API for Flutter (FFI)
10. ✅ Flutter Dashboard
11. ✅ Tracker Management
12. ✅ Calibration UI
13. ✅ Settings
14. ✅ System Tray (Hide-to-Tray, Menü, Status-Tooltip)
15. ✅ Logging & Diagnostics (Datei + Rotation + GUI-Logs)
16. ✅ Tests (Unit + Pipeline-Integration)
17. ✅ Windows Installer (Inno Setup + Stage-Skript)

## Roadmap

Priorisierte Produktverbesserungen, SlimeVR-Diagnose und die geplante UI-Lokalisierung
auf Deutsch, Englisch, Japanisch und vereinfachtes Chinesisch stehen in
[`docs/product-roadmap.md`](docs/product-roadmap.md).

## Configuration

Configuration is stored in `%APPDATA%\Mocoslime\config.json`. Existing
`%APPDATA%\MoSlime-RS` settings and tracker roles are migrated on first launch.

```json
{
  "general": {
    "startWithWindows": false,
    "minimizeToTray": true,
    "startMinimized": false,
    "language": "en",
    "logLevel": "INFO"
  },
  "bluetooth": {
    "autoScan": true,
    "autoConnect": true,
    "reconnectDelayMs": 5000,
    "connectionTimeoutMs": 10000,
    "maxTrackers": 12
  },
  "slimevr": {
    "serverIp": "127.0.0.1",
    "serverPort": 6969,
    "enabled": true,
    "packetRate": 128
  },
  "tracking": {
    "coordinateSystem": "SlimeVR",
    "enableFiltering": true,
    "filterSlerpFactor": 0.1,
    "filterAccelAlpha": 0.2,
    "enableCalibration": true
  }
}
```

## Tracker Roles

Supported body roles (mapped to SlimeVR sensor IDs 0-14):

| Role | Sensor ID |
|------|-----------|
| Head | 0 |
| Chest | 1 |
| Waist | 2 |
| Left Upper Arm | 3 |
| Right Upper Arm | 4 |
| Left Lower Arm | 5 |
| Right Lower Arm | 6 |
| Left Hand | 7 |
| Right Hand | 8 |
| Left Upper Leg | 9 |
| Right Upper Leg | 10 |
| Left Lower Leg | 11 |
| Right Lower Leg | 12 |
| Left Foot | 13 |
| Right Foot | 14 |

## License

MIT (see [LICENSE](LICENSE) and upstream attribution in [NOTICE](NOTICE)).

## Acknowledgments

- Original MoSlime Python project
- SlimeVR protocol documentation
- Sony Mocopi tracker specifications

## Open-source release status

See [Release readiness](docs/release-readiness.md) for the checklist, known
limitations, and tracking/SlimeVR feature roadmap. Contributions are welcome
under the MIT license; see [CONTRIBUTING.md](CONTRIBUTING.md) and
[SECURITY.md](SECURITY.md).
[CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md).
