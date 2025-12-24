use thiserror::Error;

pub type AIResult<T> = Result<T, AIError>;

#[derive(Error, Debug)]
pub enum AIError {
    #[error("Backend not available: {0}")]
    BackendUnavailable(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Connector failed: {0}")]
    ConnectorError(String),

    #[error("Parsing failed: {0}")]
    ParseError(String),

    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Stream interrupted")]
    StreamInterrupted,

    #[error("Unknown error: {0}")]
    Unknown(String),

    #[error("Network Error: {0}")]
    NetworkError(String),
}
