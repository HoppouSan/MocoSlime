use thiserror::Error;
use tracking_core::TrackingError;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    #[error("Tracker not found: {id}")]
    TrackerNotFound { id: String },

    #[error("Tracker already exists: {id}")]
    TrackerAlreadyExists { id: String },

    #[error("Invalid tracker state for operation: {state}")]
    InvalidTrackerState { state: String },

    #[error("SlimeVR connection failed: {reason}")]
    SlimeVRConnectionFailed { reason: String },

    #[error("SlimeVR not connected")]
    SlimeVRNotConnected,

    #[error("Configuration error: {reason}")]
    ConfigurationError { reason: String },

    #[error("BLE error: {0}")]
    BleError(String),

    #[error("Tracking error: {0}")]
    TrackingError(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Calibration in progress")]
    CalibrationInProgress,

    #[error("Operation timed out")]
    Timeout,
}

impl From<TrackingError> for CoreError {
    fn from(e: TrackingError) -> Self {
        CoreError::TrackingError(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
