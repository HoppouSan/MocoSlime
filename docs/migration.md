# Migration Map: MoSlime (Python) → MoSlime-RS (Rust + Flutter)

This document compares the existing project in `moslime-main/` (two Python modules plus helper scripts) with the Rust and Flutter implementation. Protocol knowledge such as byte layouts, UUIDs, commands, and conversion factors was retained. The architecture, concurrency, error handling, and UI were redesigned.

## Module mapping

| Legacy Python module | Previous responsibility | New Rust module | Notes |
|---|---|---|---|
| `moslime.py :: trackerHandler` | One thread per tracker, `bluepy.Peripheral`, MTU, command characteristic, handshake, notification loop, exception-based reconnect | `mocopi-ble-windows` (`BleManager`, `WinRtConnection` in `winrt.rs` and `backend.rs`) plus `moslime-core::TrackerManager` | Per-tracker threads became Tokio tasks and broadcast channels; reconnect uses exponential backoff instead of `sleep(3)` and retry; implicit loops became a `Disconnected…Streaming` state machine. |
| `moslime.py :: NotificationHandler` | `handleNotification(svc, data)` for service 73 (IMU) / 34 (command), `ignorePackets=-40`, offset `(0.707,0.707,0,0)`, packet-loss detection using counter delta 78125 | `mocopi-protocol::decode_packet`, `tracking-core::CalibrationManager`, and `moslime-core::process_imu_packet` | The startup `ignorePackets` quirk was replaced with averaging N calibration samples. Counter-delta checks were replaced by monotonic timestamps and filter drop counters. |
| `moslime_common.MocopiPacket` | Named tuple `(qw,qx,qy,qz,ax,ay,az,counter)` | `mocopi-protocol::MocopiPacket` (`UnitQuaternion<f32>`, `Vector3<f32>`, `counter`, `timestamp_ns`) | Strongly typed, normalized quaternions, and Serde support for FFI. |
| `hexToQuat` (`/8192`, little-endian i16) | Quaternion scaling | `mocopi-protocol::QUATERNION_SCALE`, `parse_imu_packet` | Preserved with length/type validation and tests. |
| `hexToFloat` (`<e`) and `correctAccel` (`*0.12`) | Acceleration decoding and correction | `mocopi-protocol::ACCEL_CORRECTION_FACTOR`, axis remap `(x,z,y)` | Preserved; the axis swap is documented. |
| `multiply` (hand-written quaternion multiplication) | Offset correction | `nalgebra::UnitQuaternion` (`offset * quat`) | Replaced hand-written math with a maintained library. |
| `process_packet(data, offset)` | Quaternion at bytes 8–16, acceleration at 24–30, counter `data[1:8]` | `parse_imu_packet`, `decode_packet`, `calculate_calibration_offset` | Layout preserved, with validation, framing (`7e 07 09` for battery), and an `Unknown` variant instead of exceptions. |
| `calc_batt` (`(raw-67410)/6501390`, `+3.2`, clamp 3.0–4.2) | Battery conversion | `parse_battery_packet` | Formula preserved, with clamping and framed/raw packet support. |
| `build_handshake`, `build_sensor_info`, `build_rotation_packet`, `build_accel_packet`, `build_battery_packet`, `build_error_packet` | SlimeVR UDP (`>Q`, `>I`, `>ffff` with `(-qx,qz,qy,qw)`) | `slimevr-protocol` (`build_*`, `SlimeVRPacket` without a length prefix) | Byte-compatible with the Python reference layout (tested with vectors); no separate length field (a previous bug was fixed). |
| `find_slime` (bind UDP 6969, `Hey OVR`, five attempts, localhost fallback) | SlimeVR autodiscovery | `moslime-core::discover_slimevr` | Same semantics (5 × 1s, fallback to 127.0.0.1), without calling `quit()` on failure. |
| `create_sock` (one socket per tracker) | UDP transport | `moslime-core` (one socket, `sensor_id` = body role) | Intentional difference: Python distinguishes trackers by source port; Rust uses sensor ID. This is documented and protocol-compatible. |
| `cfg_from_json` (`moslime.json`, `tps` migration, autostart) | Configuration | `configuration::AppConfig` (versioned, nested, `load/save/migrate`) | Flat keys became sections (`general/bluetooth/slimevr/tracking/trackers`); migration replaces `quit()` on errors. |
| `scripts/autopair.py`, `scripts/raspi/pair-trackers.py` (`bluetoothctl pair/disconnect`, `main.conf` hack) | Pairing | Removed (pair through Windows Settings; discover through WinRT enumeration) | The Linux `bluetoothctl` dependency was removed (Spec §1). |
| `raspi/*`, `scripts/steamdeck_*`, `requirements.txt` (`bluepy==1.3.0`) | Platform support | Removed | The target is Windows 10/11 x64; no Python runtime is required. |
| CLI (`input()`, `print`, `os.system("clear")`, Ctrl+C loop) | UI | Flutter (`flutter/lib/...`: Dashboard, Trackers, Calibration, Settings, Logs) plus `ffi` bridge | Business logic stays out of widgets; the Rust core runs in-process with the app. |

## Configuration file (robustness)

`%APPDATA%\MoSlime-RS\config.json` is written atomically (temporary file plus rename, never a partial file), with `config.bak` preserving the last valid version. Load order is `config.json` → `config.bak` → defaults, so startup does not fail on a damaged file. Versioned migration (`version`, currently 2) covers pose offsets, skeleton, auto-pose, mount flip, and minimum RSSI.

## Protocol knowledge retained (not copied)

- BLE command service `0000ff00-…` (characteristics `FF01` write, `FF03` notify/replies); start streaming `7e 03 18 d6 01 00 00`, status `7e 02 09 02 9a da`, battery request similar. The `0000fff0-…` service is absent on the verified current firmware; IMU data uses notification characteristics in vendor services (for example, `25047E64-…`, handle `0x49`, as in “BLE handles.txt”). Connection setup performs uncached discovery with retry and cached merge, and also subscribes to notification pipes in non-standard services.
- IMU frame: 36 bytes, type `0x49` or `0x30` (same layout), counter bytes 1–8 (LE), quaternion i16-LE `/8192` starting at byte 8, previous-packet quaternion at bytes 16–24 (fallback for a corrupt frame; the protocol has no CRC), acceleration half-LE starting at byte 24 with axes `(x,z,y)` and factor `0.12`.
- Battery frame: `7e 07 09 …`, raw u32-BE at bytes 7–11, percentage `(raw−67410)/6501390`, voltage `+3.2`.
- SlimeVR: u32-BE type + u64-BE counter + payload (no length field); rotation `(-qx,qz,qy,qw)` as f32-BE; handshake board/IMU/MCU types 15/8/7, firmware string, MAC, `0xFF`.

## Intentional improvements over Python

1. **Isolation:** BLE transport, protocol decoder, tracking core, and SlimeVR output are separate crates with clear boundaries (Spec §25).
2. **Error model:** `Result<T, E>` with `BleError/ProtocolError/TrackerError/SlimeVRError/ConfigurationError` replaces exceptions and `quit()`; a faulty tracker does not affect the others.
3. **Real-time path:** no allocation loops in the hot path and no JSON in the tracking loop (GUI events are throttled to about 4 Hz per tracker); the GUI is not blocked.
4. **Testability:** the `BleBackend` trait and `MemoryBackend` allow BLE logic to be tested without hardware; protocol vectors have unit and pipeline integration tests.
