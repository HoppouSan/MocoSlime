use thiserror::Error;

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum FfiError {
    #[error("Success")]
    Success = 0,
    #[error("Not initialized")]
    NotInitialized = 1,
    #[error("Invalid input")]
    InvalidInput = 2,
    #[error("Serialization error")]
    SerializationError = 3,
    #[error("Channel full")]
    ChannelFull = 4,
    #[error("Internal error")]
    InternalError = 5,
    #[error("Already initialized")]
    AlreadyInitialized = 6,
}

pub type Result<T> = std::result::Result<T, FfiError>;
