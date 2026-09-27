# Third-Party Licenses

All dependencies are open source. Windows system APIs (the `windows` crate,
which projects the OS APIs) are excluded and may be used. No proprietary SDKs
are included.

For the complete transitive dependency list, run `cargo tree --workspace -e
normal` (Rust) or `flutter pub deps` (Dart). The direct dependencies are listed
below.

## Rust (Cargo)

| Package | Version (lockfile) | License | Purpose |
|---|---|---|---|
| tokio (full) | 1.53 | MIT | Async runtime |
| serde / serde_json | 1.0 / 1.0 | MIT **or** Apache-2.0 | Serialization (config, FFI JSON) |
| thiserror | 1.0/2.0 | MIT **or** Apache-2.0 | Structured errors |
| anyhow | 1.0 | MIT **or** Apache-2.0 | Error handling where appropriate |
| tracing | 0.1 | MIT | Instrumentation |
| tracing-subscriber (fmt, json, env-filter) | 0.3 | MIT | Console/file layers and level filter |
| tracing-appender | 0.2 | MIT | Daily log rotation |
| bytes | 1.12 | MIT | SlimeVR buffer; currently rarely used and may be removed |
| uuid (v4, serde) | 1.26 | MIT **or** Apache-2.0 | Device IDs |
| nalgebra | 0.32 | Apache-2.0 | Quaternions and vectors |
| bytemuck / zerocopy | 1.25 / 0.8 | MIT/Apache/Zlib and BSD-2/MIT/Apache | Currently unused; candidates for removal (no hidden costs) |
| parking_lot | 0.12 | MIT **or** Apache-2.0 | Locks (filter/calibration) |
| async-trait | 0.1 | MIT **or** Apache-2.0 | `BleBackend` trait |
| futures | 0.3 | MIT **or** Apache-2.0 | Test and combinator support |
| bitflags | 2.13 | MIT **or** Apache-2.0 | Reserved; unused and may be removed |
| once_cell | 1.21 | MIT **or** Apache-2.0 | FFI statics (migration to `std::sync::OnceLock` in progress; only remaining uses are here) |
| chrono | 0.4 | MIT **or** Apache-2.0 | RFC-3339 log timestamps |
| dirs | 5.0 | MIT **or** Apache-2.0 | `%APPDATA%` paths |
| half | 2.7 | MIT **or** Apache-2.0 | f16 acceleration values (Mocopi frames) |
| hex | 0.4 | MIT **or** Apache-2.0 | MAC parsing (SlimeVR handshake) |
| rosc | 0.11 | MIT **or** Apache-2.0 | OSC 1.0 encoding (UDP, actively maintained, no runtime dependencies) |
| windows | 0.58 | MIT **or** Apache-2.0 | Native WinRT BLE projection |
| winreg | 0.52 | MIT **or** Apache-2.0 | Autostart (`HKCU\...\Run`), Windows only |

Code quality note (§19, “no unnecessary dependencies”): `bytes`, `bytemuck`,
`zerocopy`, `bitflags`, and `once_cell` (FFI) are marked as removal candidates.
They will either be used or removed from the manifests in the next cleanup.

## Dart/Flutter (pubspec)

| Package | License | Purpose |
|---|---|---|
| ffi / provider / json_annotation | BSD-3 / MIT / BSD-3 | FFI loader, state, models |
| path_provider / shared_preferences | BSD-3 | Reserved for native paths/preferences |
| window_manager | MIT | Window control |
| system_tray | MIT | Tray icon, menu, tooltip |
| permission_handler | MIT | Reserved; BLE permissions are not required on Windows desktop |
| logging | BSD-3 | Dart-side logging; Rust logs are exposed through FFI |
| flutter_lints / build_runner / json_serializable | BSD-3 | Development only |

## Project code

`mocoslime-rs` (workspace root + all `crates/*`): `MIT` (see `LICENSE`). The
The applicable upstream MIT notice is retained in `NOTICE`. Mocoslime is an
independent project and is not affiliated with the separate MoSlime project.

This inventory is a starting point, not a generated release bill of materials.
Regenerate it from the locked Rust and Flutter dependency trees before each
binary release, and include the applicable notices with distributed binaries.
