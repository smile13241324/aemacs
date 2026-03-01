use async_trait::async_trait;
use futures::StreamExt;
use reqwest::{Client, header};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_util::io::StreamReader; // Wichtig für den Fix von vorhin
use tracing::{info, instrument};

use crate::{AIBackend, AIError, AIRequest, AIResponseStream, AIResult, Message};

#[derive(Clone)]
pub struct OpenAICompatibleBackend {
    client: Client,
    base_url: String,
    _api_key: Option<String>,
}

impl OpenAICompatibleBackend {
    pub fn new(base_url: impl Into<String>, api_key: Option<String>) -> Self {
        let mut headers = header::HeaderMap::new();

        if let Some(key) = &api_key {
            let mut auth_value = header::HeaderValue::from_str(&format!("Bearer {}", key))
                .expect("Invalid API Key chars");
            auth_value.set_sensitive(true);
            headers.insert(header::AUTHORIZATION, auth_value);
        }

        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(300)) // ACO-007: 5-minute timeout
            .connect_timeout(Duration::from_secs(10)) // Snappy connection check
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            base_url: base_url.into(),
            _api_key: api_key,
        }
    }

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
    fn name(&self) -> &str {
        "OpenAI Compatible REST"
    }

    async fn health_check(&self) -> AIResult<()> {
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AIError::BackendUnavailable(format!("Network Error: {}", e)))?;

        if resp.status().is_success() {
            info!("Healthcheck passed for {}", self.base_url);
            Ok(())
        } else {
            Err(AIError::BackendUnavailable(format!(
                "Server returned {}",
                resp.status()
            )))
        }
    }

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
            return Err(AIError::ConnectorError(format!(
                "API Error: {}",
                error_text
            )));
        }

        let openai_resp: OpenAIResponse = resp
            .json()
            .await
            .map_err(|e| AIError::ParseError(e.to_string()))?;

        openai_resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message)
            .ok_or(AIError::ParseError("No choices in response".to_string()))
    }

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
            return Err(AIError::ConnectorError(format!("API Error: {}", error_text)));
        }

        let byte_stream = resp.bytes_stream();
        let stream_with_io_error = byte_stream.map(|res| res.map_err(std::io::Error::other));
        let reader = StreamReader::new(stream_with_io_error);

        let line_stream = FramedRead::new(reader, LinesCodec::new()).map(|result| {
            match result {
                Ok(line) => {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with(':') || line == "data: [DONE]" {
                        return Ok(Vec::new());
                    }

                    if let Some(json_str) = line.strip_prefix("data: ") {
                        if let Ok(chunk) = serde_json::from_str::<OpenAIStreamChunk>(json_str) {
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
                    }

                    Ok(Vec::new())
                }
                Err(e) => Err(AIError::IoError(std::io::Error::other(e))),
            }
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
