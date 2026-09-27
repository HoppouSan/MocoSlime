//! Windows autostart via `HKCU\\...\\Run`.
//!
//! Non-Windows builds get a stub that reports success without side effects
//! so shared code (and tests) stay portable.

use crate::error::{ConfigError, Result};

const RUN_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "Mocoslime";
const LEGACY_VALUE_NAME: &str = "MoSlime-RS";

/// Enable or disable "Start with Windows".
pub fn set_autostart(enabled: bool) -> Result<()> {
    #[cfg(windows)]
    {
        set_autostart_windows(enabled, RUN_SUBKEY)
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Ok(())
    }
}

/// Current autostart state.
pub fn is_autostart_enabled() -> Result<bool> {
    #[cfg(windows)]
    {
        is_autostart_enabled_windows(RUN_SUBKEY)
    }
    #[cfg(not(windows))]
    {
        Ok(false)
    }
}

#[cfg(windows)]
fn app_exe() -> Result<String> {
    std::env::current_exe()
        .map_err(|e| ConfigError::IoError(e.to_string()))
        .map(|p| format!("\"{}\"", p.display()))
}

#[cfg(windows)]
fn set_autostart_windows(enabled: bool, subkey: &str) -> Result<()> {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(subkey)
        .map_err(|e| ConfigError::IoError(e.to_string()))?;
    if enabled {
        key.set_value(VALUE_NAME, &app_exe()?)
            .map_err(|e| ConfigError::IoError(e.to_string()))?;
        let _ = key.delete_value(LEGACY_VALUE_NAME);
        Ok(())
    } else {
        for value_name in [VALUE_NAME, LEGACY_VALUE_NAME] {
            match key.delete_value(value_name) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(ConfigError::IoError(e.to_string())),
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
fn is_autostart_enabled_windows(subkey: &str) -> Result<bool> {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey(subkey)
        .map_err(|e| ConfigError::IoError(e.to_string()))?;
    match key.get_value::<String, _>(VALUE_NAME) {
        Ok(_) => {
            let _ = key.delete_value(LEGACY_VALUE_NAME);
            return Ok(true);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(ConfigError::IoError(e.to_string())),
    }
    match key.get_value::<String, _>(LEGACY_VALUE_NAME) {
        Ok(_) => {
            // The previous install path may no longer exist after rename.
            // Point its existing preference at the currently running app.
            set_autostart_windows(true, subkey)?;
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(ConfigError::IoError(e.to_string())),
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    const TEST_SUBKEY: &str = "Software\\MoSlime-RS-Test\\Run";

    #[test]
    fn test_autostart_roundtrip_isolated_key() {
        set_autostart_windows(true, TEST_SUBKEY).unwrap();
        assert!(is_autostart_enabled_windows(TEST_SUBKEY).unwrap());
        set_autostart_windows(false, TEST_SUBKEY).unwrap();
        assert!(!is_autostart_enabled_windows(TEST_SUBKEY).unwrap());
        // Cleanup best effort.
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let _ = hkcu.delete_subkey(TEST_SUBKEY);
        let _ = hkcu.delete_subkey("Software\\MoSlime-RS-Test");
    }
}
