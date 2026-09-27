# Mocoslime Windows Installer

The installer creates a `Mocoslime.exe` setup with a Start Menu entry, optional desktop shortcut, optional autostart, and an uninstaller. No Python runtime or Bluetooth command-line tools are required.

## System requirements

- **Windows 10 (build 10240+, 1809+ recommended) or Windows 11**, 64-bit. The installer blocks older systems (`MinVersion=10.0.10240`) because they lack the required WinRT Bluetooth LE APIs.
- A **Bluetooth LE (4.0+)** adapter with a Windows driver. Intel, MediaTek, Realtek, and Qualcomm adapters use the standard WinRT APIs; use the laptop or motherboard vendor's driver rather than a generic driver.
- Bluetooth must be **enabled** in Windows Settings. Mocopi trackers (`QM-SS1 …`) must be powered on and within range. Trackers already paired with Windows are also discovered.

## Option A — Inno Setup (recommended, open source)

1. Install [Inno Setup 6](https://jrsoftware.org/isinfo.php).
2. Build the application:
   ```powershell
   cargo build --release
   flutter build windows --release
   powershell -File installer/stage.ps1
   ```
   This copies `Mocoslime.exe`, `mocoslime_ffi.dll`, `data/`, and Flutter DLLs to `installer/stage/`. The Rust DLL **must** sit beside the executable so Dart can load it.
3. Compile the setup:
   ```powershell
   iscc installer/mocoslime.iss
   ```
   Output: `installer/output/Mocoslime-<version>-windows-x64.exe`.

## Option B — MSIX (Store/sideload)

Deferred: trusted installation requires a paid code-signing certificate. The app is compatible with the MSIX model (no admin rights, HKCU autostart, `%APPDATA%` config/logs). An MSIX manifest could later be derived from `mocoslime.iss` by converting the `[Registry]` autostart entry to a StartupTask.

## Autostart

Two options use the same registry value (`HKCU\...\Run\Mocoslime = "<exe>" --minimized`):

- Installer task **Start Mocoslime with Windows**, or
- In-app: Settings → General → Start with Windows (calls `mocoslime_set_autostart` → `configuration::set_autostart`).
