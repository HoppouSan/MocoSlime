# SlimeVR / OSC Output (P3)

## SlimeVR

- Set the destination with `set_target(ip, port, auto)`. `mocoslime_get_slimevr_status` returns `SlimevrStatus {enabled, ip, port, auto_discovered, packets_sent, errors}`.
- Autodiscovery listens for about 3 seconds for `Hey OVR ` and binds to `:6969`. If that port is already in use (for example, by a local server), the app does not crash; it falls back to the configured destination.
- Events: `SlimeVRDiscovered(ip,port)` when found, `SlimeVRConnected(ip,port,auto)` on connection (`auto=true` makes the GUI show `(Auto)`), and `SlimeVRDisconnected` on stop.
- Handshakes are sent when streaming starts and for trackers that are already streaming at startup (`resend_handshakes`, FFI `mocoslime_resend_handshake`, Dashboard button **Send handshake**). The server will not create sensors without a handshake.
- The Dashboard card shows `ip:port (Auto)` and packet counters. Quick action **SlimeVR Reconnect** calls `startStreaming`; status is polled every 2 seconds.
- Settings validate IP (IPv4/hostname), port 1–65535, and rate 0–200.

## OSC

- `OscOutput` retains its send behavior (queue of 64, rate limit, bundle mode). New `send_test` emits `OscTestSent`, and the GUI displays the timestamp.
- The Dashboard OSC card shows `Preset ip:port • n pkt • Test HH:MM:SS`. Settings provide one-click `Generic/VRChat/Custom` ChoiceChips and a dropdown. IP, port (1–65535), and rate (0–240) are validated.
- FFI `sendOscTest` retries when the channel is full.
