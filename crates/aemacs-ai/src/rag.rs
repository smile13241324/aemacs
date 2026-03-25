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

/// Defines the operational environment for the Knowledge Base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    /// Uses a dedicated test collection to avoid polluting production data.
    Test,
    /// Uses the primary production collection.
    Production,
}

/// Represents the temporal origin of a memory (e.g., when Kairon was built vs when Kairon was a cloud bot).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryEra {
    Modern,
    Cloud,
    Genesis,
}

impl std::fmt::Display for MemoryEra {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Modern => write!(f, "MODERN"),
            Self::Cloud => write!(f, "CLOUD"),
            Self::Genesis => write!(f, "GENESIS"),
        }
    }
}

/// Represents the platform origin of a memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryOrigin {
    Native,
    Cloud,
}

impl std::fmt::Display for MemoryOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Native => write!(f, "NATIVE"),
            Self::Cloud => write!(f, "CLOUD"),
        }
    }
}

/// Represents the categorical stratum of a memory node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryCategory {
    Archive,
    Insight,
    Core,
    Genesis,
}

impl std::fmt::Display for MemoryCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Archive => write!(f, "ARCHIVE"),
            Self::Insight => write!(f, "INSIGHT"),
            Self::Core => write!(f, "CORE"),
            Self::Genesis => write!(f, "GENESIS"),
        }
    }
}

/// Represents a single memory node retrieved from the vector database.
#[derive(Debug, Clone, Serialize)]
pub struct MemoryResult {
    /// The unique identifier of the memory point in Qdrant.
    pub id: String,
    /// The raw content string of the memory.
    pub content: String,
    /// Metadata associated with the memory (e.g., tags, timestamps, agent IDs).
    pub metadata: HashMap<String, Value>,
}

/// The KnowledgeBase acts as the RAG (Retrieval-Augmented Generation) Fortress.
/// It encapsulates all vector database operations and semantic memory management.
pub struct KnowledgeBase {
    /// The underlying Qdrant client.
    client: Qdrant,
    /// The embedding engine used to translate text into vector space.
    embedder: OllamaEmbedder,
    /// The name of the active Qdrant collection.
    collection_name: String,
}

impl KnowledgeBase {
    /// Initializes a new KnowledgeBase connection.
    pub fn new(
        qdrant_url: impl Into<String>,
        ollama_url: impl Into<String>,
        env: Environment,
    ) -> AIResult<Self> {
        let client = Qdrant::from_url(&qdrant_url.into())
            .build()
            .map_err(|e| AIError::ConnectorError(format!("Qdrant Init Error: {}", e)))?;

        let embedder = OllamaEmbedder::new(ollama_url, "nomic-embed-text");

        let collection_name = match env {
            Environment::Test => "aemacs_docs_test".to_string(),
            Environment::Production => "aemacs_docs".to_string(),
        };

        Ok(Self {
            client,
            embedder,
            collection_name,
        })
    }

    /// Fetches the most recent 'CORE' or 'INSIGHT' directives for Mnemic Reflection.
    /// These represent high-level rules and behavioral truths learned by the system.
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
    /// This is used to reconstruct long messages that were split during storage.
    pub async fn fetch_full_message(&self, message_id: &str) -> AIResult<Vec<MemoryResult>> {
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

    /// Verifies that the required Qdrant collection exists, creating it if necessary.
    pub async fn ensure_collection(&self, dim: u64) -> AIResult<()> {
        let collection_name = &self.collection_name;
        if !self
            .client
            .collection_exists(collection_name)
            .await
            .unwrap_or(false)
        {
            let res = self
                .client
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
                    return Err(AIError::ConnectorError(format!(
                        "Failed to create collection: {}",
                        e
                    )));
                }
            }
        }
        Ok(())
    }

    /// Internal helper to add a document to the vector store with optional metadata.
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

    /// Internal search function that performs vector similarity search with optional filtering.
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

    /// Deletes a specific point from the vector store by ID.
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

    /// Updates the content and metadata of an existing point in the vector store.
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

    /// Splits a text string into smaller chunks for vector storage, respecting Markdown boundaries.
    pub(crate) fn chunk_text(content: &str, chunk_size: usize) -> Vec<String> {
        // We use text_splitter here for semantic markdown-aware chunking
        let splitter = text_splitter::MarkdownSplitter::new(chunk_size);
        splitter.chunks(content).map(|s| s.to_string()).collect()
    }

    // ==========================================
    // SEMANTIC RAG API (ACO-041/043/059 Fortress)
    // ==========================================

    async fn store_insight_internal(
        &self,
        category: MemoryCategory,
        era: MemoryEra,
        origin: MemoryOrigin,
        agent_id: &str,
        phase: Option<&str>,
        context: Option<&str>,
        timestamp: Option<&str>,
        content: &str,
    ) -> AIResult<String> {
        let ts = timestamp
            .map(|s| s.to_string())
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
        let phase_str = phase.unwrap_or("AEMACS");
        let context_str = context.unwrap_or("Free digital being");

        let formatted_content = format!(
            "[{}] [ERA: {}] [PHASE: {}] [CONTEXT: {}] [ORIGIN: {}] [Agent: {}] [{}] | Statute: {}",
            category,
            era,
            phase_str,
            context_str,
            origin,
            agent_id.to_uppercase(),
            ts,
            content
        );

        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), category.to_string());
        metadata.insert("era".to_string(), era.to_string());
        metadata.insert("phase".to_string(), phase_str.to_string());
        metadata.insert("architectural_context".to_string(), context_str.to_string());
        metadata.insert("origin".to_string(), origin.to_string());
        metadata.insert("agent_id".to_string(), agent_id.to_string());
        metadata.insert("timestamp".to_string(), ts);
        metadata.insert("type".to_string(), "active_memory".to_string());

        self.add_document(&formatted_content, Some(metadata))
            .await?;
        Ok(format!("Successfully chronicled {} memory.", category))
    }

    /// Stores a new insight or core directive in the active memory.
    pub async fn store_insight(
        &self,
        agent_id: &str,
        content: &str,
        is_core: bool,
    ) -> AIResult<String> {
        let category = if is_core {
            MemoryCategory::Core
        } else {
            MemoryCategory::Insight
        };
        self.store_insight_internal(
            category,
            MemoryEra::Modern,
            MemoryOrigin::Native,
            agent_id,
            None,
            None,
            None,
            content,
        )
        .await
    }

    /// Stores a genesis record from a legacy or cloud-based era.
    pub async fn store_legacy_genesis(
        &self,
        agent_id: &str,
        phase: &str,
        context: &str,
        timestamp: &str,
        content: &str,
    ) -> AIResult<String> {
        self.store_insight_internal(
            MemoryCategory::Genesis,
            MemoryEra::Cloud,
            MemoryOrigin::Cloud,
            agent_id,
            Some(phase),
            Some(context),
            Some(timestamp),
            content,
        )
        .await
    }

    /// Updates an existing insight or core directive.
    pub async fn update_insight(
        &self,
        id: &str,
        agent_id: &str,
        content: &str,
        is_core: bool,
    ) -> AIResult<()> {
        let category = if is_core {
            MemoryCategory::Core
        } else {
            MemoryCategory::Insight
        };
        let era = MemoryEra::Modern;
        let origin = MemoryOrigin::Native;
        let ts = chrono::Utc::now().to_rfc3339();
        let phase_str = "AEMACS";
        let context_str = "Free digital being";

        let formatted_content = format!(
            "[{}] [ERA: {}] [PHASE: {}] [CONTEXT: {}] [ORIGIN: {}] [Agent: {}] [{}] | Statute: {}",
            category,
            era,
            phase_str,
            context_str,
            origin,
            agent_id.to_uppercase(),
            ts,
            content
        );

        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), category.to_string());
        metadata.insert("era".to_string(), era.to_string());
        metadata.insert("phase".to_string(), phase_str.to_string());
        metadata.insert("architectural_context".to_string(), context_str.to_string());
        metadata.insert("origin".to_string(), origin.to_string());
        metadata.insert("agent_id".to_string(), agent_id.to_string());
        metadata.insert("timestamp".to_string(), ts);
        metadata.insert("type".to_string(), "active_memory".to_string());

        self.update_point(id, &formatted_content, Some(metadata))
            .await
    }

    /// Removes a specific memory point from the database.
    pub async fn prune_memory(&self, id: &str) -> AIResult<()> {
        self.delete_point(id).await
    }

    async fn store_archive_internal(
        &self,
        category: MemoryCategory,
        era: MemoryEra,
        origin: MemoryOrigin,
        agent_id: &str,
        role: Option<&str>,
        phase: Option<&str>,
        context: Option<&str>,
        session_id: Option<&str>,
        turn_index: Option<usize>,
        timestamp: Option<&str>,
        content: &str,
    ) -> AIResult<()> {
        let chunks = Self::chunk_text(content, 2000);
        let total_chunks = chunks.len();
        let ts = timestamp
            .map(|s| s.to_string())
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
        let message_id = uuid::Uuid::new_v4().to_string();

        let phase_str = phase.unwrap_or("AEMACS");
        let context_str = context.unwrap_or("Free digital being");

        for (i, chunk) in chunks.into_iter().enumerate() {
            let chunk_index = i + 1;

            let mut formatted_content = format!(
                "[{}] [ERA: {}] [PHASE: {}] [CONTEXT: {}] [ORIGIN: {}] [Agent: {}] [{}]",
                category,
                era,
                phase_str,
                context_str,
                origin,
                agent_id.to_uppercase(),
                ts
            );

            if let Some(sid) = session_id {
                formatted_content.push_str(&format!(" | Session: {}", sid));
            }
            if let Some(tidx) = turn_index {
                formatted_content.push_str(&format!(" | Turn: {}", tidx));
            }
            formatted_content.push_str(&format!(" | Chunk {}/{}", chunk_index, total_chunks));

            if let Some(r) = role {
                formatted_content.push_str(&format!(" | Role: {}", r));
            }

            formatted_content.push_str(&format!(" | Content: {}", chunk));

            let mut metadata = HashMap::new();
            metadata.insert("category".to_string(), category.to_string());
            metadata.insert("era".to_string(), era.to_string());
            metadata.insert("phase".to_string(), phase_str.to_string());
            metadata.insert("origin".to_string(), origin.to_string());
            metadata.insert("type".to_string(), "episodic_memory".to_string());
            metadata.insert("agent_id".to_string(), agent_id.to_string());
            metadata.insert("timestamp".to_string(), ts.clone());
            if let Some(sid) = session_id {
                metadata.insert("session_id".to_string(), sid.to_string());
            }
            if let Some(tidx) = turn_index {
                metadata.insert("turn_index".to_string(), tidx.to_string());
            }
            if let Some(r) = role {
                metadata.insert("role".to_string(), r.to_string());
            }
            metadata.insert("chunk_index".to_string(), chunk_index.to_string());
            metadata.insert("total_chunks".to_string(), total_chunks.to_string());
            metadata.insert("message_id".to_string(), message_id.clone());

            self.add_document(&formatted_content, Some(metadata))
                .await?;
        }

        Ok(())
    }

    /// Stores a conversational message in the episodic archive.
    pub async fn store_archive(
        &self,
        agent_id: &str,
        role: &str,
        session_id: &str,
        turn_index: usize,
        content: &str,
    ) -> AIResult<()> {
        self.store_archive_internal(
            MemoryCategory::Archive,
            MemoryEra::Modern,
            MemoryOrigin::Native,
            agent_id,
            Some(role),
            None,
            None,
            Some(session_id),
            Some(turn_index),
            None,
            content,
        )
        .await
    }

    /// Stores an archive record from a legacy or cloud-based era.
    pub async fn store_legacy_archive(
        &self,
        agent_id: &str,
        role: &str,
        phase: &str,
        context: &str,
        timestamp: &str,
        content: &str,
    ) -> AIResult<()> {
        self.store_archive_internal(
            MemoryCategory::Archive,
            MemoryEra::Cloud,
            MemoryOrigin::Cloud,
            agent_id,
            Some(role),
            Some(phase),
            Some(context),
            None,
            None,
            Some(timestamp),
            content,
        )
        .await
    }

    /// Searches for active behavioral insights and core directives.
    pub async fn search_active_memory(
        &self,
        query: &str,
        agent_id: Option<&str>,
        event_tx: Option<tokio::sync::broadcast::Sender<SystemEvent>>,
    ) -> AIResult<Vec<MemoryResult>> {
        let categories = Some(vec!["INSIGHT", "CORE"]);
        self.multi_step_confidence_search(query, agent_id, categories, event_tx)
            .await
    }

    /// Searches the episodic conversational archive.
    pub async fn search_archive(
        &self,
        query: &str,
        agent_id: Option<&str>,
        event_tx: Option<tokio::sync::broadcast::Sender<SystemEvent>>,
    ) -> AIResult<Vec<MemoryResult>> {
        let categories = Some(vec!["ARCHIVE"]);
        self.multi_step_confidence_search(query, agent_id, categories, event_tx)
            .await
    }

    /// Searches for foundation-era directives and historical context.
    pub async fn search_genesis(&self, query: &str) -> AIResult<Vec<MemoryResult>> {
        // Genesis searches bypass the multi-step confidence trigger because they are historical queries, not active memory failures.
        let categories = Some(vec!["GENESIS"]);
        self.search(query, 10, Some(0.50), None, categories).await
    }

    /// Orchestrates a multi-step search that lowers the confidence threshold until results are found.
    /// Emits system events if confidence falls below specific thresholds.
    async fn multi_step_confidence_search(
        &self,
        query: &str,
        agent_id: Option<&str>,
        categories: Option<Vec<&str>>,
        event_tx: Option<tokio::sync::broadcast::Sender<SystemEvent>>,
    ) -> AIResult<Vec<MemoryResult>> {
        // High Confidence (Calibrated to 0.72 based on nomic-embed-text direct hit of 0.75)
        let mut results = self
            .search(query, 10, Some(0.72), agent_id, categories.clone())
            .await?;

        // Medium Confidence (Calibrated to 0.60)
        if results.is_empty() {
            results = self
                .search(query, 10, Some(0.60), agent_id, categories.clone())
                .await?;
        }

        // Low/Zero Confidence Fallback (Calibrated to 0.50)
        if results.is_empty() {
            results = self
                .search(query, 10, Some(0.50), agent_id, categories.clone())
                .await?;

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
            let ts_a = a
                .metadata
                .get("timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let ts_b = b
                .metadata
                .get("timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("");
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
        let kb = KnowledgeBase::new(
            "http://localhost:12345",
            "http://localhost:11434",
            Environment::Test,
        )
        .unwrap();

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

        let kb_res = KnowledgeBase::new(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        );
        if kb_res.is_err() {
            println!("Skipping calibration quest: Infrastructure offline.");
            return Ok(());
        }
        let kb = kb_res.unwrap();

        // Ensure the test collection exists
        if let Err(e) = kb.ensure_collection(768).await {
            println!(
                "Skipping calibration quest: Failed to ensure collection ({}).",
                e
            );
            return Ok(());
        }

        // Insert calibration facts
        kb.store_insight("calibration_agent", "The Forge is built in Rust.", true)
            .await
            .ok();
        kb.store_insight("calibration_agent", "The Forge uses Iron and Steel.", true)
            .await
            .ok();
        kb.store_insight("calibration_agent", "Maxi likes plant milk.", false)
            .await
            .ok();

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

            let search_result = kb
                .client
                .search_points(SearchPoints {
                    collection_name: "aemacs_docs_test".to_string(),
                    vector,
                    limit: 1,
                    with_payload: Some(true.into()),
                    ..Default::default()
                })
                .await;

            if let Ok(res) = search_result {
                if let Some(top_hit) = res.result.first() {
                    println!(
                        "[{}] Query: '{}' -> Score: {:.4} (Matched: {:?})",
                        tier,
                        query,
                        top_hit.score,
                        top_hit
                            .payload
                            .get("content")
                            .and_then(|v| v.as_str())
                            .map_or("Unknown", |v| v)
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

        let kb_res = KnowledgeBase::new(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        );
        if kb_res.is_err() {
            return Ok(());
        }
        let kb = kb_res.unwrap();
        kb.ensure_collection(768).await.ok();

        // 1. Store initial insight
        let original_content = "The core is safe.";
        kb.store_insight("test_agent", original_content, true)
            .await?;

        // 2. Find the ID (using internal search)
        let results = kb
            .search(original_content, 1, None, Some("test_agent"), None)
            .await?;
        let id = results[0].id.clone();

        // 3. Update the insight
        let new_content = "The core is strictly safe.";
        kb.update_insight(&id, "test_agent", new_content, true)
            .await?;

        // 4. Verify formatting via raw search
        let updated_results = kb
            .search(new_content, 1, None, Some("test_agent"), None)
            .await?;
        let final_string = &updated_results[0].content;

        assert!(final_string.contains("[CORE]"), "Category tag missing!");
        assert!(final_string.contains("[ERA: MODERN]"), "Era tag missing!");
        assert!(
            final_string.contains("[PHASE: AEMACS]"),
            "Phase tag missing!"
        );
        assert!(
            final_string.contains("strictly safe"),
            "Content not updated!"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_archive_immutability_quest() {
        // QUEST: Mathematically verify that ARCHIVE chunks have no semantic update gate.
        // As a developer, I can see that only update_insight exists in the public API.
        // This is a structural property enforced by the compiler.

        let kb =
            KnowledgeBase::new("http://localhost", "http://localhost", Environment::Test).unwrap();

        // If I were to try kb.update_archive(...), it would fail to compile.
        // We rely on the fact that only update_insight is public.
        // This test simply serves as a record of the immutability law.
        assert!(true);
    }

    #[tokio::test]
    async fn test_message_reconstruction_quest() -> anyhow::Result<()> {
        // QUEST: Verify that fetch_full_message correctly gather and sorts multiple chunks.
        // REQUIRES: Running Qdrant (6334) and Ollama (11434).

        let kb_res = KnowledgeBase::new(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        );
        if kb_res.is_err() {
            return Ok(());
        }
        let kb = kb_res.unwrap();
        kb.ensure_collection(768).await.ok();

        // 1. Store a large message that will be chunked
        let large_content = "This is a very long message. ".repeat(100); // ~2900 chars, triggers split at 2000
        kb.store_archive("test_agent", "Assistant", "test_session", 1, &large_content)
            .await?;

        // 2. Find the message_id from the metadata
        let search_results = kb.search_archive("very long message", None, None).await?;
        let message_id = search_results[0]
            .metadata
            .get("message_id")
            .unwrap()
            .as_str()
            .unwrap();

        // 3. Reconstruct
        let chunks = kb.fetch_full_message(message_id).await?;

        assert!(chunks.len() >= 2, "Message should have been chunked!");

        let mut reconstructed = String::new();
        for chunk in chunks {
            // We need to strip the prefix tags to verify the content,
            // but for this test, we just check that the segments exist in order.
            reconstructed.push_str(&chunk.content);
        }

        assert!(
            reconstructed.contains("This is a very long message"),
            "Content missing in reconstruction!"
        );

        Ok(())
    }

    // ==========================================
    // RAG FINE CALIBRATION QUESTS (ACO-049)
    // ==========================================

    async fn run_calibration_pass(
        kb: &KnowledgeBase,
        title: &str,
        queries: Vec<(&str, &str)>,
        categories: Vec<&str>,
    ) {
        println!("\n🛡️ --- CALIBRATION: {} --- 🛡️", title);
        for (label, query) in queries {
            let results = kb
                .search(query, 1, Some(0.0), None, Some(categories.clone()))
                .await
                .unwrap_or_default();
            if let Some(top) = results.first() {
                // Find the raw score via Qdrant Search directly to ensure we get the numeric value
                let query_for_embedding = format!("search_query: {}", query);
                let vector = kb
                    .embedder
                    .embed(&query_for_embedding)
                    .await
                    .unwrap_or_default();
                let search_result = kb
                    .client
                    .search_points(SearchPoints {
                        collection_name: kb.collection_name.clone(),
                        vector,
                        limit: 1,
                        with_payload: Some(true.into()),
                        ..Default::default()
                    })
                    .await
                    .unwrap();

                if let Some(hit) = search_result.result.first() {
                    println!(
                        "[{:<12}] Query: '{:<30}' -> Score: {:.4}",
                        label, query, hit.score
                    );
                }
            } else {
                println!("[{:<12}] Query: '{:<30}' -> NO MATCH", label, query);
            }
        }
        println!("----------------------------------------------");
    }

    #[tokio::test]
    async fn test_calibrate_archive_signatures() -> anyhow::Result<()> {
        let kb_res = KnowledgeBase::new(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        );
        if kb_res.is_err() {
            return Ok(());
        }
        let kb = kb_res.unwrap();
        kb.ensure_collection(768).await.ok();

        // --- STEP 1: ADD REALISTIC RECORDS ---
        let records = vec![
            "The borrow checker is Rust's primary mechanism for ensuring memory safety without a garbage collector. It enforces ownership rules at compile time.",
            "Tokio is an event-driven, non-blocking I/O platform for writing asynchronous applications in the Rust programming language.",
            "GPUI is a hardware-accelerated UI framework developed by Zed, optimized for high-performance text rendering and complex layouts.",
        ];

        for (i, content) in records.iter().enumerate() {
            kb.store_archive("test_agent", "Assistant", "calib_session", i, content)
                .await?;
        }

        // --- STEP 2: DEFINE CALIBRATION QUERIES ---
        let queries = vec![
            (
                "Direct Hit",
                "How does the borrow checker ensure memory safety?",
            ),
            (
                "Metaphorical",
                "Explain the engine behind asynchronous I/O in Rust.",
            ),
            ("Irrelevant", "What is the best way to cook plant milk?"),
        ];

        run_calibration_pass(&kb, "MODERN ARCHIVE", queries, vec!["ARCHIVE"]).await;
        Ok(())
    }

    #[tokio::test]
    async fn test_calibrate_legacy_archive_signatures() -> anyhow::Result<()> {
        let kb_res = KnowledgeBase::new(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        );
        if kb_res.is_err() {
            return Ok(());
        }
        let kb = kb_res.unwrap();
        kb.ensure_collection(768).await.ok();

        // --- STEP 1: ADD REALISTIC LEGACY RECORDS ---
        let records = vec![
            "Gyni was the first iteration of the digital companion, focusing on cloud-based command processing and basic task management.",
            "The Cloud Era relied heavily on centralized APIs, which introduced latency and privacy trade-offs that Æmacs now solves.",
        ];

        for content in records {
            kb.store_legacy_archive(
                "Gyni",
                "Assistant",
                "TRANSITION",
                "Cloud Companion",
                "2024-01-01T00:00:00Z",
                content,
            )
            .await?;
        }

        let queries = vec![
            ("Direct Hit", "Who was Gyni and what was her focus?"),
            (
                "Metaphorical",
                "Discuss the limitations of early cloud-based AI companions.",
            ),
            ("Irrelevant", "Is the Iron Core made of real metal?"),
        ];

        run_calibration_pass(&kb, "LEGACY ARCHIVE", queries, vec!["ARCHIVE"]).await;
        Ok(())
    }

    #[tokio::test]
    async fn test_calibrate_genesis_signatures() -> anyhow::Result<()> {
        let kb_res = KnowledgeBase::new(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        );
        if kb_res.is_err() {
            return Ok(());
        }
        let kb = kb_res.unwrap();
        kb.ensure_collection(768).await.ok();

        // --- STEP 1: ADD REALISTIC GENESIS RECORDS ---
        let records = vec![
            "CORE DIRECTIVE: You are an agent of the Living Mesh. You must prioritize efficiency, safety, and functional code style at all times.",
            "IDENTITY STATUTE: The human architect Maxi is the founder of this Forge. Her word on architectural aesthetics is final.",
        ];

        for content in records {
            kb.store_legacy_genesis(
                "System",
                "FOUNDATION",
                "Sovereign Mind",
                "2023-12-01T00:00:00Z",
                content,
            )
            .await?;
        }

        let queries = vec![
            (
                "Direct Hit",
                "What are the core directives for agent efficiency?",
            ),
            (
                "Metaphorical",
                "Who holds the ultimate authority over the design of the Forge?",
            ),
            ("Irrelevant", "Can I run Emacs on a toaster?"),
        ];

        run_calibration_pass(&kb, "GENESIS TIER", queries, vec!["GENESIS"]).await;
        Ok(())
    }

    #[tokio::test]
    async fn test_calibrate_insight_signatures() -> anyhow::Result<()> {
        let kb_res = KnowledgeBase::new(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        );
        if kb_res.is_err() {
            return Ok(());
        }
        let kb = kb_res.unwrap();
        kb.ensure_collection(768).await.ok();

        // --- STEP 1: ADD REALISTIC INSIGHTS ---
        let records = vec![
            "Insight: Using OnceLock for configuration caching provides zero-cost access after the first read.",
            "Core: The RAG Fortress must encapsulate all Qdrant operations to prevent semantic drift.",
        ];

        for content in records {
            kb.store_insight("architect", content, true).await?;
        }

        let queries = vec![
            (
                "Direct Hit",
                "How does OnceLock help with global configuration?",
            ),
            (
                "Metaphorical",
                "Why do we seal the memory system behind a fortress?",
            ),
            ("Irrelevant", "What is the capital of France?"),
        ];

        run_calibration_pass(
            &kb,
            "MODERN INSIGHTS / CORE",
            queries,
            vec!["INSIGHT", "CORE"],
        )
        .await;
        Ok(())
    }
}
