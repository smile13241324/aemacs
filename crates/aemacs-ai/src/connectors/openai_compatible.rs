use async_trait::async_trait;
use futures::StreamExt;
use reqwest::{Client, header};
use serde::Deserialize;
use serde_json::json;
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_util::io::StreamReader; // Wichtig für den Fix von vorhin
use tracing::{info, instrument};

use crate::{AIBackend, AIError, AIRequest, AIResponseStream, AIResult, Message};

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
}
// -----------------------

#[async_trait]
impl AIBackend for OpenAICompatibleBackend {
    fn name(&self) -> &str {
        "OpenAI Compatible REST"
    }

    async fn health_check(&self) -> AIResult<()> {
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));

        let resp = self.client.get(&url).send().await
            .map_err(|e| AIError::BackendUnavailable(format!("Network Error: {}", e)))?;

        if resp.status().is_success() {
            info!("Healthcheck passed for {}", self.base_url);
            Ok(())
        } else {
            Err(AIError::BackendUnavailable(format!("Server returned {}", resp.status())))
        }
    }

    #[instrument(skip(self, request))]
    async fn complete(&self, request: AIRequest) -> AIResult<String> {
        let body = json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature,
            "stream": false
        });

        let resp = self.client.post(self.chat_url())
            .json(&body)
            .send()
            .await
            .map_err(|e| AIError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
             let error_text = resp.text().await.unwrap_or_default();
             return Err(AIError::ConnectorError(format!("API Error: {}", error_text)));
        }

        let openai_resp: OpenAIResponse = resp.json().await
            .map_err(|e| AIError::ParseError(e.to_string()))?;

        openai_resp.choices.into_iter().next()
            .map(|c| c.message.content)
            .ok_or(AIError::ParseError("No choices in response".to_string()))
    }

    async fn stream(&self, request: AIRequest) -> AIResult<AIResponseStream> {
        let body = json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature,
            "stream": true
        });

        let resp = self.client.post(self.chat_url())
            .json(&body)
            .send()
            .await
            .map_err(|e| AIError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
             let error_text = resp.text().await.unwrap_or_default();
             return Err(AIError::ConnectorError(format!("API Error: {}", error_text)));
        }

        // 1. fetch the byte stream
        let byte_stream = resp.bytes_stream();

        // 2. Error conversion (Reqwest -> IO)
        let stream_with_io_error = byte_stream
            .map(|res| res.map_err(std::io::Error::other)); // FIX 2: Modern

        // 3. Tokio Reader
        let reader = StreamReader::new(stream_with_io_error);

        // 4. FramedRead & Parsing Logic
        let line_stream = FramedRead::new(reader, LinesCodec::new())
            .map(|result| {
                match result {
                    Ok(line) => {
                        let line = line.trim();
                        // Ignore empty lines or SSE comments
                        if line.is_empty() || line.starts_with(':') || line == "data: [DONE]" {
                            return Ok("".to_string());
                        }

                        // Functional Chain (Pipeline) statt Nested Ifs
                        if let Some(content) = line.strip_prefix("data: ")
                            .and_then(|json| serde_json::from_str::<OpenAIStreamChunk>(json).ok())
                            .and_then(|mut chunk| chunk.choices.pop())
                            .and_then(|choice| choice.delta.content)
                        {
                            return Ok(content);
                        }

                        Ok("".to_string())
                    },
                    Err(e) => Err(AIError::IoError(std::io::Error::other(e))),
                }
            });

        // Filter empty strings
        let clean_stream = line_stream.filter(|res| {
             futures::future::ready(match res {
                 Ok(s) => !s.is_empty(),
                 Err(_) => true,
             })
        });

        Ok(Box::pin(clean_stream))
    }
}
