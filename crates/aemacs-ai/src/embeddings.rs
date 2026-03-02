use crate::{AIError, AIResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct OllamaEmbedder {
    client: Client,
    base_url: String,
    model: String,
}

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    prompt: &'a str,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

impl OllamaEmbedder {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            model: model.into(),
        }
    }

    pub async fn embed(&self, text: &str) -> AIResult<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.base_url.trim_end_matches('/'));

        let body = EmbeddingRequest {
            model: &self.model,
            prompt: text,
        };

        let res = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AIError::NetworkError(e.to_string()))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();

            if status == reqwest::StatusCode::NOT_FOUND {
                return Err(AIError::ConnectorError(format!(
                    "Embedding Error: 404 Not Found. Ensure the model '{}' is pulled (ollama pull {}) and the URL is correct: {}",
                    self.model, self.model, url
                )));
            }

            return Err(AIError::ConnectorError(format!(
                "Embedding Error ({}): {}",
                status, err_text
            )));
        }

        let response: EmbeddingResponse = res
            .json()
            .await
            .map_err(|e| AIError::ParseError(e.to_string()))?;

        Ok(response.embedding)
    }
}
