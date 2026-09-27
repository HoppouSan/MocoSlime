# Pose Profiles: Standing, Sitting, and Lying

Each role has a separate offset profile for each pose (`standing`, `sitting`, `lying`). The core always applies the profile for the active pose.

## Calibration

1. Calibrate **standing** first (I-pose, 60 samples): establishes the baseline and estimates the skeleton.
2. Then calibrate **sitting** (hips/knees at 90°, feet flat, 60 samples).
3. **Lying** is optional (on the back, 90 samples to account for additional drift).

Full-body calibration calibrates all connected trackers for the selected pose. Offsets are stored per pose in `%APPDATA%\Mocoslime\config.json` (the older `%APPDATA%\MoSlime-RS` configuration is migrated on first launch). They are stored in `trackers[].calibration_offsets`. Config v2 migration copies the former single offset to `standing`.

## Skeleton

`tracking.skeleton` uses centimeters. `height_cm` can be edited; when `auto_estimate=true`, the remaining measurements are estimated from height (leg length ≈ 48%) and refreshed after each standing calibration. Validation limits height to 120–230 cm and the leg-to-height ratio to 35–60%.

## Automatic pose switching

`tracking.enable_auto_pose=true` by default. The detector uses calibrated rotations, **not absolute position** (Mocopi provides rotation and acceleration only):

- Standing: waist-to-thigh angle < 30°
- Sitting: waist-to-thigh angle 60–120°
- Lying: torso tilt ≥ 55° (requires more frames: 30 instead of 15)

Hysteresis requires N stable frames before switching. A manual pose change pauses automatic switching for about 30 seconds. Events: `AutoPoseChanged(pose, confidence)` and `ActivePoseChanged(pose)`.

## FFI (API v2)

- `moslime_calibrate_tracker_with_pose(id, "sitting")`
- `moslime_reset_tracker_with_pose(id, "sitting")` (`null`/empty means the active pose)
- `moslime_set_active_pose("lying")`, `moslime_set_auto_pose(1/0)`
- `moslime_set_skeleton(json)`, `moslime_get_pose_status()` → `{active_pose, auto_enabled, confidence, roles:{Role:{pose:{calibrated,at_ns}}}}`
- Legacy `moslime_calibrate_tracker` uses the active pose. Older DLLs (v1) remain supported; new symbols are optional in Dart.
