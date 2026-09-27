# BLE Reliability (P2)

## Scanning

- `BleManager::start_scan` uses `scan_duration_ms` (10 seconds by default) plus a 5-second outer budget.
- The RSSI filter uses `rssi_min_dbm` (default: -85). A value of `0` means unknown and always passes; otherwise, the signal must be at least the configured minimum. Results are sorted by signal strength and limited to `max_trackers`.
- In the GUI, `AppState.scanDevices` checks the adapter first with `moslime_get_adapter_status`. A hard 20-second timeout is used as a fallback; `ScanCompleted` and `Error` cancel the timer.

## Adapter status

- Rust calls `mocopi_ble_windows::adapter_status()` (WinRT `GetDefaultAsync`).
- FFI `moslime_get_adapter_status()` returns `{ok, error}` for the Dashboard card without creating a full diagnostics bundle. The Dashboard shows `No adapter` in red, `Reconnecting (n)` in blue, and `Ready` in green.

## Reconnection

- The scheduler starts automatically from `run()` (it previously was never started). A 1-second ticker schedules each device's `next_retry`; due attempts run as separate tasks. A `connecting` guard prevents races between auto-connect, the ticker, and manual actions.
- Only one GATT connection setup runs at a time (`connect_lock` is shared by manual connect, auto-connect, and the scheduler). Initial connections are staggered by 2 seconds.
- Retries continue without a fixed attempt limit, using exponential backoff and jitter: 5s → 10s → 20s → 40s → 60s maximum (up to 62s with jitter). Deterministic jitter of ±20% and an additional 0–2s per-device offset (MAC hash) spread attempts. `reconnect_attempts` remains available for display.
- Live configuration is read on every tick, so delay, timeout, and auto-connect changes apply without restarting.
- Retries stop after a manual disconnect, when `auto_connect=false`, or when scanning/stopping requires it. Successful connections, scans, and manual connects reset the retry counter.
- Startup auto-connect is staggered by 500ms and logs successful connections.
- The GUI shows `Reconnecting… attempt n (… backoff) • next in Xs`, an error card with attempt count, and a manual reconnect button that resets backoff. RSSI `0` is displayed as `–`.

## RSSI controls

- The Trackers screen has a minimum signal slider (-100 to -40 dBm) and a `Hide weak devices` toggle (`visibleTrackers`). The Settings screen provides the same persistent slider (`bluetooth.rssi_min_dbm` in `AppConfig`).
