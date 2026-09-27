pub mod autostart;
pub mod config;
pub mod error;

pub use autostart::{is_autostart_enabled, set_autostart};
pub use config::{
    AppConfig, BluetoothConfig, GeneralConfig, SlimeVRConfig, TrackerConfig, TrackingConfig,
};
pub use error::{ConfigError, Result};
