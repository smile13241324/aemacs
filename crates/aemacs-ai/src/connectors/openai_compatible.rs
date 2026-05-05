use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::{Client, header};
use serde::Deserialize;
use serde_json::json;
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_util::io::StreamReader; // Wichtig für den Fix von vorhin
use tracing::{info, instrument};

use crate::{AIBackend, AIError, AIRequest, AIResponseStream, AIResult, Message};

/// An AI backend implementation that communicates with OpenAI-compatible REST APIs.
/// This is used to connect to services like Ollama, vLLM, or `OpenAI` itself.
#[derive(Clone)]
pub struct OpenAICompatibleBackend {
    /// The underlying HTTP client.
    client: Client,
    /// The base URL of the API (e.g., <http://localhost:11434/v1>).
    base_url: String,
    /// Optional API key for authenticated requests.
    _api_key: Option<String>,
}

impl std::fmt::Debug for OpenAICompatibleBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAICompatibleBackend")
            .field("base_url", &self.base_url)
            .field("_api_key", &self._api_key)
            .field("client", &"reqwest::Client")
            .finish()
    }
}

impl OpenAICompatibleBackend {
    /// Initializes a new `OpenAICompatibleBackend`.
    pub fn new(base_url: impl Into<String>, api_key: Option<String>) -> Self {
        let mut headers = header::HeaderMap::new();

        if let Some(key) = &api_key {
            let mut auth_value = header::HeaderValue::from_str(&format!("Bearer {key}"))
                .expect("Invalid API Key chars");
            auth_value.set_sensitive(true);
            headers.insert(header::AUTHORIZATION, auth_value);
        }

        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_mins(5)) // ACO-007: 5-minute timeout
            .connect_timeout(Duration::from_secs(10)) // Snappy connection check
            .build()
            .expect("Failed to build HTTP client");

        Self { client, base_url: base_url.into(), _api_key: api_key }
    }

    /// Internal helper to construct the full completion URL.
    fn chat_url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }
}

// --- JSON structures ---
#[derive(Deserialize, Debug)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Deserialize, Debug)]
struct OpenAIChoice {
    message: Message,
}

#[derive(Deserialize, Debug)]
struct OpenAIStreamChunk {
    choices: Vec<OpenAIStreamChoice>,
}

#[derive(Deserialize, Debug)]
struct OpenAIStreamChoice {
    delta: OpenAIDelta,
}

#[derive(Deserialize, Debug)]
struct OpenAIDelta {
    content: Option<String>,
    tool_calls: Option<Vec<crate::models::ToolCall>>,
}
// -----------------------

#[async_trait]
impl AIBackend for OpenAICompatibleBackend {
    /// Returns the descriptive name of this backend.
    fn name(&self) -> &'static str {
        "OpenAI Compatible REST"
    }

    /// Performs a connectivity and health check against the API endpoint.
    async fn health_check(&self) -> AIResult<()> {
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AIError::BackendUnavailable(format!("Network Error: {e}")))?;

        if resp.status().is_success() {
            info!("Healthcheck passed for {}", self.base_url);
            Ok(())
        } else {
            Err(AIError::BackendUnavailable(format!("Server returned {}", resp.status())))
        }
    }

    /// Sends a non-streaming request to the backend and awaits the full response.
    #[instrument(skip(self, request))]
    async fn complete(&self, request: AIRequest) -> AIResult<Message> {
        info!("🤖 [Ollama] Requesting completion from model: {}", request.model);
        let body = json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature,
            "stream": false,
            "options": request.options,
            "tools": request.tools
        });

        let resp = self
            .client
            .post(self.chat_url())
            .json(&body)
            .send()
            .await
            .map_err(|e| AIError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            return Err(AIError::ConnectorError(format!("API Error: {error_text}")));
        }

        let openai_resp: OpenAIResponse =
            resp.json().await.map_err(|e| AIError::ParseError(e.to_string()))?;

        openai_resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message)
            .ok_or(AIError::ParseError("No choices in response".to_string()))
    }

    /// Sends a streaming request to the backend and returns a stream of tokens/events.
    async fn stream(&self, request: AIRequest) -> AIResult<AIResponseStream> {
        info!("🤖 [Ollama] Requesting stream from model: {}", request.model);
        let body = json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature,
            "stream": true,
            "options": request.options,
            "tools": request.tools
        });

        let resp = self
            .client
            .post(self.chat_url())
            .json(&body)
            .send()
            .await
            .map_err(|e| AIError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            return Err(AIError::ConnectorError(format!("API Error: {error_text}")));
        }

        let byte_stream = resp.bytes_stream();
        let stream_with_io_error = byte_stream.map(|res| res.map_err(std::io::Error::other));
        let reader = StreamReader::new(stream_with_io_error);

        let line_stream = FramedRead::new(reader, LinesCodec::new()).map(|result| match result {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() || line.starts_with(':') || line == "data: [DONE]" {
                    return Ok(Vec::new());
                }

                if let Some(json_str) = line.strip_prefix("data: ")
                    && let Ok(chunk) = serde_json::from_str::<OpenAIStreamChunk>(json_str)
                {
                    let mut events = Vec::new();
                    for choice in chunk.choices {
                        if let Some(content) = choice.delta.content {
                            events.push(crate::StreamEvent::Content(content));
                        }
                        if let Some(tool_calls) = choice.delta.tool_calls {
                            for tc in tool_calls {
                                events.push(crate::StreamEvent::ToolCall(tc));
                            }
                        }
                    }
                    return Ok(events);
                }

                Ok(Vec::new())
            },
            Err(e) => Err(AIError::IoError(std::io::Error::other(e))),
        });

        // Flatten the events and filter empty ones
        let event_stream = line_stream
            .map(|res| match res {
                Ok(events) => events.into_iter().map(Ok).collect::<Vec<_>>(),
                Err(e) => vec![Err(e)],
            })
            .flat_map(futures::stream::iter);

        Ok(Box::pin(event_stream))
    }
}

#[cfg(test)]
mod tests {
    use tokio::{io::AsyncWriteExt, net::TcpListener};

    use super::*;
    use crate::models::AIRequest;

    #[tokio::test]
    async fn test_streaming_hydra_cage_quest() -> anyhow::Result<()> {
        // QUEST: Verify the connector handles fragmented JSON streams correctly.

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let base_url = format!("http://{addr}");

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0; 1024];
            let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut buf).await;

            // Send a standard HTTP response
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n")
                .await
                .unwrap();

            // Part 1: Start of the line
            socket.write_all(b"data: {\"choices\":[{\"d").await.unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Part 2: Rest of the line + [DONE] signal
            socket
                .write_all(b"elta\":{\"content\":\"Hello \"}}]}\n\ndata: [DONE]\n\n")
                .await
                .unwrap();
        });

        let backend = OpenAICompatibleBackend::new(base_url, None);
        let request = AIRequest::default();
        let mut stream = backend.stream(request).await?;

        let mut chunks = Vec::new();
        while let Some(event) = stream.next().await {
            if let Ok(crate::StreamEvent::Content(c)) = event {
                chunks.push(c);
            }
        }

        assert_eq!(chunks.len(), 1, "The 'Streaming-Hydra' failed! Expected 1 content chunk.");
        assert_eq!(chunks[0], "Hello ", "Fragmented JSON was not correctly reassembled!");

        Ok(())
    }
}
