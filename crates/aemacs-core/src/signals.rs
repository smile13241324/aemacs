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

/// Represents a signal that a tool is currently executing or has finished.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolProgressSignal {
    pub tool_name: String,
    pub is_running: bool,
}

/// Represents a periodic heartbeat signal for autonomous triggers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimePulseSignal {
    pub tick_count: u64,
    pub interval_seconds: u64,
}

// --- ACO-029: The Schema of Meaning ---

/// Represents high-level autonomous goals derived from environment signals.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AutonomousIntent {
    /// Review a specific change in the codebase.
    ReviewChange,
    /// Fix a build or runtime error.
    FixBuildError,
    /// Summarize external data (e.g. from webhooks).
    SummarizeExternalData,
    /// Perform a routine mental inventory or task check.
    RoutineCheck,
    /// Handle multiple simultaneous updates.
    BulkUpdate,
}

/// A context-rich snapshot of the environment at the time of a signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalContext {
    pub intent: AutonomousIntent,
    pub file_path: Option<String>,
    pub snippet: Option<String>,
    pub metadata: std::collections::HashMap<String, String>,
}
