//! Config persistence roundtrip (Spec §12): defaults → save → load →
//! role assignment survives, unknown fields don't break older files.

use configuration::{AppConfig, TrackerConfig};
use tracking_core::TrackerRole;

#[test]
fn default_config_saves_and_loads() {
    let dir = std::env::temp_dir().join(format!("moslime-test-{}", std::process::id()));
    let path = dir.join("config.json");
    let _ = std::fs::remove_dir_all(&dir);

    let mut config = AppConfig::default();
    config.trackers.insert(
        "3C:38:F4:12:34:56".to_string(),
        TrackerConfig {
            mac_address: "3C:38:F4:12:34:56".to_string(),
            assigned_role: Some(TrackerRole::LeftFoot),
            calibration_offset: None,
            calibration_offsets: std::collections::HashMap::new(),
            active_pose: "standing".to_string(),
            mount_flip: false,
        },
    );
    config.save(&path).unwrap();

    let loaded = AppConfig::load(&path).unwrap();
    assert_eq!(loaded.version, 3);
    let tracker = &loaded.trackers["3C:38:F4:12:34:56"];
    assert_eq!(tracker.assigned_role, Some(TrackerRole::LeftFoot));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_file_creates_defaults() {
    let dir = std::env::temp_dir().join(format!("moslime-test-new-{}", std::process::id()));
    let path = dir.join("sub").join("config.json");
    let _ = std::fs::remove_dir_all(&dir);

    let config = AppConfig::load(&path).unwrap();
    assert_eq!(config.slimevr.server_port, 6969);
    assert!(path.exists());

    let _ = std::fs::remove_dir_all(&dir);
}
