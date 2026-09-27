# Tracking Improvements (T1/T2/T5)

## T1: Effective and adaptive filtering

- `TrackingConfig.filter_slerp_factor/filter_accel_alpha/enable_filtering` are read for every packet (previously defaults were always used, making the sliders ineffective).
- Scale by role and pose: steady torso/legs (1.3–1.4×), responsive hands/head (0.6–0.7×), sitting/lying (0.8–0.85×). Clamp Slerp to 0.02–0.6 and Alpha to 0.05–0.9.
- Adaptive filtering: `effective_slerp(angle)` = base × angle/5° (0.5–3×), smooth at rest and low-lag during fast movement. Hold spikes above 35° within ≤20ms and increment the drop counter.
- Gravity compensation in the filter (opt-in while streaming): linear acceleration = raw − `g_sensor`, then low-pass filtering. Static acceleration should be approximately zero.

## T2: Calibration quality

- `CalibrationData.variance_deg` stores the maximum sample spread. `quality()` reports good below 5°, acceptable below 8°, and unstable above that. Older JSON defaults to 0.
- `CalibrationFinished(..., variance)` adds a fourth, backward-compatible field. Log and GUI warnings appear above 8° (“hold still”).
- FFI `moslime_get_pose_status` returns `variance_deg` and `quality` for each pose.
- The Calibration card shows a green/orange/red indicator and the measured angle.

## T5: Feet and floor (VRC)

- `smooth_positions` applies a per-role low-pass filter with Alpha 0.3 and clamps floor height to Y ≥ 0 (VRC position output only; SlimeVR remains rotation-only).

## Second dead-code replacement round

- `CalibrateAll` uses a real queue (`Queue` and `CalibrationAllFinished(n)`); Cancel clears the queue.
- `import_calibrations` uses FFI `moslime_import_calibrations` to load offsets into the manager and persistent tracker records with matching roles.
- `ResetAllActivePose` resets the active pose; `ResetAllPoses` resets all poses (including GUI button and FFI `moslime_reset_all_calibrations`).
- SlimeVR health monitor emits `SlimeVRDisconnected` after two polling windows without packets.
- `FilterStats.over_rate_packets` counts packets arriving too quickly.
- Scanning restores persisted roles; `moslime_is_scanning` guard, `ble_config`, and `calibrations` are included in the diagnostics bundle (API v4).
- Flutter uses compatibility flags: a pose banner for older DLLs, event lag in the Dashboard error card, and opt-in OSC input UI (port 9001).

## Role persistence and calibration guards

- Dart previously wrote settings with a stale tracker map, deleting roles, offsets, and mount flip. `saveSettings()` now reads tracker data and version from the core first; `assignTracker` and `resetTracker` refresh the cache afterward.
- Calibration requires streaming. Rust rejects requests without data rather than hanging; buttons are disabled without a connection; a watchdog cancels sessions without samples after 30 seconds with `CalibrationCancelled` and an error.
- Only the tracker being calibrated feeds the sampler (`calibrating_device`), preventing other streaming trackers from skewing the mean (the 60° spread symptom).
- Lying detection also requires an extended hip (<45°) to avoid mistaking sitting for lying. Without leg trackers, torso tilt must reach 70°.

## Mounting zero point (FBT style)

- The offset is the **inverse** of the calibration mean, so the calibration pose produces identity (`offset * raw ≈ 1`). Config v3 migrates older offsets by inverting them to avoid a double rotation.
- Quick calibration on a worn tracker (half the samples) and a guided FBT assistant (countdown, progress, quality indicator) are available in the GUI.

## T3: Coordinates and mount flip

- `conversion_to` avoids recursion: A→B = (Mocopi→B)·(Mocopi→A)⁻¹. Round-trip tests cover all pairs (Mocopi/SlimeVR/OpenVR/Unity).
- `mount_flip_correction()` applies 180° yaw per tracker through `TrackerConfig.mount_flip` (default false; older config remains supported). The manager applies the flip before calibration/filtering. FFI `moslime_set_mount_flip` and a switch on the tracker card control it.
- `TrackingConfig.coordinate_system` is applied in this order: flip → calibration (sensor frame) → `TrackerPose` conversion → filter.
- `BleManager::get_config` is included in the diagnostics bundle as `ble_config`.

## T4: Prediction (~8ms, max 15ms)

- `predict_orientation(prev, last, dt, horizon)` extrapolates a capped delta. It is used only for streaming output (SlimeVR/VRC); the GUI and calibration use the filtered signal. Motion state is maintained per device in the manager.
