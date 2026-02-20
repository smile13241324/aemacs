use async_trait::async_trait;
use futures::StreamExt;
use std::process::Stdio;
use tokio::process::Command;
use tokio_util::codec::{FramedRead, LinesCodec};
use tracing::{info, instrument};

use crate::error::{AIError, AIResult};
use crate::models::{AIRequest, Message, Role};
use crate::{AIBackend, AIResponseStream};

pub struct LocalBackend {
    pub bin_path: String,
}

impl LocalBackend {
    pub fn new(binary: impl Into<String>) -> Self {
        Self {
            bin_path: binary.into(),
        }
    }
}

#[async_trait]
impl AIBackend for LocalBackend {
    fn name(&self) -> &str {
        &self.bin_path
    }

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

        // Translate the raw byte stream into a stream of strings (lines)
        let stream = FramedRead::new(stdout, LinesCodec::new()).map(|result| match result {
            Ok(line) => Ok(line + "\n"),
            Err(e) => Err(AIError::IoError(std::io::Error::other(e))),
        });

        // We spawn a small task to ensure the process ends cleanly
        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        Ok(Box::pin(stream))
    }
}
