# Generic OSC output

OSC runs independently from the SlimeVR UDP output. It is intended for
TouchOSC, Stream Deck, and other applications that accept generic Mocoslime
messages. VRChat-specific tracker paths and OSCQuery discovery have been
removed; old saved VRChat presets are migrated to Generic OSC.

## Defaults

- Destination: 127.0.0.1:9000
- Rate: 60 Hz
- Bundles enabled
- Naming: numeric tracker IDs
- Output disabled until enabled by the user

## Messages

- Rotation: quaternion floats x y z w
- Euler compatibility message: pitch yaw roll in radians
- Position is emitted only when position output is enabled and the tracking
  pipeline provides an estimate
- Acceleration, battery, RSSI, status, and /moslime/test are optional

The OSC sender uses a bounded queue. When it fills, it drops the oldest frame
so recent tracking data takes priority. Network I/O does not block the tracking
core.

## OSC input

The opt-in listener accepts /moslime/start, /moslime/stop,
/moslime/calibrate, /moslime/reset, and per-tracker reset commands.
