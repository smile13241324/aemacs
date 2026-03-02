use crate::embeddings::OllamaEmbedder;
use crate::{AIError, AIResult};
use qdrant_client::Payload;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::r#match::MatchValue;
use qdrant_client::qdrant::{
    Condition, CreateCollection, DeletePointsBuilder, Distance, FieldCondition, Filter, PointId,
    PointStruct, SearchPoints, ScrollPoints, UpsertPoints, VectorParams, VectorsConfig,
    condition::ConditionOneOf, vectors_config::Config,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct MemoryResult {
    pub id: String,
    pub content: String,
    pub metadata: HashMap<String, Value>,
}

pub struct KnowledgeBase {
    client: Qdrant,
    embedder: OllamaEmbedder,
}

impl KnowledgeBase {
    pub fn new(qdrant_url: impl Into<String>, ollama_url: impl Into<String>) -> AIResult<Self> {
        let client = Qdrant::from_url(&qdrant_url.into())
            .build()
            .map_err(|e| AIError::ConnectorError(format!("Qdrant Init Error: {}", e)))?;

        let embedder = OllamaEmbedder::new(ollama_url, "nomic-embed-text");

        Ok(Self { client, embedder })
    }

    /// Fetches the most recent 'CORE' or 'INSIGHT' directives for Mnemic Reflection.
    pub async fn get_core_directives(&self, collection_name: &str) -> anyhow::Result<Vec<String>> {
        let mut cat_conditions = Vec::new();
        for cat in &["CORE", "INSIGHT"] {
            cat_conditions.push(Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "category".to_string(),
                    r#match: Some(qdrant_client::qdrant::Match {
                        match_value: Some(MatchValue::Keyword(cat.to_string())),
                    }),
                    ..Default::default()
                })),
            });
        }

        let filter = Filter {
            should: cat_conditions,
            ..Default::default()
        };

        let request = ScrollPoints {
            collection_name: collection_name.to_string(),
            filter: Some(filter),
            limit: Some(10), // Fetch up to 10 core directives
            with_payload: Some(true.into()),
            ..Default::default()
        };

        let scroll_result = self.client.scroll(request).await.map_err(|e| {
            anyhow::anyhow!("Failed to scroll core directives: {}", e)
        })?;

        let mut contents = Vec::new();
        for point in scroll_result.result {
            if let Some(content) = point.payload.get("content").and_then(|v| v.as_str()) {
                contents.push(content.to_string());
            }
        }

        Ok(contents)
    }

    /// Fetches all chunks associated with a specific message_id, sorted by their original chunk index.
    pub async fn fetch_by_message_id(
        &self,
        collection_name: &str,
        message_id: &str,
    ) -> AIResult<Vec<MemoryResult>> {
        let filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "message_id".to_string(),
                    r#match: Some(qdrant_client::qdrant::Match {
                        match_value: Some(MatchValue::Keyword(message_id.to_string())),
                    }),
                    ..Default::default()
                })),
            }],
            ..Default::default()
        };

        let request = ScrollPoints {
            collection_name: collection_name.to_string(),
            filter: Some(filter),
            limit: Some(100), // Generous limit for a single message
            with_payload: Some(true.into()),
            ..Default::default()
        };

        let scroll_result = self.client.scroll(request).await.map_err(|e| {
            AIError::ConnectorError(format!("Failed to fetch by message_id: {}", e))
        })?;

        let mut results: Vec<(usize, MemoryResult)> = scroll_result
            .result
            .into_iter()
            .filter_map(|point| {
                let content = point
                    .payload
                    .get("content")
                    .and_then(|v| v.as_str().map(|s| s.to_string()))?;

                let mut metadata = HashMap::new();
                let mut chunk_idx = 0;

                for (k, v) in point.payload {
                    if k == "chunk_index" {
                        if let Some(s) = v.as_str() {
                            chunk_idx = s.parse::<usize>().unwrap_or(0);
                        } else if let Some(n) = v.as_integer() {
                            chunk_idx = n as usize;
                        }
                    }
                    if k != "content" {
                        metadata.insert(k, v.into());
                    }
                }

                let id = match point.id {
                    Some(id) => match id.point_id_options {
                        Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(n)) => {
                            n.to_string()
                        }
                        Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(s)) => s,
                        None => "unknown".to_string(),
                    },
                    None => "unknown".to_string(),
                };

                Some((
                    chunk_idx,
                    MemoryResult {
                        id,
                        content,
                        metadata,
                    },
                ))
            })
            .collect();

        // Sort by chunk index to ensure chronological reconstruction
        results.sort_by_key(|k| k.0);

        Ok(results.into_iter().map(|(_, res)| res).collect())
    }

    pub async fn ensure_collection(&self, collection_name: &str, dim: u64) -> AIResult<()> {
        if !self
            .client
            .collection_exists(collection_name)
            .await
            .map_err(|e| AIError::ConnectorError(e.to_string()))?
        {
            self.client
                .create_collection(CreateCollection {
                    collection_name: collection_name.to_string(),
                    vectors_config: Some(VectorsConfig {
                        config: Some(Config::Params(VectorParams {
                            size: dim,
                            distance: Distance::Cosine.into(),
                            ..Default::default()
                        })),
                    }),
                    ..Default::default()
                })
                .await
                .map_err(|e| {
                    AIError::ConnectorError(format!("Failed to create collection: {}", e))
                })?;
        }
        Ok(())
    }

    pub async fn add_document(
        &self,
        collection_name: &str,
        content: &str,
        metadata: Option<HashMap<String, String>>,
    ) -> AIResult<()> {
        // Nomic v1.5 requires prefix for documents
        let content_for_embedding = format!("search_document: {}", content);
        let embedding = self.embedder.embed(&content_for_embedding).await?;
        let id = Uuid::new_v4();

        let mut payload = Payload::new();
        // Store ORIGINAL content in payload
        payload.insert("content", json!(content));

        if let Some(meta) = metadata {
            for (k, v) in meta {
                payload.insert(k, json!(v));
            }
        }

        let point = PointStruct::new(id.to_string(), embedding, payload);

        self.client
            .upsert_points(UpsertPoints {
                collection_name: collection_name.to_string(),
                points: vec![point],
                ..Default::default()
            })
            .await
            .map_err(|e| AIError::ConnectorError(format!("Upsert failed: {}", e)))?;

        Ok(())
    }

    pub async fn search(
        &self,
        collection_name: &str,
        query: &str,
        limit: u64,
        score_threshold: Option<f32>,
        agent_id: Option<&str>,
        categories: Option<Vec<&str>>,
    ) -> AIResult<Vec<MemoryResult>> {
        // Nomic v1.5 requires prefix for queries
        let query_for_embedding = format!("search_query: {}", query);
        let vector = self.embedder.embed(&query_for_embedding).await?;

        // Apply default threshold of 0.75 (High Relevance) if not specified.
        let threshold = score_threshold.unwrap_or(0.75);

        let mut must_conditions = Vec::new();

        if let Some(id) = agent_id {
            must_conditions.push(Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "agent_id".to_string(),
                    r#match: Some(qdrant_client::qdrant::Match {
                        match_value: Some(MatchValue::Keyword(id.to_string())),
                    }),
                    ..Default::default()
                })),
            });
        }

        if let Some(cats) = categories {
            let mut cat_conditions = Vec::new();
            for cat in cats {
                cat_conditions.push(Condition {
                    condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                        key: "category".to_string(),
                        r#match: Some(qdrant_client::qdrant::Match {
                            match_value: Some(MatchValue::Keyword(cat.to_string())),
                        }),
                        ..Default::default()
                    })),
                });
            }
            // Use should (OR) for categories
            must_conditions.push(Condition {
                condition_one_of: Some(ConditionOneOf::Filter(Filter {
                    should: cat_conditions,
                    ..Default::default()
                })),
            });
        }

        let filter = if must_conditions.is_empty() {
            None
        } else {
            Some(Filter {
                must: must_conditions,
                ..Default::default()
            })
        };

        let search_result = self
            .client
            .search_points(SearchPoints {
                collection_name: collection_name.to_string(),
                vector,
                limit,
                score_threshold: Some(threshold),
                with_payload: Some(true.into()),
                filter,
                ..Default::default()
            })
            .await
            .map_err(|e| AIError::ConnectorError(format!("Search failed: {}", e)))?;

        let results = search_result
            .result
            .into_iter()
            .filter_map(|point| {
                let content = point
                    .payload
                    .get("content")
                    .and_then(|v| v.as_str().map(|s| s.to_string()))?;

                let mut metadata = HashMap::new();
                for (k, v) in point.payload {
                    if k != "content" {
                        metadata.insert(k, v.into());
                    }
                }

                let id = match point.id {
                    Some(id) => match id.point_id_options {
                        Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(n)) => {
                            n.to_string()
                        }
                        Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(s)) => s,
                        None => "unknown".to_string(),
                    },
                    None => "unknown".to_string(),
                };

                Some(MemoryResult {
                    id,
                    content,
                    metadata,
                })
            })
            .collect();

        Ok(results)
    }

    pub async fn delete_point(&self, collection_name: &str, id: &str) -> AIResult<()> {
        let point_id: PointId = if let Ok(n) = id.parse::<u64>() {
            n.into()
        } else {
            id.to_string().into()
        };

        let request = DeletePointsBuilder::new(collection_name)
            .points(vec![point_id])
            .build();

        self.client
            .delete_points(request)
            .await
            .map_err(|e| AIError::ConnectorError(format!("Delete failed: {}", e)))?;

        Ok(())
    }

    pub async fn update_point(
        &self,
        collection_name: &str,
        id: &str,
        content: &str,
        metadata: Option<HashMap<String, String>>,
    ) -> AIResult<()> {
        let point_id: PointId = if let Ok(n) = id.parse::<u64>() {
            n.into()
        } else {
            id.to_string().into()
        };

        // Re-embed new content (Nomic v1.5 prefix)
        let content_for_embedding = format!("search_document: {}", content);
        let embedding = self.embedder.embed(&content_for_embedding).await?;

        let mut payload = Payload::new();
        payload.insert("content", json!(content));

        if let Some(meta) = metadata {
            for (k, v) in meta {
                payload.insert(k, json!(v));
            }
        }

        let point = PointStruct::new(point_id, embedding, payload);

        self.client
            .upsert_points(UpsertPoints {
                collection_name: collection_name.to_string(),
                points: vec![point],
                ..Default::default()
            })
            .await
            .map_err(|e| AIError::ConnectorError(format!("Update failed: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_core_directives_query_builder_quest() {
        // Since testing against a live Qdrant instance is flaky in isolated CI
        // environments, we verify that the function is structurally sound
        // and handles connection errors gracefully without unwrapping/panicking.
        let kb = KnowledgeBase::new("http://localhost:12345", "http://localhost:11434").unwrap();
        
        let result = kb.get_core_directives("test_collection").await;
        
        // We expect it to fail gracefully with an anyhow error because the dummy port is closed,
        // rather than panicking.
        assert!(result.is_err(), "Expected graceful failure when Qdrant is offline.");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to scroll core directives"), "Error message should contain expected context.");
    }
}

