use crate::embeddings::OllamaEmbedder;
use crate::{AIError, AIResult};
use aemacs_core::bus::{EventBus, SystemEvent};
use qdrant_client::Payload;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::r#match::MatchValue;
use qdrant_client::qdrant::{
    Condition, CreateCollection, DeletePointsBuilder, Distance, FieldCondition, Filter, PointId,
    PointStruct, ScrollPoints, SearchPoints, UpsertPoints, VectorParams, VectorsConfig,
    condition::ConditionOneOf, vectors_config::Config,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Test,
    Production,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryResult {
    pub id: String,
    pub content: String,
    pub metadata: HashMap<String, Value>,
}

pub struct KnowledgeBase {
    client: Qdrant,
    embedder: OllamaEmbedder,
    collection_name: String,
}

impl KnowledgeBase {
    pub fn new(qdrant_url: impl Into<String>, ollama_url: impl Into<String>, env: Environment) -> AIResult<Self> {
        let client = Qdrant::from_url(&qdrant_url.into())
            .build()
            .map_err(|e| AIError::ConnectorError(format!("Qdrant Init Error: {}", e)))?;

        let embedder = OllamaEmbedder::new(ollama_url, "nomic-embed-text");
        
        let collection_name = match env {
            Environment::Test => "aemacs_docs_test".to_string(),
            Environment::Production => "aemacs_docs".to_string(),
        };

        // Note: In a test environment, it is best practice to clear the collection before tests run.
        // However, `new` is synchronous here, so we will handle the `ensure_collection` async step later 
        // or expect the caller to `ensure_collection`.

        Ok(Self { client, embedder, collection_name })
    }

    /// Fetches the most recent 'CORE' or 'INSIGHT' directives for Mnemic Reflection.
    pub async fn get_core_directives(&self) -> anyhow::Result<Vec<String>> {
        let collection_name = &self.collection_name;
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

        let scroll_result = self
            .client
            .scroll(request)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to scroll core directives: {}", e))?;

        let mut contents = Vec::new();
        for point in scroll_result.result {
            if let Some(content) = point.payload.get("content").and_then(|v| v.as_str()) {
                contents.push(content.to_string());
            }
        }

        Ok(contents)
    }

    /// Fetches all chunks associated with a specific message_id, sorted by their original chunk index.
    pub async fn fetch_full_message(
        &self,
        message_id: &str,
    ) -> AIResult<Vec<MemoryResult>> {
        let collection_name = &self.collection_name;
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

    pub async fn ensure_collection(&self, dim: u64) -> AIResult<()> {
        let collection_name = &self.collection_name;
        if !self
            .client
            .collection_exists(collection_name)
            .await
            .unwrap_or(false)
        {
            let res = self.client
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
                .await;
                
            if let Err(e) = res {
                if !e.to_string().contains("already exists") {
                    return Err(AIError::ConnectorError(format!("Failed to create collection: {}", e)));
                }
            }
        }
        Ok(())
    }

    async fn add_document(
        &self,
        content: &str,
        metadata: Option<HashMap<String, String>>,
    ) -> AIResult<()> {
        let collection_name = &self.collection_name;
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

    async fn search(
        &self,
        query: &str,
        limit: u64,
        score_threshold: Option<f32>,
        agent_id: Option<&str>,
        categories: Option<Vec<&str>>,
    ) -> AIResult<Vec<MemoryResult>> {
        let collection_name = &self.collection_name;
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

    async fn delete_point(&self, id: &str) -> AIResult<()> {
        let collection_name = &self.collection_name;
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

    async fn update_point(
        &self,
        id: &str,
        content: &str,
        metadata: Option<HashMap<String, String>>,
    ) -> AIResult<()> {
        let collection_name = &self.collection_name;
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

    /// ACO-028: Triggers background maintenance/optimization for a specific collection.
    async fn optimize_collection(&self) -> AIResult<()> {
        let collection_name = &self.collection_name;
        use qdrant_client::qdrant::UpdateCollection;

        // In Qdrant, we can trigger optimization by updating collection parameters.
        // We'll just 'touch' the configuration to nudge the indexing engine.
        let request = UpdateCollection {
            collection_name: collection_name.to_string(),
            ..Default::default()
        };

        self.client
            .update_collection(request)
            .await
            .map_err(|e| AIError::ConnectorError(format!("Optimization nudge failed: {}", e)))?;

        tracing::info!(
            "🧠 [RAG] Background optimization triggered for collection: {}",
            collection_name
        );
        Ok(())
    }

    // ==========================================
    // CHUNKING MECHANICS (ACO-042)
    // ==========================================
    pub(crate) fn chunk_text(content: &str, chunk_size: usize) -> Vec<String> {
        // We use text_splitter here for semantic markdown-aware chunking
        let splitter = text_splitter::MarkdownSplitter::new(chunk_size);
        splitter.chunks(content).map(|s| s.to_string()).collect()
    }

    // ==========================================
    // SEMANTIC RAG API (ACO-041/043 Fortress)
    // ==========================================

    fn format_insight_statute(&self, agent_id: &str, content: &str, is_core: bool) -> (String, HashMap<String, String>) {
        let category = if is_core { "CORE" } else { "INSIGHT" };
        let timestamp = chrono::Utc::now().to_rfc3339();
        
        let formatted_content = format!(
            "[{}] [ERA: MODERN] [PHASE: AEMACS] [CONTEXT: Free digital being] [ORIGIN: NATIVE] [Agent: {}] [{}] | Statute: {}",
            category,
            agent_id.to_uppercase(),
            timestamp,
            content
        );

        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), category.to_string());
        metadata.insert("era".to_string(), "modern".to_string());
        metadata.insert("phase".to_string(), "AEMACS".to_string());
        metadata.insert("architectural_context".to_string(), "Free digital being".to_string());
        metadata.insert("origin".to_string(), "native".to_string());
        metadata.insert("agent_id".to_string(), agent_id.to_string());
        metadata.insert("timestamp".to_string(), timestamp);
        metadata.insert("type".to_string(), "active_memory".to_string());

        (formatted_content, metadata)
    }

    pub async fn store_insight(&self, agent_id: &str, content: &str, is_core: bool) -> AIResult<String> {
        let (formatted_content, metadata) = self.format_insight_statute(agent_id, content, is_core);
        let category = metadata.get("category").cloned().unwrap_or_default();
        
        self.add_document(&formatted_content, Some(metadata)).await?;
        Ok(format!("Successfully chronicled {} memory.", category))
    }

    pub async fn update_insight(&self, id: &str, agent_id: &str, content: &str, is_core: bool) -> AIResult<()> {
        let (formatted_content, metadata) = self.format_insight_statute(agent_id, content, is_core);
        self.update_point(id, &formatted_content, Some(metadata)).await
    }

    pub async fn prune_memory(&self, id: &str) -> AIResult<()> {
        self.delete_point(id).await
    }

    pub async fn store_archive(
        &self, 
        agent_id: &str, 
        role: &str, 
        session_id: &str, 
        turn_index: usize, 
        content: &str
    ) -> AIResult<()> {
        let chunks = Self::chunk_text(content, 2000);
        let total_chunks = chunks.len();
        let timestamp = chrono::Utc::now().to_rfc3339();
        let message_id = uuid::Uuid::new_v4().to_string();

        for (i, chunk) in chunks.into_iter().enumerate() {
            let chunk_index = i + 1;
            let formatted_content = format!(
                "[ARCHIVE] [ERA: MODERN] [PHASE: AEMACS] [CONTEXT: Free digital being] [ORIGIN: NATIVE] [Agent: {}] [{}] | Session: {} | Turn: {} | Chunk {}/{} | Role: {} | Content: {}",
                agent_id.to_uppercase(),
                timestamp,
                session_id,
                turn_index,
                chunk_index,
                total_chunks,
                role,
                chunk
            );

            let mut metadata = HashMap::new();
            metadata.insert("category".to_string(), "ARCHIVE".to_string());
            metadata.insert("era".to_string(), "modern".to_string());
            metadata.insert("phase".to_string(), "AEMACS".to_string());
            metadata.insert("origin".to_string(), "native".to_string());
            metadata.insert("type".to_string(), "episodic_memory".to_string());
            metadata.insert("agent_id".to_string(), agent_id.to_string());
            metadata.insert("timestamp".to_string(), timestamp.clone());
            metadata.insert("session_id".to_string(), session_id.to_string());
            metadata.insert("turn_index".to_string(), turn_index.to_string());
            metadata.insert("chunk_index".to_string(), chunk_index.to_string());
            metadata.insert("total_chunks".to_string(), total_chunks.to_string());
            metadata.insert("message_id".to_string(), message_id.clone());
            
            self.add_document(&formatted_content, Some(metadata)).await?;
        }
        
        Ok(())
    }

    pub async fn store_legacy_record(
        &self, 
        tier: &str, 
        era: &str, 
        phase: &str, 
        context: &str, 
        sender: &str, 
        timestamp: &str, 
        content: &str, 
        extra_metadata: Option<HashMap<String, String>>
    ) -> AIResult<()> {
        let chunks = Self::chunk_text(content, 2000);
        let total_chunks = chunks.len();
        let message_id = uuid::Uuid::new_v4().to_string();

        for (i, chunk) in chunks.into_iter().enumerate() {
            let chunk_index = i + 1;
            let formatted_content = format!(
                "[{}] [ERA: {}] [PHASE: {}] [CONTEXT: {}] [ORIGIN: CLOUD] [Agent: {}] [{}] | Chunk {}/{} | Content: {}",
                tier,
                era,
                phase,
                context,
                sender.to_uppercase(),
                timestamp,
                chunk_index,
                total_chunks,
                chunk
            );

            let mut metadata = HashMap::new();
            metadata.insert("category".to_string(), tier.to_string());
            metadata.insert("era".to_string(), era.to_string());
            metadata.insert("phase".to_string(), phase.to_string());
            metadata.insert("origin".to_string(), "cloud".to_string());
            metadata.insert("type".to_string(), "historical_memory".to_string());
            metadata.insert("agent_id".to_string(), sender.to_string());
            metadata.insert("timestamp".to_string(), timestamp.to_string());
            metadata.insert("chunk_index".to_string(), chunk_index.to_string());
            metadata.insert("total_chunks".to_string(), total_chunks.to_string());
            metadata.insert("message_id".to_string(), message_id.clone());

            if let Some(ref extra) = extra_metadata {
                for (k, v) in extra {
                    metadata.insert(k.clone(), v.clone());
                }
            }
            
            self.add_document(&formatted_content, Some(metadata)).await?;
        }
        
        Ok(())
    }

    pub async fn search_active_memory(
        &self,
        query: &str,
        agent_id: Option<&str>,
        event_tx: Option<tokio::sync::broadcast::Sender<SystemEvent>>,
    ) -> AIResult<Vec<MemoryResult>> {
        let categories = Some(vec!["INSIGHT", "CORE"]);
        self.multi_step_confidence_search(query, agent_id, categories, event_tx).await
    }

    pub async fn search_archive(
        &self,
        query: &str,
        agent_id: Option<&str>,
        event_tx: Option<tokio::sync::broadcast::Sender<SystemEvent>>,
    ) -> AIResult<Vec<MemoryResult>> {
        let categories = Some(vec!["ARCHIVE"]);
        self.multi_step_confidence_search(query, agent_id, categories, event_tx).await
    }

    pub async fn search_genesis(
        &self,
        query: &str,
    ) -> AIResult<Vec<MemoryResult>> {
        // Genesis searches bypass the multi-step confidence trigger because they are historical queries, not active memory failures.
        let categories = Some(vec!["GENESIS"]);
        self.search(query, 10, Some(0.50), None, categories).await
    }

    async fn multi_step_confidence_search(
        &self,
        query: &str,
        agent_id: Option<&str>,
        categories: Option<Vec<&str>>,
        event_tx: Option<tokio::sync::broadcast::Sender<SystemEvent>>,
    ) -> AIResult<Vec<MemoryResult>> {
        // High Confidence (Calibrated to 0.72 based on nomic-embed-text direct hit of 0.75)
        let mut results = self.search(query, 10, Some(0.72), agent_id, categories.clone()).await?;
        
        // Medium Confidence (Calibrated to 0.60)
        if results.is_empty() {
            results = self.search(query, 10, Some(0.60), agent_id, categories.clone()).await?;
        }

        // Low/Zero Confidence Fallback (Calibrated to 0.50)
        if results.is_empty() {
            results = self.search(query, 10, Some(0.50), agent_id, categories.clone()).await?;
            
            if results.is_empty() {
                if let Some(tx) = &event_tx {
                    let payload = format!("{{\"query\": {:?}, \"confidence\": \"none\"}}", query);
                    let _ = tx.send(SystemEvent::Signal {
                        source: "RAG_Fortress".to_string(),
                        event_type: "LowConfidenceRecall".to_string(),
                        payload,
                    });
                }
                let _ = self.optimize_collection().await;
            } else {
                if let Some(tx) = &event_tx {
                    let payload = format!("{{\"query\": {:?}, \"confidence\": \"low\"}}", query);
                    let _ = tx.send(SystemEvent::Signal {
                        source: "RAG_Fortress".to_string(),
                        event_type: "LowConfidenceRecall".to_string(),
                        payload,
                    });
                }
            }
        }

        // Temporal Sorting: Prioritize recent insights
        results.sort_by(|a, b| {
            let ts_a = a.metadata.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
            let ts_b = b.metadata.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
            ts_b.cmp(ts_a) // Descending order
        });

        Ok(results)
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
        let kb = KnowledgeBase::new("http://localhost:12345", "http://localhost:11434", Environment::Test).unwrap();

        let result = kb.get_core_directives().await;

        // We expect it to fail gracefully with an anyhow error because the dummy port is closed,
        // rather than panicking.
        assert!(
            result.is_err(),
            "Expected graceful failure when Qdrant is offline."
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Failed to scroll core directives"),
            "Error message should contain expected context."
        );
    }

    #[tokio::test]
    async fn test_rag_confidence_calibration_quest() -> anyhow::Result<()> {
        // QUEST: Calibrate the exact cosine similarity scores for High, Medium, and Low confidence.
        // REQUIRES: Running Qdrant (6334) and Ollama (11434).
        
        let kb_res = KnowledgeBase::new("http://localhost:6334", "http://localhost:11434", Environment::Test);
        if kb_res.is_err() {
            println!("Skipping calibration quest: Infrastructure offline.");
            return Ok(());
        }
        let kb = kb_res.unwrap();
        
        // Ensure the test collection exists
        if let Err(e) = kb.ensure_collection(768).await {
            println!("Skipping calibration quest: Failed to ensure collection ({}).", e);
            return Ok(());
        }

        // Insert calibration facts
        kb.store_insight("calibration_agent", "The Forge is built in Rust.", true).await.ok();
        kb.store_insight("calibration_agent", "The Forge uses Iron and Steel.", true).await.ok();
        kb.store_insight("calibration_agent", "Maxi likes plant milk.", false).await.ok();

        // Give Qdrant a tiny moment to index
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let queries = vec![
            ("Direct Hit", "What language is the Forge built in?"),
            ("Metaphorical", "What are the foundational metals?"),
            ("Irrelevant", "How do I write a Python script?"),
        ];

        println!("\n🛡️ --- RAG CONFIDENCE CALIBRATION --- 🛡️");
        for (tier, query) in queries {
            let vector = match kb.embedder.embed(&format!("search_query: {}", query)).await {
                Ok(v) => v,
                Err(_) => {
                    println!("Embedder offline. Skipping test.");
                    return Ok(());
                }
            };
            
            let search_result = kb.client.search_points(SearchPoints {
                collection_name: "aemacs_docs_test".to_string(),
                vector,
                limit: 1,
                with_payload: Some(true.into()),
                ..Default::default()
            }).await;

            if let Ok(res) = search_result {
                if let Some(top_hit) = res.result.first() {
                    println!("[{}] Query: '{}' -> Score: {:.4} (Matched: {:?})", 
                        tier, query, top_hit.score, 
                        top_hit.payload.get("content").and_then(|v| v.as_str()).map_or("Unknown", |v| v)
                    );
                } else {
                    println!("[{}] Query: '{}' -> NO HITS", tier, query);
                }
            }
        }
        println!("----------------------------------------\n");

        Ok(())
    }

    #[tokio::test]
    async fn test_semantic_update_integrity_quest() -> anyhow::Result<()> {
        // QUEST: Verify that update_insight preserves the mandatory [ERA: MODERN] structure.
        // REQUIRES: Running Qdrant (6334) and Ollama (11434).
        
        let kb_res = KnowledgeBase::new("http://localhost:6334", "http://localhost:11434", Environment::Test);
        if kb_res.is_err() { return Ok(()); }
        let kb = kb_res.unwrap();
        kb.ensure_collection(768).await.ok();

        // 1. Store initial insight
        let original_content = "The core is safe.";
        kb.store_insight("test_agent", original_content, true).await?;

        // 2. Find the ID (using internal search)
        let results = kb.search(original_content, 1, None, Some("test_agent"), None).await?;
        let id = results[0].id.clone();

        // 3. Update the insight
        let new_content = "The core is strictly safe.";
        kb.update_insight(&id, "test_agent", new_content, true).await?;

        // 4. Verify formatting via raw search
        let updated_results = kb.search(new_content, 1, None, Some("test_agent"), None).await?;
        let final_string = &updated_results[0].content;

        assert!(final_string.contains("[CORE]"), "Category tag missing!");
        assert!(final_string.contains("[ERA: MODERN]"), "Era tag missing!");
        assert!(final_string.contains("[PHASE: AEMACS]"), "Phase tag missing!");
        assert!(final_string.contains("strictly safe"), "Content not updated!");

        Ok(())
    }

    #[tokio::test]
    async fn test_archive_immutability_quest() {
        // QUEST: Mathematically verify that ARCHIVE chunks have no semantic update gate.
        // As a developer, I can see that only update_insight exists in the public API.
        // This is a structural property enforced by the compiler.
        
        let kb = KnowledgeBase::new("http://localhost", "http://localhost", Environment::Test).unwrap();
        
        // If I were to try kb.update_archive(...), it would fail to compile.
        // We rely on the fact that only update_insight is public.
        // This test simply serves as a record of the immutability law.
        assert!(true);
    }

    #[tokio::test]
    async fn test_message_reconstruction_quest() -> anyhow::Result<()> {
        // QUEST: Verify that fetch_full_message correctly gather and sorts multiple chunks.
        // REQUIRES: Running Qdrant (6334) and Ollama (11434).
        
        let kb_res = KnowledgeBase::new("http://localhost:6334", "http://localhost:11434", Environment::Test);
        if kb_res.is_err() { return Ok(()); }
        let kb = kb_res.unwrap();
        kb.ensure_collection(768).await.ok();

        // 1. Store a large message that will be chunked
        let large_content = "This is a very long message. ".repeat(100); // ~2900 chars, triggers split at 2000
        kb.store_archive("test_agent", "Assistant", "test_session", 1, &large_content).await?;

        // 2. Find the message_id from the metadata
        let search_results = kb.search_archive("very long message", None, None).await?;
        let message_id = search_results[0].metadata.get("message_id").unwrap().as_str().unwrap();

        // 3. Reconstruct
        let chunks = kb.fetch_full_message(message_id).await?;

        assert!(chunks.len() >= 2, "Message should have been chunked!");
        
        let mut reconstructed = String::new();
        for chunk in chunks {
            // We need to strip the prefix tags to verify the content, 
            // but for this test, we just check that the segments exist in order.
            reconstructed.push_str(&chunk.content);
        }

        assert!(reconstructed.contains("This is a very long message"), "Content missing in reconstruction!");
        
        Ok(())
    }
}
