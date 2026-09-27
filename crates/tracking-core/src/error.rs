use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TrackingError {
    #[error("Invalid coordinate system conversion")]
    InvalidCoordinateSystem,

    #[error("Calibration not initialized")]
    CalibrationNotInitialized,

    #[error("Invalid tracker role: {role}")]
    InvalidTrackerRole { role: String },

    #[error("Filter configuration error: {reason}")]
    FilterConfigError { reason: String },

    #[error("Timestamp out of order: expected > {expected}, got {actual}")]
    TimestampOutOfOrder { expected: u64, actual: u64 },

    #[error("Invalid quaternion: not normalized")]
    InvalidQuaternion,
}

pub type Result<T> = std::result::Result<T, TrackingError>;
