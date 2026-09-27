# Logging and Diagnostics (Spec §17)

## Levels

`tracing` levels are `ERROR < WARN < INFO < DEBUG < TRACE`. They can be changed under Settings → General → Log Level. Changes take effect immediately across the console, log file, and GUI ring buffer through the reloadable filter (`moslime-core::logging::set_log_level`) and are saved in the configuration.

## Destinations

| Destination | Format | Location |
|---|---|---|
| Console (stdout) | Human-readable `fmt` | Process standard output |
| File | One JSON object per line, daily rotation | `%APPDATA%\Mocoslime\logs\moslime.log` (+ `moslime.log.YYYY-MM-DD`) |
| GUI | Ring buffer with the latest 500 entries, polled once per second | Screens → Logs (level filter and search) |
| Panics | `tracing::error` plus synchronous file append | File and GUI (`panic` target) |

Logs rotate daily with `tracing-appender::rolling::daily`. At startup, `prune_old_logs` removes all but the latest seven rotated files. Rust panics are captured by `init_panic_hook` (installed by `moslime_init`), logged, and synchronously appended to the current log file.

## Diagnostics bundle for bug reports

Screens → Logs → the bug icon, **Export diagnostics file**, writes `%APPDATA%\Mocoslime\diagnostics-<timestamp>.json`. The single-file bundle contains:

- `version`, `api_version`, `os`, and `arch`
- `bluetooth_adapter_ok` and `bluetooth_adapter_error` (for example, `AdapterNotFound`)
- Full configuration and known trackers (including state, errors, and RSSI)
- `recent_logs` (the latest 200 Rust entries, without draining the log buffer) and `flutter_logs`
- `log_dir`, the path to rotated log files

Attach this file to a bug report to provide the Bluetooth diagnostics without follow-up questions.

## Privacy

Regular logs store only level, target, and message. They do not store Bluetooth addresses, quaternions, or raw packets. The GUI also hides full paths. A diagnostics bundle deliberately includes configuration and tracker status (including MAC addresses), and is created only after the user explicitly exports it.

## FFI

- `moslime_set_log_level(level)` applies immediately to the console and saves the setting in the configuration.
- `moslime_poll_logs()` returns a draining JSON array of `{timestamp, level, target, message}`. It never blocks and returns an empty array when the buffer is empty.
- `moslime_get_device(uuid)` returns one tracker's status as JSON (`null` if unknown); release the string with `moslime_free_string`.
- `moslime_get_diagnostics()` returns a non-draining JSON support bundle; release the string with `moslime_free_string`. This symbol is optional, so older DLLs without it remain supported.
