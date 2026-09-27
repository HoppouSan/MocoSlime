# Mocoslime open-source release readiness

This checkout is prepared for a first public source release, but it has not
been published. There is no Git metadata in the supplied workspace, so a
maintainer must connect it to the intended public repository before tagging.

## Included in this preparation

- Product identity changed to Mocoslime in the Flutter app, Windows executable,
  Windows file metadata, installer, Flutter package, Rust workspace package,
  docs, and SlimeVR handshake identification.
- Legacy internal crate and FFI names are retained (`moslime-core`,
  `moslime_ffi.dll`, `moslime_*` symbols) to avoid an unnecessary ABI break.
- First launch moves `%APPDATA%/MoSlime-RS` to `%APPDATA%/Mocoslime` when the
  destination does not already exist. Tracker-role and app settings files stay
  together. Existing autostart is moved to the current executable on first run.
- MIT license, upstream attribution, contribution/security guidance, ignore
  rules, and Windows CI workflow are present.
- The installer carries the project MIT license, upstream attribution, and
  third-party dependency inventory alongside the app.
- Release version is currently `0.1.0` across the Rust workspace, Flutter app,
  and installer.

## Required before publishing

1. Set the actual public repository/homepage URL in `installer/mocoslime.iss`.
   It currently contains an intentionally invalid placeholder, so installer
   links must not be shipped as-is.
2. Create/attach the intended Git remote, inspect history and tracked files,
   and confirm the maintainers control the repository and product name.
3. Audit source and binary assets for provenance. Keep the inherited MoSlime
   MIT notice; confirm the tray icon and any copied protocol/algorithm material
   can be distributed with this project.
4. Review `THIRD_PARTY_LICENSES.md` against the actual Cargo and Flutter release
   dependency trees. The installer includes the current inventory; replace it
   with a generated release bill of materials before distributing binaries.
5. Run the Windows CI workflow and a hardware release check with multiple
   Mocopi trackers, SlimeVR Server restart, BLE recovery, and standing/sitting/
   lying poses. There is no hardware test evidence in this checkout.
6. Decide on code signing, release support window, private vulnerability
   reporting, updater policy, and where users download official builds.
7. Build and inspect the final installer and a portable staged build. Verify
   clean install, upgrade from the former MoSlime-RS installer, config/role
   migration, uninstall behavior, tray autostart, and the four UI locales.
8. Update `CHANGELOG.md` with release-specific changes and known issues, tag
   `v0.1.0`, and publish source, binaries, checksums, and release notes together.

Until those checks pass, describe the project as prepared for release, not as a
tested or official binary release. No auto-updater, signed installer, or
cross-platform BLE support is included.

## SlimeVR and tracking feature plan

### Next: data-path correctness and visibility

- Add an optional SlimeVR loopback/status probe which reports UDP reachability
  separately from successful local packet writes. SlimeVR's current UDP sender
  does not prove that its server accepted or used a packet.
- Validate body-role uniqueness before streaming. Show conflicting trackers and
  offer a jump to role assignment; do not silently overwrite a user's choice.
- Show last fresh IMU time, receive rate, counter gaps, reconnect count, and
  SlimeVR output errors together for each tracker and for the whole session.
- Add a one-click SlimeVR restart recovery sequence: rediscover configured
  target, reopen output, resend sensor handshakes, and report each result.

### Then: useful tracking controls

- Add named tracker-layout profiles for role assignment and mount-flip settings.
  Preview all changes and require streaming to stop before switching layouts.
- Add per-tracker sensor-axis/mount verification with a guided static-pose
  check. Save only physical mounting corrections; leave body calibration in
  SlimeVR to avoid double-applied offsets.
- Add a short, opt-in session recorder for packet timestamps, counter gaps,
  reconnect events, and output health. Exclude raw packet values and device IDs
  by default; make retention and export controls explicit.
- Surface battery warnings and BLE signal trends without polling faster than
  the hardware/API can support.
- Add a reproducible tracking quality report for standing, sitting, lying,
  yaw drift, motion start latency, and recovery after packet loss. Compare
  filter settings from captured measurements before enabling any new filter.

### Future: SlimeVR integration

- Track supported SlimeVR Server versions and protocol fixtures; run packet
  compatibility checks against server changes before release.
- Document that UDP packet counters are local send counters, not server-side
  acknowledgement. Avoid promising server connectivity from packet writes.
- Keep OSC independent from SlimeVR. Provide explicit per-output enablement,
  separate status, and separate endpoint validation.
- Consider a SlimeVR plugin/API integration only if an official stable API is
  available and needed for capabilities that UDP cannot report, such as server
  acceptance or calibration state. Do not scrape private server internals.

## Support and privacy boundaries

The default diagnostics export redacts IP addresses, Bluetooth addresses,
device identifiers, and paths. Users can opt in to include private details.
Explain this choice in support instructions and ask users to review exports
before posting them publicly. Do not collect or upload diagnostics silently.
