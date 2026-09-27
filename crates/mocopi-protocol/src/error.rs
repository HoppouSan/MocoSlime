use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("Invalid packet length: expected {expected}, got {actual}")]
    InvalidPacketLength { expected: usize, actual: usize },

    #[error("Unknown packet type: 0x{r#type:02x}")]
    UnknownPacketType { r#type: u8 },

    #[error("Invalid quaternion data")]
    InvalidQuaternion,

    #[error("Invalid acceleration data")]
    InvalidAcceleration,

    #[error("Invalid battery data")]
    InvalidBattery,

    #[error("Checksum mismatch")]
    ChecksumMismatch,

    #[error("Packet corrupted: {reason}")]
    Corrupted { reason: String },

    #[error("Not enough data for parsing: need {need}, have {have}")]
    InsufficientData { need: usize, have: usize },
}

pub type Result<T> = std::result::Result<T, ProtocolError>;
