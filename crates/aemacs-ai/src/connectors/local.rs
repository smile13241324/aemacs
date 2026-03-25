use async_trait::async_trait;
use futures::StreamExt;
use std::process::Stdio;
use tokio::process::Command;
use tokio_util::codec::{FramedRead, LinesCodec};
use tracing::{info, instrument};

use crate::error::{AIError, AIResult};
use crate::models::{AIRequest, Message, Role};
use crate::{AIBackend, AIResponseStream};

/// An AI backend implementation that executes a local binary to generate responses.
/// This is used for integration with local models or custom scripting.
pub struct LocalBackend {
    /// The physical path to the executable binary.
    pub bin_path: String,
}

impl LocalBackend {
    /// Initializes a new LocalBackend.
    pub fn new(binary: impl Into<String>) -> Self {
        Self {
            bin_path: binary.into(),
        }
    }
}

#[async_trait]
impl AIBackend for LocalBackend {
    /// Returns the descriptive name of this backend (the binary path).
    fn name(&self) -> &str {
        &self.bin_path
    }

    /// Checks if the binary is present and executable by calling it with '--version'.
    #[instrument(skip(self))]
    async fn health_check(&self) -> AIResult<()> {
        // Lets call the binary with "--version" to check if it's available.
        let output = Command::new(&self.bin_path)
            .arg("--version")
            .output()
            .await
            .map_err(|e| {
                AIError::BackendUnavailable(format!("Could not execute {}: {}", self.bin_path, e))
            })?;

        if output.status.success() {
            info!("Healthcheck passed for {}", self.bin_path);
            Ok(())
        } else {
            Err(AIError::BackendUnavailable(format!(
                "{} returned non-zero exit code",
                self.bin_path
            )))
        }
    }

    /// Executes the binary with the content of the last message as an argument.
    #[instrument(skip(self, request))]
    async fn complete(&self, request: AIRequest) -> AIResult<Message> {
        let last_message = request
            .messages
            .last()
            .ok_or(AIError::ConfigError("No messages in request".to_string()))?;

        info!("Sending request to local binary: {}", self.bin_path);

        let output = Command::new(&self.bin_path)
            .arg(last_message.content.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(AIError::IoError)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AIError::ConnectorError(format!("CLI Error: {}", stderr)));
        }

        let response = String::from_utf8(output.stdout)
            .map_err(|e| AIError::ParseError(format!("Invalid UTF-8: {}", e)))?;

        Ok(Message::new(Role::Assistant, response.trim().to_string()))
    }

    /// Spawns the binary and streams its standard output line-by-line.
    async fn stream(&self, request: AIRequest) -> AIResult<AIResponseStream> {
        let last_message = request
            .messages
            .last()
            .ok_or(AIError::ConfigError("No messages in request".to_string()))?;

        info!("Starting stream from local binary: {}", self.bin_path);

        let mut child = Command::new(&self.bin_path)
            .arg(last_message.content.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(AIError::IoError)?;

        let stdout = child.stdout.take().ok_or(AIError::ConnectorError(
            "Could not capture stdout".to_string(),
        ))?;

        // Translate the raw byte stream into a stream of events
        let stream = FramedRead::new(stdout, LinesCodec::new()).map(|result| match result {
            Ok(line) => Ok(crate::StreamEvent::Content(line + "\n")),
            Err(e) => Err(AIError::IoError(std::io::Error::other(e))),
        });

        // We spawn a small task to ensure the process ends cleanly
        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        Ok(Box::pin(stream))
    }
}
