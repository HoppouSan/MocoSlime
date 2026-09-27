use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    #[error("Configuration file not found: {path}")]
    NotFound { path: String },

    #[error("Failed to parse configuration: {reason}")]
    ParseError { reason: String },

    #[error("Failed to write configuration: {reason}")]
    WriteError { reason: String },

    #[error("Invalid configuration value: {field} = {value}")]
    InvalidValue { field: String, value: String },

    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error("IO error: {0}")]
    IoError(String),
}

pub type Result<T> = std::result::Result<T, ConfigError>;
