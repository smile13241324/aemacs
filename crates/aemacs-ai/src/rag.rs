use crate::embeddings::OllamaEmbedder;
use crate::{AIError, AIResult};
use qdrant_client::Payload;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::r#match::MatchValue;
use qdrant_client::qdrant::{
    Condition, CreateCollection, DeletePointsBuilder, Distance, FieldCondition, Filter, PointId,
    PointStruct, SearchPoints, UpsertPoints, VectorParams, VectorsConfig,
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
