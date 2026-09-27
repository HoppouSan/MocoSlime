use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum SlimeVRError {
    #[error("Invalid packet header")]
    InvalidHeader,

    #[error("Packet too short: expected at least {expected} bytes, got {actual}")]
    PacketTooShort { expected: usize, actual: usize },

    #[error("Unknown packet type: 0x{r#type:02x}")]
    UnknownPacketType { r#type: u8 },

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid sensor ID: {id}")]
    InvalidSensorId { id: u8 },
}

pub type Result<T> = std::result::Result<T, SlimeVRError>;
