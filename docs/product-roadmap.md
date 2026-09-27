# Mocoslime: Product and UI Roadmap

The goal is to connect Mocopi trackers reliably to SlimeVR and make the entire data path understandable. Completed work and planned improvements are identified separately.

## Principles

- SlimeVR remains responsible for body calibration and tracker alignment. Mocoslime handles BLE connections, hardware mount correction, role assignment, coordinate conversion, and forwarding.
- Add filters or drift correction only after measurements with real hardware. Extra smoothing can add latency, and gyroscope data alone cannot reliably eliminate yaw drift.
- Every error state should explain a specific cause and a useful next action.
- UI text, dialogs, tray menus, validation errors, and status messages must all be localized together.

## Already implemented

- Core UI language selection for `de`, `en`, `ja`, and `zh-CN`; invalid or legacy language codes fall back to English.
- Dashboard setup checklist with navigation to tracker and server configuration.
- SlimeVR output diagnostics with destination, packet/error counters, role status, and guidance when packets are not arriving.
- Diagnostics export redacts IP addresses, device identifiers, MAC addresses, and local paths by default. An explicit checkbox allows these details to be included when support needs them.
- Tray menu, status line, and key tracker/log states follow the selected language.

## Priority 0: Clear connection status

1. **Guided first launch:** check Bluetooth, find Mocopi trackers, discover or manually set the SlimeVR destination, assign roles, and confirm data flow.
2. **Connection overview:** show separate states for BLE, tracker data stream, and SlimeVR destination; display packet age, packets per second, and active tracker count.
3. **Actionable error help:** explain the cause, affected component, and next action for a missing SlimeVR destination, UDP error, missing tracker role, BLE disconnect, or stale data.
4. **Better role assignment:** change roles directly from the overview, visibly flag duplicate roles, and clearly identify unassigned trackers.

Acceptance criteria: A new user can connect to a server and confirm data flow without editing a configuration file. If the server or a tracker disconnects, the UI explains the difference and offers an appropriate action.

## Priority 1: SlimeVR operation and diagnostics

1. **SlimeVR status card:** summarize destination IP/port, discovery, last successful send, send errors, and last handshake time.
2. **Role and sensor status:** show each tracker's assigned SlimeVR role, whether it is active, and why it is not transmitting.
3. **Connection actions:** rediscover/reconnect the SlimeVR destination and resend tracker handshakes; report the result.
4. **Diagnostics export:** export logs, app/core versions, relevant configuration, and connection states as an editable support package. Let users review or redact device identifiers and local IPs before export.
5. **Runtime overview:** expose packet age/rate per tracker, BLE reconnects, SlimeVR send errors, and session uptime on a diagnostics screen.

Acceptance criteria: For every tracker that is not transmitting, users can tell whether BLE, role assignment, the SlimeVR destination, or UDP output is responsible. Support exports do not disclose personal data without review.

## Priority 2: Convenience and profiles

1. **Tracker profiles:** name, save, and switch between role assignments and SlimeVR destinations for different setups.
2. **Startup behavior:** optionally scan/connect at startup and clearly show when the app starts minimized or in the tray.
3. **Battery and maintenance:** show each tracker's battery, highlight low charge, and record connection quality over time where BLE exposes that data.
4. **Organized settings:** separate basic connection options from advanced tracking/diagnostic values and explain valid ranges for risky values.
5. **Optional OSC output:** show SlimeVR and OSC status separately; explain that OSC is an independent output.

Acceptance criteria: Switching profiles applies saved assignments predictably and never silently misassigns active trackers. SlimeVR and OSC can be enabled and diagnosed independently.

## Priority 3: Validation and quality

- Test extended operation with multiple Mocopi trackers.
- Exercise BLE loss, server restart, reconnection, and manual disconnect.
- Check standing, sitting, and lying with SlimeVR calibration.
- Observe packet age and loss under normal and loaded conditions.
- Evaluate further filters or drift handling only from these measurements.

## UI localization: German, English, Japanese, and Simplified Chinese

Planned locale codes are `de`, `en`, `ja`, and `zh-CN`. English (`en`) is the fallback for untranslated strings. The system language may be detected on first launch; users can change and save the selection at any time in Settings.

### Implementation plan

1. Move all visible strings from screens and widgets into a central localization catalog; avoid hard-coded text in widgets and error paths.
2. Cover navigation, Dashboard, tracker cards, calibration guidance, Settings, Logs, dialogs, tray menu, snackbars/errors, and setup flow.
3. Provide complete translations for all four languages. Keep technical product names such as Mocopi, SlimeVR, BLE, and OSC unchanged; translate technical terms consistently.
4. Localize plurals, numbers, dates/times, and dynamic error messages. Do not build translated text by concatenating fragments.
5. Check layouts with long German text, Japanese, and Simplified Chinese. Prevent font fallback issues and clipped controls.
6. Allow language changes without restarting and retain the selection after app restart.

### Localization acceptance criteria

- Every screen is translated into `de`, `en`, `ja`, and `zh-CN`, with no accidental mixed-language UI.
- Unknown or missing locale entries fall back to English safely.
- Changing the language immediately updates navigation and open screens.
- Layouts remain usable in all four languages; status icons supplement rather than replace text.
- Configuration stores a valid locale code and safely migrates invalid legacy values to `en`.

## Recommended delivery order

| Phase | Scope | Outcome |
|---|---|---|
| A | Connection overview and error help | Data flow and cause of errors at a glance |
| B | Localization foundation and complete four-language UI | Consistent UI in German, English, Japanese, and Simplified Chinese |
| C | Guided first launch and improved role assignment | Less manual setup |
| D | SlimeVR diagnostics and support export | Faster issue isolation and reporting |
| E | Profiles, runtime/battery convenience, and validation | Reliable daily use across multiple setups |

Show packet/protocol details only when the existing SlimeVR UDP data path can report them reliably. This roadmap does not depend on a new SlimeVR server API or changes to the SlimeVR project.
