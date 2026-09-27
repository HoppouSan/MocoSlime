use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum BleError {
    #[error("Bluetooth adapter not found")]
    AdapterNotFound,

    #[error("Bluetooth adapter not powered on")]
    AdapterNotPowered,

    #[error("Device not found: {identifier}")]
    DeviceNotFound { identifier: String },

    #[error("Connection failed: {reason}")]
    ConnectionFailed { reason: String },

    #[error("Connection timeout")]
    ConnectionTimeout,

    #[error("GATT service not found: {uuid}")]
    ServiceNotFound { uuid: String },

    #[error("GATT characteristic not found: {uuid}")]
    CharacteristicNotFound { uuid: String },

    #[error("GATT read failed: {reason}")]
    ReadFailed { reason: String },

    #[error("GATT write failed: {reason}")]
    WriteFailed { reason: String },

    #[error("GATT notify failed: {reason}")]
    NotifyFailed { reason: String },

    #[error("Device disconnected")]
    Disconnected,

    #[error("Invalid device state for operation: {state}")]
    InvalidState { state: String },

    #[error("Invalid Bluetooth address '{address}': {reason}")]
    InvalidAddress { address: String, reason: String },

    #[error("WinRT error: {code} - {message}")]
    WinRTError { code: i32, message: String },

    #[error("MTU negotiation failed")]
    MtuNegotiationFailed,

    #[error("Operation cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, BleError>;

impl From<windows::core::Error> for BleError {
    fn from(e: windows::core::Error) -> Self {
        BleError::WinRTError {
            code: e.code().0,
            message: e.message().to_string(),
        }
    }
}
