# SlimeVR Focus: Next Steps

The goal is a reliable Mocopi-to-SlimeVR data path. SlimeVR remains responsible for body calibration and tracker alignment; this app supplies BLE tracking data, roles, mount flip, and diagnostics.

## Priorities

1. **Per-tracker data quality** — packet age and packets per second are visible in the UI. Next, show the last valid IMU sample and more specific error reasons.
2. **Reconnect behavior** — a delayed data stream does not trigger a reconnect; the UI reports the delay. Reconnect only when the BLE link is missing or disconnected. Automatic reconnection with backoff continues indefinitely unless the user disconnects manually. Per-tracker manual reconnect remains available.
3. **Deferred: adaptive smoothing** — do not add another filter layer yet. Extra smoothing can add latency and motion lag; reassess only after stable measurements with real trackers.
4. **SlimeVR output** — make handshakes, roles, coordinate system, rotation, acceleration, and packet loss understandable from SlimeVR status. Show a specific reason in the UI for UDP destination errors.
5. **Setup and runtime quality** — bring SlimeVR connection, tracker assignment, and startup state together in a guided status area. Migrate configuration without continuing to apply obsolete local calibration values.
6. **Hardware validation** — test with multiple Mocopi trackers and a running SlimeVR server: motion start, extended operation, BLE loss, server restart, and sitting/lying/standing. Reassess filters or drift correction only afterward.

## Technical limits

Gyroscope data alone cannot reliably remove yaw drift. An absolute heading reference or correction from SlimeVR is required. Local body calibration would duplicate SlimeVR's own calibration, so it remains removed.
