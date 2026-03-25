use thiserror::Error;

/// A specialized Result type for AI-related operations.
pub type AIResult<T> = Result<T, AIError>;

/// Defines the possible error conditions that can occur within the AI Mesh.
#[derive(Error, Debug)]
pub enum AIError {
    /// The requested AI backend (e.g., Ollama) is not reachable or responding.
    #[error("Backend not available: {0}")]
    BackendUnavailable(String),

    /// An error occurred while loading or applying system configuration.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// A generic failure within a specific AI connector (e.g., Qdrant, OpenAI).
    #[error("Connector failed: {0}")]
    ConnectorError(String),

    /// Failed to parse a response from the model or a tool.
    #[error("Parsing failed: {0}")]
    ParseError(String),

    /// An error related to agent persona definition or loading.
    #[error("Persona error: {0}")]
    Persona(String),

    /// Standard input/output failure.
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),

    /// The token stream was unexpectedly terminated.
    #[error("Stream interrupted")]
    StreamInterrupted,

    /// An unforeseen error that does not fit into other categories.
    #[error("Unknown error: {0}")]
    Unknown(String),

    /// A failure occurring during network communication with remote providers.
    #[error("Network Error: {0}")]
    NetworkError(String),
}
