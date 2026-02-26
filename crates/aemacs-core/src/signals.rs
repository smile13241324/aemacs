use serde::{Deserialize, Serialize};

/// Represents a standard diagnostic signal (e.g., from an LSP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticSignal {
    pub file_path: String,
    pub line_number: usize,
    pub message: String,
    pub severity: String,
}

/// Represents a signal that a buffer has been modified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferModifiedSignal {
    pub file_path: String,
    pub diff_chunk: String,
}

/// Represents a generic or custom signal payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericSignal {
    pub details: String,
}
