# Mocoslime 0.1.1 Release Plan

## Goal

Ship a focused maintenance release that makes language switching reliable,
closes background BLE work when the app exits, and keeps the source and release
package consistent with the Mocoslime name. Keep new tracking features out of
this patch release; plan them for a later minor version after validation.

## Scope

### 1. Localization and UI correctness

- Verify German, English, Japanese, and Simplified Chinese in every screen,
  dialog, tray menu, and validation/error message.
- Confirm Flutter's Material, Widgets, and Cupertino localization delegates are
  active for all four supported locales.
- Check the German Settings and Tracker forms for blank values, white-on-white
  text, clipped labels, and dropdowns that lose their selected value.
- Confirm a language change redraws the current screen immediately and remains
  selected after restarting the application.
- Fix any untranslated dynamic status text and German layout overflow found in
  the review.

### 2. Shutdown and BLE lifecycle

- Check closing through both the window close control and the tray's **Close
  application** command.
- Confirm streaming stops, BLE reconnect and event workers exit, the tray icon
  is removed, and the process terminates.
- Confirm a later launch can initialize BLE and tracking normally.

### 3. Branding and release files

- Keep the 2026 Mocoslime copyright, repository link, and independent-project
  disclaimer consistent in `LICENSE`, `NOTICE`, README, source archive, and
  release package.
- Keep legacy migration keys and third-party license/provenance material
  unchanged; document why they remain.
- Replace the invalid placeholder homepage in `installer/mocoslime.iss` with
  `https://github.com/HoppouSan/MocoSlime/` before building an installer.

## Version and artifacts

When scope is complete, set the app version consistently:

- Cargo workspace: `0.1.1`
- Flutter package: `0.1.1+2`
- Inno Setup installer: `0.1.1`
- Release folder and archive: `Mocoslime-0.1.1-windows-x64`

Rebuild the Rust workspace and Flutter Windows release from a clean checkout.
Stage the executable, matching `mocoslime_ffi.dll`, Flutter runtime, and `data`
directory together. Include the current license, notices, release notes, and
SHA-256 checksums in the portable archive and installer output.

## Release checks

- `cargo check --workspace` and `cargo build --release --workspace` succeed.
- `flutter analyze` and `flutter build windows --release` succeed.
- Manually inspect each of the four locales, especially German Settings fields
  and Tracker controls; change language, navigate screens, and restart once.
- Manually check shutdown via the window and tray, including a connected tracker
  and active streaming.
- Verify staged DLL and Flutter data files match the just-built artifacts;
  verify archive checksums after packaging.
- Before calling it a hardware-verified release, check BLE reconnect, SlimeVR
  server restart and handshake recovery with real trackers.

## Implementation status

- Implemented: locale delegates, darker populated form fields, translations for
  role/RSSI/log empty states, release version alignment, repository URL, and
  refreshed source and portable release packages.
- Static validation passed: Cargo workspace check/build and Flutter analyze/
  Windows release build.
- Not verified interactively: changing all four locales in the running app,
  field rendering after restart, and process exit through both close controls.
- Hardware validation with physical trackers and SlimeVR restart recovery is
  still outstanding. Do not describe the release as hardware-verified.
- Keep new tracking algorithms, profiles, and protocol features out of 0.1.1;
  use the next minor version for those changes.

## Completion criteria

The four locales render usable screens with populated controls, shutdown leaves
no Mocoslime background process, package files identify Mocoslime consistently,
all release builds and checksums pass, and release notes list remaining
hardware limitations.
