use crate::error::{ConfigError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracking_core::{PoseDetectorConfig, SkeletonConfig, TrackerRole};

fn default_active_pose() -> String {
    "standing".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackerConfig {
    pub mac_address: String,
    pub assigned_role: Option<TrackerRole>,
    /// Physical sensor mounting correction; SlimeVR handles body calibration.
    #[serde(default)]
    pub mount_flip: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothConfig {
    pub auto_scan: bool,
    pub auto_connect: bool,
    pub reconnect_delay_ms: u64,
    pub connection_timeout_ms: u64,
    pub max_trackers: usize,
    pub scan_duration_ms: u64,
    #[serde(default = "default_rssi_min")]
    pub rssi_min_dbm: i16,
}

fn default_rssi_min() -> i16 {
    -85
}

impl Default for BluetoothConfig {
    fn default() -> Self {
        Self {
            auto_scan: true,
            auto_connect: true,
            reconnect_delay_ms: 5000,
            connection_timeout_ms: 15000,
            max_trackers: 12,
            scan_duration_ms: 10000,
            rssi_min_dbm: -85,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlimeVRConfig {
    pub server_ip: String,
    pub server_port: u16,
    pub enabled: bool,
    pub packet_rate: u32,
    pub auto_discover: bool,
}

impl Default for SlimeVRConfig {
    fn default() -> Self {
        Self {
            server_ip: "127.0.0.1".to_string(),
            server_port: 6969,
            enabled: true,
            packet_rate: 128,
            auto_discover: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingConfig {
    pub coordinate_system: String,
    pub enable_filtering: bool,
    pub filter_slerp_factor: f32,
    pub filter_accel_alpha: f32,
    #[serde(default)]
    pub skeleton: SkeletonConfig,
    #[serde(default = "default_auto_pose")]
    pub enable_auto_pose: bool,
    #[serde(default = "default_active_pose")]
    pub active_pose: String,
    #[serde(default)]
    pub pose_detector: PoseDetectorConfig,
}

fn default_auto_pose() -> bool {
    true
}

impl Default for TrackingConfig {
    fn default() -> Self {
        Self {
            coordinate_system: "SlimeVR".to_string(),
            enable_filtering: true,
            filter_slerp_factor: 0.1,
            filter_accel_alpha: 0.2,
            skeleton: SkeletonConfig::default(),
            enable_auto_pose: true,
            active_pose: default_active_pose(),
            pose_detector: PoseDetectorConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub start_with_windows: bool,
    pub minimize_to_tray: bool,
    pub start_minimized: bool,
    pub language: String,
    pub log_level: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            start_with_windows: false,
            minimize_to_tray: true,
            start_minimized: false,
            language: "en".to_string(),
            log_level: "INFO".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub bluetooth: BluetoothConfig,
    pub slimevr: SlimeVRConfig,
    pub tracking: TrackingConfig,
    pub trackers: HashMap<String, TrackerConfig>,
    pub version: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            bluetooth: BluetoothConfig::default(),
            slimevr: SlimeVRConfig::default(),
            tracking: TrackingConfig::default(),
            trackers: HashMap::new(),
            version: 4,
        }
    }
}

impl AppConfig {
    pub fn load(path: &PathBuf) -> Result<Self> {
        if !path.exists() {
            let default = Self::default();
            default.save(path)?;
            return Ok(default);
        }

        let content =
            std::fs::read_to_string(path).map_err(|e| ConfigError::IoError(e.to_string()))?;

        match serde_json::from_str::<AppConfig>(&content) {
            Ok(mut config) => {
                config.migrate();
                Ok(config)
            }
            Err(_) => {
                // Korrupte Datei (Kill während Write): Backup versuchen,
                // sonst Defaults. Startet nie mit Fehler.
                let bak = Self::backup_path(path);
                if let Ok(bak_content) = std::fs::read_to_string(&bak) {
                    if let Ok(mut config) = serde_json::from_str::<AppConfig>(&bak_content) {
                        config.migrate();
                        let _ = config.save(path);
                        return Ok(config);
                    }
                }
                let default = Self::default();
                default.save_without_backup(path)?;
                Ok(default)
            }
        }
    }

    /// Atomares Schreiben: Temp-Datei + Rename (nie halbe Dateien).
    /// Vorher wird die alte Datei nach `config.bak` kopiert (nur wenn gültig).
    pub fn save(&self, path: &PathBuf) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ConfigError::IoError(e.to_string()))?;
        }

        if path.exists() {
            if let Ok(existing) = std::fs::read_to_string(path) {
                if serde_json::from_str::<AppConfig>(&existing).is_ok() {
                    let _ = std::fs::copy(path, Self::backup_path(path));
                }
            }
        }

        let content = serde_json::to_string_pretty(self).map_err(|e| ConfigError::WriteError {
            reason: e.to_string(),
        })?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, content).map_err(|e| ConfigError::IoError(e.to_string()))?;
        std::fs::rename(&tmp, path).map_err(|e| ConfigError::IoError(e.to_string()))?;

        Ok(())
    }

    fn save_without_backup(&self, path: &PathBuf) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ConfigError::IoError(e.to_string()))?;
        }
        let content = serde_json::to_string_pretty(self).map_err(|e| ConfigError::WriteError {
            reason: e.to_string(),
        })?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, content).map_err(|e| ConfigError::IoError(e.to_string()))?;
        std::fs::rename(&tmp, path).map_err(|e| ConfigError::IoError(e.to_string()))?;
        Ok(())
    }

    fn backup_path(path: &PathBuf) -> PathBuf {
        path.with_extension("bak")
    }

    fn migrate(&mut self) {
        if self.tracking.active_pose.is_empty() {
            self.tracking.active_pose = default_active_pose();
        }
        // v4 removes app-local calibration offsets. Old unknown fields are
        // ignored by serde and disappear on the next save; SlimeVR owns these.
        self.version = 4;
    }

    pub fn get_config_dir() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        let current = base.join("Mocoslime");
        let legacy = base.join("MoSlime-RS");

        // Keep existing users' settings, roles, logs and backups when the
        // product is renamed. Rename the complete directory once; if Windows
        // temporarily blocks it, keep using the legacy directory safely.
        if !current.exists() && legacy.exists() {
            if std::fs::rename(&legacy, &current).is_err() {
                return legacy;
            }
        }
        current
    }

    pub fn default_config_path() -> PathBuf {
        Self::get_config_dir().join("config.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.version, 4);
        assert!(config.bluetooth.auto_connect);
        assert_eq!(config.slimevr.server_port, 6969);
        assert!(config.tracking.enable_auto_pose);
        assert!(config.tracking.skeleton.validate().is_ok());
    }

    #[test]
    #[test]
    fn test_config_serialization() {
        let config = AppConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.version, parsed.version);
    }

    #[test]
    #[test]
    fn test_migrate_v1_to_v2() {
        let v1 = r#"{
            "general":{"start_with_windows":false,"minimize_to_tray":true,"start_minimized":false,"language":"en","log_level":"INFO"},
            "bluetooth":{"auto_scan":true,"auto_connect":true,"reconnect_delay_ms":5000,"connection_timeout_ms":10000,"max_trackers":12,"scan_duration_ms":10000},
            "slimevr":{"server_ip":"127.0.0.1","server_port":6969,"enabled":true,"packet_rate":128,"auto_discover":true},
            "tracking":{"coordinate_system":"SlimeVR","enable_filtering":true,"filter_slerp_factor":0.1,"filter_accel_alpha":0.2},
            "trackers":{"AA:BB:CC":{"mac_address":"AA:BB:CC","assigned_role":"Head"}},
            "version":1
        }"#;
        let mut parsed: AppConfig = serde_json::from_str(v1).unwrap();
        parsed.migrate();
        assert_eq!(parsed.version, 4);
        let t = parsed.trackers.get("AA:BB:CC").unwrap();
        assert_eq!(t.assigned_role, Some(TrackerRole::Head));
        assert!(parsed.tracking.enable_auto_pose);
    }

    #[test]
    #[test]
    fn test_corrupt_config_falls_back_to_backup() {
        let dir = std::env::temp_dir().join(format!("mocoslime-test-bak-{}", std::process::id()));
        let path = dir.join("config.json");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut config = AppConfig::default();
        config.slimevr.server_port = 6970;
        config.save(&path).unwrap();
        // Zweites Save legt das Backup an.
        config.slimevr.server_port = 6971;
        config.save(&path).unwrap();
        // Hauptdatei korrumpieren (Kill während Write).
        std::fs::write(&path, "{trailing characters!!!").unwrap();

        let loaded = AppConfig::load(&path).unwrap();
        assert_eq!(loaded.slimevr.server_port, 6970);
        // Hauptdatei wurde aus dem Backup wiederhergestellt.
        let reloaded = AppConfig::load(&path).unwrap();
        assert_eq!(reloaded.slimevr.server_port, 6970);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_corrupt_config_and_backup_gives_defaults() {
        let dir = std::env::temp_dir().join(format!("mocoslime-test-bak2-{}", std::process::id()));
        let path = dir.join("config.json");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "{broken").unwrap();
        std::fs::write(AppConfig::backup_path(&path), "{also broken").unwrap();

        let loaded = AppConfig::load(&path).unwrap();
        assert_eq!(loaded.slimevr.server_port, 6969);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
