use crate::embeddings::OllamaEmbedder;
use crate::{AIError, AIResult};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    vectors_config::Config, CreateCollection, Distance, PointStruct, SearchPoints, UpsertPoints, VectorParams,
    VectorsConfig,
};
use qdrant_client::Payload;
use uuid::Uuid;
use std::collections::HashMap;

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
        if !self.client.collection_exists(collection_name).await.map_err(|e| AIError::ConnectorError(e.to_string()))? {
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
                .map_err(|e| AIError::ConnectorError(format!("Failed to create collection: {}", e)))?;
        }
        Ok(())
    }

    pub async fn add_document(
        &self,
        collection_name: &str,
        content: &str,
        metadata: Option<HashMap<String, String>>,
    ) -> AIResult<()> {
        let embedding = self.embedder.embed(content).await?;
        let id = Uuid::new_v4();
        
        let mut payload = Payload::new();
        payload.insert("content", content.to_string());
        if let Some(meta) = metadata {
            for (k, v) in meta {
                payload.insert(k, v);
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
    ) -> AIResult<Vec<String>> {
        let vector = self.embedder.embed(query).await?;

        let search_result = self.client
            .search_points(SearchPoints {
                collection_name: collection_name.to_string(),
                vector,
                limit,
                with_payload: Some(true.into()),
                ..Default::default()
            })
            .await
            .map_err(|e| AIError::ConnectorError(format!("Search failed: {}", e)))?;

        let results = search_result.result.into_iter().filter_map(|point| {
            point.payload.get("content").and_then(|v| v.as_str().map(|s| s.to_string()))
        }).collect();

        Ok(results)
    }
}
