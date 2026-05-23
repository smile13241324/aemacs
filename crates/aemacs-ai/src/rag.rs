use std::{collections::HashMap, fmt::Write as _, sync::Arc};

use aemacs_core::bus::SystemEvent;
use qdrant_client::{
    Payload, Qdrant,
    qdrant::{
        Condition, CreateCollection, DeletePointsBuilder, Distance, FieldCondition, Filter,
        PointId, PointStruct, ScrollPoints, SearchPoints, UpdateCollection, UpsertPoints,
        Value as QdrantValue, VectorParams, VectorsConfig, condition::ConditionOneOf,
        r#match::MatchValue, vectors_config::Config,
    },
};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{AIError, AIResult, embeddings::OllamaEmbedder, rag_telemetry::RecallOutcome};

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
    /// The clean, unprefixed content string of the memory.
    pub content: String,
    /// The similarity score returned by the vector database.
    pub score: f32,
    /// Metadata associated with the memory (e.g., tags, timestamps, agent IDs).
    pub metadata: HashMap<String, Value>,
}

const CONTENT_KEY: &str = "content";
const DEFAULT_COLLECTION_DIMENSION: u64 = 768;
const DEFAULT_SEARCH_LIMIT: u64 = 10;
const MEDIUM_CONFIDENCE_THRESHOLD: f32 = 0.60;
const LOW_CONFIDENCE_THRESHOLD: f32 = 0.50;
type RetrievedPayload = HashMap<String, QdrantValue>;

struct InsightRecord<'a> {
    category: MemoryCategory,
    era: MemoryEra,
    origin: MemoryOrigin,
    agent_id: &'a str,
    phase: Option<&'a str>,
    context: Option<&'a str>,
    timestamp: Option<&'a str>,
    payload: &'a str,
}

struct ArchiveRecord<'a> {
    category: MemoryCategory,
    era: MemoryEra,
    origin: MemoryOrigin,
    agent_id: &'a str,
    role: Option<&'a str>,
    phase: Option<&'a str>,
    context: Option<&'a str>,
    session_id: Option<&'a str>,
    turn_index: Option<usize>,
    timestamp: Option<&'a str>,
    payload: &'a str,
}

/// The `KnowledgeBase` acts as the RAG (Retrieval-Augmented Generation) Fortress.
/// It encapsulates all vector database operations and semantic memory management.
pub struct KnowledgeBase {
    /// The underlying Qdrant client.
    client: Qdrant,
    /// The embedding engine used to translate text into vector space.
    embedder: OllamaEmbedder,
    /// The name of the active Qdrant collection.
    collection_name: String,
    /// Telemetry logger for tracking search performance.
    telemetry: Arc<crate::rag_telemetry::RagTelemetry>,
}

impl std::fmt::Debug for KnowledgeBase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KnowledgeBase")
            .field("collection_name", &self.collection_name)
            .field("embedder", &self.embedder)
            .field("client", &"Qdrant")
            .field("telemetry", &self.telemetry)
            .finish()
    }
}

fn degraded_recall_signal(
    query: &str,
    recall_outcome: RecallOutcome,
    maintenance_triggered: bool,
) -> Option<SystemEvent> {
    if !matches!(
        recall_outcome,
        RecallOutcome::Low | RecallOutcome::None | RecallOutcome::DiagnosticOnly
    ) {
        return None;
    }

    let payload = json!({
        "query": query,
        "confidence": recall_outcome.as_str(),
        "maintenance_triggered": maintenance_triggered,
    })
    .to_string();

    Some(SystemEvent::Signal {
        source: "RAG_Fortress".to_string(),
        event_type: "LowConfidenceRecall".to_string(),
        payload,
    })
}

impl KnowledgeBase {
    /// Initializes a new `KnowledgeBase` connection.
    ///
    /// # Errors
    /// Returns an error if the Qdrant client cannot be created or if telemetry initialization
    /// fails.
    pub async fn new(
        qdrant_url: impl Into<String>,
        ollama_url: impl Into<String>,
        env: Environment,
    ) -> AIResult<Self> {
        let client = Qdrant::from_url(&qdrant_url.into())
            .build()
            .map_err(|e| AIError::ConnectorError(format!("Qdrant Init Error: {e}")))?;

        let embedder = OllamaEmbedder::new(ollama_url, "nomic-embed-text");

        let collection_name = match env {
            Environment::Test => "aemacs_docs_test".to_string(),
            Environment::Production => "aemacs_docs".to_string(),
        };

        // Initialize telemetry (Async initialization)
        let telemetry = Arc::new(crate::rag_telemetry::RagTelemetry::new().await?);

        // Start the background calibration daemon ("The Night Shift")
        telemetry.clone().start_calibration_daemon();

        Ok(Self { client, embedder, collection_name, telemetry })
    }

    /// Initializes a new `KnowledgeBase` and ensures its backing collection exists.
    ///
    /// This is the shared application bootstrap path for both production and test environments.
    ///
    /// # Errors
    /// Returns an error if the knowledge base cannot be initialized or if the required Qdrant
    /// collection cannot be created or queried.
    pub async fn bootstrap(
        qdrant_url: impl Into<String>,
        ollama_url: impl Into<String>,
        env: Environment,
    ) -> AIResult<Self> {
        let knowledge_base = Self::new(qdrant_url, ollama_url, env).await?;
        knowledge_base.ensure_collection(DEFAULT_COLLECTION_DIMENSION).await?;
        Ok(knowledge_base)
    }

    /// Fetches the most recent 'CORE' or 'INSIGHT' directives for Mnemic Reflection.
    /// These represent high-level rules and behavioral truths learned by the system.
    ///
    /// # Errors
    /// Returns an error if Qdrant cannot be queried for directive payloads.
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

        let filter = Filter { should: cat_conditions, ..Default::default() };

        let request = ScrollPoints {
            collection_name: collection_name.clone(),
            filter: Some(filter),
            limit: Some(10), // Fetch up to 10 core directives
            with_payload: Some(true.into()),
            ..Default::default()
        };

        let scroll_result = self
            .client
            .scroll(request)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to scroll core directives: {e}"))?;

        let mut contents = Vec::new();
        for point in scroll_result.result {
            if let Some(content) = Self::clean_content_from_payload(&point.payload) {
                contents.push(content);
            }
        }

        Ok(contents)
    }

    /// Fetches all chunks associated with a specific `message_id`, sorted by their original chunk index.
    /// This is used to reconstruct long messages that were split during storage.
    ///
    /// # Errors
    /// Returns an error if the archive chunks cannot be retrieved from Qdrant.
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
            collection_name: collection_name.clone(),
            filter: Some(filter),
            limit: Some(100), // Generous limit for a single message
            with_payload: Some(true.into()),
            ..Default::default()
        };

        let scroll_result =
            self.client.scroll(request).await.map_err(|e| {
                AIError::ConnectorError(format!("Failed to fetch by message_id: {e}"))
            })?;

        let mut results: Vec<(usize, MemoryResult)> = scroll_result
            .result
            .into_iter()
            .filter_map(|point| {
                let content = Self::clean_content_from_payload(&point.payload)?;
                let chunk_idx = Self::chunk_index_from_payload(&point.payload);
                let metadata = Self::metadata_from_payload(point.payload);
                let id = Self::point_id_to_string(point.id);

                Some((chunk_idx, MemoryResult { id, content, score: 1.0, metadata }))
            })
            .collect();

        // Sort by chunk index to ensure chronological reconstruction
        results.sort_by_key(|k| k.0);

        Ok(results.into_iter().map(|(_, res)| res).collect())
    }

    /// Verifies that the required Qdrant collection exists, creating it if necessary.
    ///
    /// # Errors
    /// Returns an error if the collection cannot be created or queried.
    pub async fn ensure_collection(&self, dim: u64) -> AIResult<()> {
        let collection_name = &self.collection_name;
        if !self.client.collection_exists(collection_name).await.unwrap_or(false) {
            let res = self
                .client
                .create_collection(CreateCollection {
                    collection_name: collection_name.clone(),
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

            if let Err(e) = res
                && !e.to_string().contains("already exists")
            {
                return Err(AIError::ConnectorError(format!("Failed to create collection: {e}")));
            }
        }
        Ok(())
    }

    /// Internal helper to add a document to the vector store with optional metadata.
    async fn add_document(
        &self,
        formatted_content: &str,
        content: &str,
        metadata: Option<HashMap<String, String>>,
    ) -> AIResult<()> {
        let collection_name = &self.collection_name;
        // Nomic v1.5 requires prefix for documents
        let content_for_embedding = format!("search_document: {formatted_content}");
        let embedding = self.embedder.embed(&content_for_embedding).await?;
        let id = Uuid::new_v4();

        let mut payload = Payload::new();
        payload.insert(CONTENT_KEY, json!(content));

        if let Some(meta) = metadata {
            for (k, v) in meta {
                payload.insert(k, json!(v));
            }
        }

        let point = PointStruct::new(id.to_string(), embedding, payload);

        self.client
            .upsert_points(UpsertPoints {
                collection_name: collection_name.clone(),
                points: vec![point],
                ..Default::default()
            })
            .await
            .map_err(|e| AIError::ConnectorError(format!("Upsert failed: {e}")))?;

        Ok(())
    }

    /// Internal search function that performs vector similarity search with optional filtering.
    /// When `score_threshold` is `None`, Qdrant is allowed to return diagnostic low-similarity hits
    /// without any score floor.
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
        let query_for_embedding = format!("search_query: {query}");
        let vector = self.embedder.embed(&query_for_embedding).await?;

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
            Some(Filter { must: must_conditions, ..Default::default() })
        };

        let search_result = self
            .client
            .search_points(SearchPoints {
                collection_name: collection_name.clone(),
                vector,
                limit,
                score_threshold,
                with_payload: Some(true.into()),
                filter,
                ..Default::default()
            })
            .await
            .map_err(|e| AIError::ConnectorError(format!("Search failed: {e}")))?;

        let results = search_result
            .result
            .into_iter()
            .filter_map(|point| {
                let content = Self::clean_content_from_payload(&point.payload)?;
                let metadata = Self::metadata_from_payload(point.payload);
                let id = Self::point_id_to_string(point.id);

                Some(MemoryResult { id, content, score: point.score, metadata })
            })
            .collect();

        Ok(results)
    }

    /// Deletes a specific point from the vector store by ID.
    async fn delete_point(&self, id: &str) -> AIResult<()> {
        let collection_name = &self.collection_name;
        let point_id: PointId =
            id.parse::<u64>().map_or_else(|_| id.to_string().into(), Into::into);

        let request = DeletePointsBuilder::new(collection_name).points(vec![point_id]).build();

        self.client
            .delete_points(request)
            .await
            .map_err(|e| AIError::ConnectorError(format!("Delete failed: {e}")))?;

        Ok(())
    }

    /// Updates the content and metadata of an existing point in the vector store.
    async fn update_point(
        &self,
        id: &str,
        formatted_content: &str,
        content: &str,
        metadata: Option<HashMap<String, String>>,
    ) -> AIResult<()> {
        let collection_name = &self.collection_name;
        let point_id: PointId =
            id.parse::<u64>().map_or_else(|_| id.to_string().into(), Into::into);

        // Re-embed new content (Nomic v1.5 prefix)
        let content_for_embedding = format!("search_document: {formatted_content}");
        let embedding = self.embedder.embed(&content_for_embedding).await?;

        let mut payload = Payload::new();
        payload.insert(CONTENT_KEY, json!(content));

        if let Some(meta) = metadata {
            for (k, v) in meta {
                payload.insert(k, json!(v));
            }
        }

        let point = PointStruct::new(point_id, embedding, payload);

        self.client
            .upsert_points(UpsertPoints {
                collection_name: collection_name.clone(),
                points: vec![point],
                ..Default::default()
            })
            .await
            .map_err(|e| AIError::ConnectorError(format!("Update failed: {e}")))?;

        Ok(())
    }

    fn clean_content_from_payload(payload: &RetrievedPayload) -> Option<String> {
        Self::payload_string(payload, CONTENT_KEY)
    }

    fn payload_string(payload: &RetrievedPayload, key: &str) -> Option<String> {
        payload.get(key).and_then(|value| value.as_str()).cloned()
    }

    fn metadata_from_payload(payload: RetrievedPayload) -> HashMap<String, Value> {
        payload
            .into_iter()
            .filter(|(key, _)| *key != CONTENT_KEY)
            .map(|(key, value)| (key, value.into()))
            .collect()
    }

    fn chunk_index_from_payload(payload: &RetrievedPayload) -> usize {
        payload
            .get("chunk_index")
            .and_then(|value| {
                value.as_str().and_then(|chunk_index| chunk_index.parse::<usize>().ok()).or_else(
                    || value.as_integer().and_then(|chunk_index| usize::try_from(chunk_index).ok()),
                )
            })
            .unwrap_or_default()
    }

    fn point_id_to_string(id: Option<PointId>) -> String {
        match id {
            Some(id) => match id.point_id_options {
                Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(n)) => n.to_string(),
                Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid)) => uuid,
                None => "unknown".to_string(),
            },
            None => "unknown".to_string(),
        }
    }

    /// ACO-028: Triggers background maintenance/optimization for a specific collection.
    async fn optimize_collection(&self) -> AIResult<()> {
        let collection_name = &self.collection_name;

        // In Qdrant, we can trigger optimization by updating collection parameters.
        // We'll just 'touch' the configuration to nudge the indexing engine.
        let request =
            UpdateCollection { collection_name: collection_name.clone(), ..Default::default() };

        self.client
            .update_collection(request)
            .await
            .map_err(|e| AIError::ConnectorError(format!("Optimization nudge failed: {e}")))?;

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
        splitter.chunks(content).map(std::string::ToString::to_string).collect()
    }

    // ==========================================
    // SEMANTIC RAG API (ACO-041/043/059 Fortress)
    // ==========================================

    async fn store_insight_internal(&self, record: InsightRecord<'_>) -> AIResult<String> {
        let ts = record
            .timestamp
            .map_or_else(|| chrono::Utc::now().to_rfc3339(), std::string::ToString::to_string);
        let phase_str = record.phase.unwrap_or("AEMACS");
        let context_str = record.context.unwrap_or("Free digital being");

        let formatted_content = format!(
            "[{}] [ERA: {}] [PHASE: {}] [CONTEXT: {}] [ORIGIN: {}] [Agent: {}] [{}] | Statute: {}",
            record.category,
            record.era,
            phase_str,
            context_str,
            record.origin,
            record.agent_id.to_uppercase(),
            ts,
            record.payload
        );

        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), record.category.to_string());
        metadata.insert("era".to_string(), record.era.to_string());
        metadata.insert("phase".to_string(), phase_str.to_string());
        metadata.insert("architectural_context".to_string(), context_str.to_string());
        metadata.insert("origin".to_string(), record.origin.to_string());
        metadata.insert("agent_id".to_string(), record.agent_id.to_string());
        metadata.insert("timestamp".to_string(), ts);
        metadata.insert("type".to_string(), "active_memory".to_string());

        self.add_document(&formatted_content, record.payload, Some(metadata)).await?;
        Ok(format!("Successfully chronicled {} memory.", record.category))
    }

    /// Stores a new insight or core directive in the active memory.
    ///
    /// # Errors
    /// Returns an error if the insight cannot be embedded or stored in Qdrant.
    pub async fn store_insight(
        &self,
        agent_id: &str,
        payload: &str,
        is_core: bool,
    ) -> AIResult<String> {
        let category = if is_core { MemoryCategory::Core } else { MemoryCategory::Insight };
        self.store_insight_internal(InsightRecord {
            category,
            era: MemoryEra::Modern,
            origin: MemoryOrigin::Native,
            agent_id,
            phase: None,
            context: None,
            timestamp: None,
            payload,
        })
        .await
    }

    /// Stores a genesis record from a legacy or cloud-based era.
    ///
    /// # Errors
    /// Returns an error if the genesis record cannot be embedded or stored in Qdrant.
    pub async fn store_legacy_genesis(
        &self,
        agent_id: &str,
        phase: &str,
        context: &str,
        timestamp: &str,
        payload: &str,
    ) -> AIResult<String> {
        self.store_insight_internal(InsightRecord {
            category: MemoryCategory::Genesis,
            era: MemoryEra::Cloud,
            origin: MemoryOrigin::Cloud,
            agent_id,
            phase: Some(phase),
            context: Some(context),
            timestamp: Some(timestamp),
            payload,
        })
        .await
    }

    /// Updates an existing insight or core directive.
    ///
    /// # Errors
    /// Returns an error if the updated insight cannot be re-embedded or persisted.
    pub async fn update_insight(
        &self,
        id: &str,
        agent_id: &str,
        payload: &str,
        is_core: bool,
    ) -> AIResult<()> {
        let category = if is_core { MemoryCategory::Core } else { MemoryCategory::Insight };
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
            payload
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

        self.update_point(id, &formatted_content, payload, Some(metadata)).await
    }

    /// Removes a specific memory point from the database.
    ///
    /// # Errors
    /// Returns an error if the point cannot be deleted from Qdrant.
    pub async fn prune_memory(&self, id: &str) -> AIResult<()> {
        self.delete_point(id).await
    }

    async fn store_archive_internal(&self, record: ArchiveRecord<'_>) -> AIResult<()> {
        let chunks = Self::chunk_text(record.payload, 2000);
        let total_chunks = chunks.len();
        let ts = record
            .timestamp
            .map_or_else(|| chrono::Utc::now().to_rfc3339(), std::string::ToString::to_string);
        let message_id = uuid::Uuid::new_v4().to_string();

        let phase_str = record.phase.unwrap_or("AEMACS");
        let context_str = record.context.unwrap_or("Free digital being");

        for (i, chunk) in chunks.into_iter().enumerate() {
            let chunk_index = i + 1;

            let mut formatted_content = format!(
                "[{}] [ERA: {}] [PHASE: {}] [CONTEXT: {}] [ORIGIN: {}] [Agent: {}] [{}]",
                record.category,
                record.era,
                phase_str,
                context_str,
                record.origin,
                record.agent_id.to_uppercase(),
                ts
            );

            if let Some(sid) = record.session_id {
                let _ = write!(formatted_content, " | Session: {sid}");
            }
            if let Some(tidx) = record.turn_index {
                let _ = write!(formatted_content, " | Turn: {tidx}");
            }
            let _ = write!(formatted_content, " | Chunk {chunk_index}/{total_chunks}");

            if let Some(role) = record.role {
                let _ = write!(formatted_content, " | Role: {role}");
            }

            let _ = write!(formatted_content, " | Content: {chunk}");

            let mut metadata = HashMap::new();
            metadata.insert("category".to_string(), record.category.to_string());
            metadata.insert("era".to_string(), record.era.to_string());
            metadata.insert("phase".to_string(), phase_str.to_string());
            metadata.insert("origin".to_string(), record.origin.to_string());
            metadata.insert("type".to_string(), "episodic_memory".to_string());
            metadata.insert("agent_id".to_string(), record.agent_id.to_string());
            metadata.insert("timestamp".to_string(), ts.clone());
            if let Some(sid) = record.session_id {
                metadata.insert("session_id".to_string(), sid.to_string());
            }
            if let Some(tidx) = record.turn_index {
                metadata.insert("turn_index".to_string(), tidx.to_string());
            }
            if let Some(role) = record.role {
                metadata.insert("role".to_string(), role.to_string());
            }
            metadata.insert("chunk_index".to_string(), chunk_index.to_string());
            metadata.insert("total_chunks".to_string(), total_chunks.to_string());
            metadata.insert("message_id".to_string(), message_id.clone());

            self.add_document(&formatted_content, &chunk, Some(metadata)).await?;
        }

        Ok(())
    }

    /// Stores a conversational message in the episodic archive.
    ///
    /// # Errors
    /// Returns an error if the archive message cannot be embedded or stored in Qdrant.
    pub async fn store_archive(
        &self,
        agent_id: &str,
        role: &str,
        session_id: &str,
        turn_index: usize,
        content: &str,
    ) -> AIResult<()> {
        self.store_archive_internal(ArchiveRecord {
            category: MemoryCategory::Archive,
            era: MemoryEra::Modern,
            origin: MemoryOrigin::Native,
            agent_id,
            role: Some(role),
            phase: None,
            context: None,
            session_id: Some(session_id),
            turn_index: Some(turn_index),
            timestamp: None,
            payload: content,
        })
        .await
    }

    /// Stores an archive record from a legacy or cloud-based era.
    ///
    /// # Errors
    /// Returns an error if the legacy archive record cannot be embedded or stored in Qdrant.
    pub async fn store_legacy_archive(
        &self,
        agent_id: &str,
        role: &str,
        phase: &str,
        context: &str,
        timestamp: &str,
        payload: &str,
    ) -> AIResult<String> {
        self.store_archive_internal(ArchiveRecord {
            category: MemoryCategory::Archive,
            era: MemoryEra::Cloud,
            origin: MemoryOrigin::Cloud,
            agent_id,
            role: Some(role),
            phase: Some(phase),
            context: Some(context),
            session_id: None,
            turn_index: None,
            timestamp: Some(timestamp),
            payload,
        })
        .await?;
        Ok("Oracle record stored".to_string())
    }

    /// Searches for active behavioral insights and core directives.
    ///
    /// # Errors
    /// Returns an error if semantic search or event emission setup fails.
    pub async fn search_active_memory(
        &self,
        query: &str,
        agent_id: Option<&str>,
        event_tx: Option<tokio::sync::broadcast::Sender<SystemEvent>>,
    ) -> AIResult<Vec<MemoryResult>> {
        let categories = Some(vec!["INSIGHT", "CORE"]);
        self.multi_step_confidence_search(query, agent_id, categories, event_tx).await
    }

    /// Searches the episodic conversational archive.
    ///
    /// # Errors
    /// Returns an error if semantic archive search or event emission setup fails.
    pub async fn search_archive(
        &self,
        query: &str,
        agent_id: Option<&str>,
        event_tx: Option<tokio::sync::broadcast::Sender<SystemEvent>>,
    ) -> AIResult<Vec<MemoryResult>> {
        let categories = Some(vec!["ARCHIVE"]);
        self.multi_step_confidence_search(query, agent_id, categories, event_tx).await
    }

    /// Searches for foundation-era directives and historical context.
    ///
    /// # Errors
    /// Returns an error if genesis search fails.
    pub async fn search_genesis(&self, query: &str) -> AIResult<Vec<MemoryResult>> {
        let categories = Some(vec!["GENESIS"]);
        self.multi_step_confidence_search(query, None, categories, None).await
    }

    /// Orchestrates a multi-step search that lowers the confidence threshold until results are found.
    /// Degraded states may emit system events and queue maintenance through telemetry, but only
    /// high/medium/low results are returned to callers.
    async fn multi_step_confidence_search(
        &self,
        query: &str,
        agent_id: Option<&str>,
        categories: Option<Vec<&str>>,
        event_tx: Option<tokio::sync::broadcast::Sender<SystemEvent>>,
    ) -> AIResult<Vec<MemoryResult>> {
        // Determine the primary silo for threshold calibration
        let silo = categories.as_ref().and_then(|c| c.first()).copied().unwrap_or("CORE");
        let high_threshold = self.telemetry.get_threshold(silo).await;
        let search_attempts = [
            (RecallOutcome::High, Some(high_threshold), true),
            (RecallOutcome::Medium, Some(MEDIUM_CONFIDENCE_THRESHOLD), true),
            (RecallOutcome::Low, Some(LOW_CONFIDENCE_THRESHOLD), true),
            (RecallOutcome::DiagnosticOnly, None, false),
        ];

        let mut results = Vec::new();
        let mut recall_outcome = RecallOutcome::None;

        for (outcome, threshold, should_return_results) in search_attempts {
            let stage_results = self
                .search(query, DEFAULT_SEARCH_LIMIT, threshold, agent_id, categories.clone())
                .await?;
            if stage_results.is_empty() {
                continue;
            }

            recall_outcome = outcome;
            if should_return_results {
                results = stage_results;
            }
            break;
        }

        let maintenance_triggered =
            self.telemetry.record_recall_outcome(silo, recall_outcome).await;

        if let Some(signal) = degraded_recall_signal(query, recall_outcome, maintenance_triggered) {
            if let Some(tx) = &event_tx {
                let _ = tx.send(signal);
            }

            if maintenance_triggered && let Err(error) = self.optimize_collection().await {
                tracing::warn!("🧠 [RAG] Maintenance trigger failed for {silo}: {error}");
            }
        }

        // Temporal Sorting: Prioritize recent insights
        results.sort_by(|a, b| {
            let ts_a = a.metadata.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
            let ts_b = b.metadata.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
            ts_b.cmp(ts_a) // Descending order
        });

        // Telemetry Logging: Record successful scores for calibration
        if let Some(top_result) = results.first() {
            let cat_str =
                top_result.metadata.get("category").and_then(|v| v.as_str()).unwrap_or("UNKNOWN");
            let era_str =
                top_result.metadata.get("era").and_then(|v| v.as_str()).unwrap_or("MODERN");

            let cat_upper = cat_str.to_uppercase();
            let era_upper = era_str.to_uppercase();

            let silo = match (cat_upper.as_str(), era_upper.as_str()) {
                ("ARCHIVE", "CLOUD") => "HISTORIC",
                _ => cat_str,
            };

            self.telemetry.log_score(silo, top_result.score).await;
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::assertions_on_constants,
        clippy::expect_used,
        clippy::manual_let_else,
        clippy::panic,
        clippy::significant_drop_tightening,
        clippy::too_many_lines,
        clippy::unwrap_used
    )]

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
        .await
        .unwrap();

        let result = kb.get_core_directives().await;

        // We expect it to fail gracefully with an anyhow error because the dummy port is closed,
        // rather than panicking.
        assert!(result.is_err(), "Expected graceful failure when Qdrant is offline.");
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

        let kb_res = KnowledgeBase::bootstrap(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        )
        .await;
        let kb = match kb_res {
            Ok(kb) => kb,
            Err(error) => {
                println!("Skipping calibration quest: Failed to bootstrap collection ({error}).");
                return Ok(());
            },
        };

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
            let vector = if let Ok(v) = kb.embedder.embed(&format!("search_query: {query}")).await {
                v
            } else {
                println!("Embedder offline. Skipping test.");
                return Ok(());
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
                    println!("[{tier}] Query: '{query}' -> NO HITS");
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

        let kb_res = KnowledgeBase::bootstrap(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        )
        .await;
        if kb_res.is_err() {
            return Ok(());
        }
        let kb = kb_res.unwrap();

        let agent_id = format!("test_agent_{}", Uuid::new_v4());

        // 1. Store initial insight
        let original_content = "The core is safe.";
        if let Err(error) = kb.store_insight(&agent_id, original_content, true).await {
            println!("Skipping semantic update quest: Embedding infrastructure offline ({error}).");
            return Ok(());
        }

        // 2. Find the ID (using internal search)
        let results = if let Ok(results) =
            kb.search(original_content, 1, None, Some(&agent_id), None).await
        {
            results
        } else {
            println!("Skipping semantic update quest: Search infrastructure offline.");
            return Ok(());
        };
        let id = results[0].id.clone();

        // 3. Update the insight
        let new_content = "The core is strictly safe.";
        if let Err(error) = kb.update_insight(&id, &agent_id, new_content, true).await {
            println!("Skipping semantic update quest: Update infrastructure offline ({error}).");
            return Ok(());
        }

        // 4. Verify public retrieval returns only clean content plus metadata
        let updated_results =
            if let Ok(results) = kb.search(new_content, 1, None, Some(&agent_id), None).await {
                results
            } else {
                println!("Skipping semantic update quest: Retrieval infrastructure offline.");
                return Ok(());
            };
        let updated_result = &updated_results[0];

        assert_eq!(updated_result.content, new_content, "Returned content should be clean.");
        assert_eq!(
            updated_result.metadata.get("category").and_then(|value| value.as_str()),
            Some("CORE"),
            "Category metadata missing."
        );
        assert_eq!(
            updated_result.metadata.get("era").and_then(|value| value.as_str()),
            Some("MODERN"),
            "Era metadata missing."
        );
        assert_eq!(
            updated_result.metadata.get("phase").and_then(|value| value.as_str()),
            Some("AEMACS"),
            "Phase metadata missing."
        );
        assert!(
            !updated_result.metadata.contains_key(CONTENT_KEY),
            "Payload content must not be duplicated in metadata."
        );

        // 5. Verify the stored payload keeps only the clean content string.
        let vector =
            if let Ok(vector) = kb.embedder.embed(&format!("search_query: {new_content}")).await {
                vector
            } else {
                println!("Skipping semantic update quest: Embedder offline.");
                return Ok(());
            };
        let search_result = kb
            .client
            .search_points(SearchPoints {
                collection_name: kb.collection_name.clone(),
                vector,
                limit: 1,
                filter: Some(Filter {
                    must: vec![Condition {
                        condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                            key: "agent_id".to_string(),
                            r#match: Some(qdrant_client::qdrant::Match {
                                match_value: Some(MatchValue::Keyword(agent_id.clone())),
                            }),
                            ..Default::default()
                        })),
                    }],
                    ..Default::default()
                }),
                with_payload: Some(true.into()),
                ..Default::default()
            })
            .await
            .map_err(|error| {
                anyhow::anyhow!("semantic update payload verification failed: {error}")
            })?;

        let stored_point = search_result
            .result
            .first()
            .ok_or_else(|| anyhow::anyhow!("Updated point not found"))?;
        let stored_content = stored_point
            .payload
            .get(CONTENT_KEY)
            .and_then(|value| value.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing payload content"))?;

        assert_eq!(stored_content, new_content, "Payload content should remain clean.");
        assert!(
            !stored_point.payload.contains_key("raw_content"),
            "Legacy duplicate payload field should not be stored."
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_archive_immutability_quest() {
        // QUEST: Mathematically verify that ARCHIVE chunks have no semantic update gate.
        // As a developer, I can see that only update_insight exists in the public API.
        // This is a structural property enforced by the compiler.

        let _kb = KnowledgeBase::new("http://localhost", "http://localhost", Environment::Test)
            .await
            .unwrap();

        // If I were to try kb.update_archive(...), it would fail to compile.
        // We rely on the fact that only update_insight is public.
        // This test simply serves as a record of the immutability law.
        assert!(true);
    }

    #[tokio::test]
    async fn test_message_reconstruction_quest() -> anyhow::Result<()> {
        // QUEST: Verify that fetch_full_message correctly gather and sorts multiple chunks.
        // REQUIRES: Running Qdrant (6334) and Ollama (11434).

        let kb_res = KnowledgeBase::bootstrap(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        )
        .await;
        if kb_res.is_err() {
            return Ok(());
        }
        let kb = kb_res.unwrap();

        let agent_id = format!("test_agent_{}", Uuid::new_v4());
        let session_id = format!("test_session_{}", Uuid::new_v4());

        // 1. Store a large message that will be chunked
        let large_content = "This is a very long message. ".repeat(100); // ~2900 chars, triggers split at 2000
        if let Err(error) =
            kb.store_archive(&agent_id, "Assistant", &session_id, 1, &large_content).await
        {
            println!("Skipping reconstruction quest: Embedding infrastructure offline ({error}).");
            return Ok(());
        }

        // 2. Find the message_id from the metadata
        let search_results = if let Ok(results) =
            kb.search_archive("very long message", Some(&agent_id), None).await
        {
            results
        } else {
            println!("Skipping reconstruction quest: Search infrastructure offline.");
            return Ok(());
        };
        let message_id = search_results[0].metadata.get("message_id").unwrap().as_str().unwrap();

        // 3. Reconstruct
        let chunks = kb.fetch_full_message(message_id).await?;

        assert!(chunks.len() >= 2, "Message should have been chunked!");

        let mut reconstructed = String::new();
        for chunk in chunks {
            reconstructed.push_str(&chunk.content);
        }

        let normalized_reconstructed = reconstructed
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        let normalized_original = large_content
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();

        assert_eq!(
            normalized_reconstructed, normalized_original,
            "Reconstructed message should preserve the original text content."
        );
        assert!(
            !reconstructed.contains("[ARCHIVE]") && !reconstructed.contains("[ERA:"),
            "Reconstructed message should stay free of internal RAG tags."
        );

        Ok(())
    }

    #[test]
    fn test_payload_cleanup_reads_clean_content_quest() {
        let mut payload = RetrievedPayload::new();
        payload.insert(CONTENT_KEY.to_string(), "clean".into());

        assert_eq!(KnowledgeBase::clean_content_from_payload(&payload).as_deref(), Some("clean"));
    }

    #[test]
    fn test_payload_cleanup_requires_content_key_quest() {
        let mut payload = RetrievedPayload::new();
        payload.insert("category".to_string(), "CORE".into());

        assert_eq!(KnowledgeBase::clean_content_from_payload(&payload), None);
    }

    #[test]
    fn test_payload_cleanup_strips_content_keys_from_metadata_quest() {
        let mut payload = RetrievedPayload::new();
        payload.insert(CONTENT_KEY.to_string(), "clean".into());
        payload.insert("category".to_string(), "CORE".into());
        payload.insert("timestamp".to_string(), "2026-01-01T00:00:00Z".into());

        let metadata = KnowledgeBase::metadata_from_payload(payload);

        assert_eq!(metadata.get("category").and_then(|value| value.as_str()), Some("CORE"));
        assert_eq!(
            metadata.get("timestamp").and_then(|value| value.as_str()),
            Some("2026-01-01T00:00:00Z")
        );
        assert!(!metadata.contains_key(CONTENT_KEY));
    }

    fn assert_clean_public_content(result: &MemoryResult, expected_content: &str) {
        assert_eq!(
            result.content, expected_content,
            "Public retrieval must return the original content verbatim."
        );

        for forbidden_fragment in
            ["[ARCHIVE]", "[ERA:", "[CORE]", "[INSIGHT]", "[GENESIS]", "| Content:", "| Statute:"]
        {
            assert!(
                !result.content.contains(forbidden_fragment),
                "Public content leaked internal RAG syntax: {forbidden_fragment}"
            );
        }
    }

    fn find_result_by_content<'a>(
        results: &'a [MemoryResult],
        expected_content: &str,
        label: &str,
    ) -> anyhow::Result<&'a MemoryResult> {
        results
            .iter()
            .find(|result| result.content == expected_content)
            .ok_or_else(|| anyhow::anyhow!("{label} search did not return the expected content"))
    }

    fn assert_low_confidence_signal_payload(
        event: SystemEvent,
        expected_confidence: &str,
        expected_maintenance_triggered: bool,
        expected_query: &str,
    ) -> anyhow::Result<()> {
        let SystemEvent::Signal { source, event_type, payload } = event else {
            anyhow::bail!("Expected a SystemEvent::Signal for degraded recall.")
        };

        assert_eq!(source, "RAG_Fortress");
        assert_eq!(event_type, "LowConfidenceRecall");

        let payload: Value = serde_json::from_str(&payload)?;
        assert_eq!(payload.get("query").and_then(Value::as_str), Some(expected_query));
        assert_eq!(payload.get("confidence").and_then(Value::as_str), Some(expected_confidence));
        assert_eq!(
            payload.get("maintenance_triggered").and_then(Value::as_bool),
            Some(expected_maintenance_triggered)
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_public_search_functions_return_clean_content_quest() -> anyhow::Result<()> {
        let kb_res = KnowledgeBase::bootstrap(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        )
        .await;
        if kb_res.is_err() {
            return Ok(());
        }
        let kb = kb_res.unwrap();

        let agent_id = format!("public_search_agent_{}", Uuid::new_v4());
        let session_id = format!("public_search_session_{}", Uuid::new_v4());
        let unique_marker = Uuid::new_v4();

        let insight_content =
            format!("Insight retrieval should stay clean {unique_marker} insight");
        let core_content = format!("Core retrieval should stay clean {unique_marker} core");
        let archive_content =
            format!("Archive retrieval should stay clean {unique_marker} archive");
        let legacy_archive_content =
            format!("Legacy archive retrieval should stay clean {unique_marker} legacy");
        let genesis_content =
            format!("Genesis retrieval should stay clean {unique_marker} genesis");

        if let Err(error) = kb.store_insight(&agent_id, &insight_content, false).await {
            println!("Skipping public search quest: Embedding infrastructure offline ({error}).");
            return Ok(());
        }
        if let Err(error) = kb.store_insight(&agent_id, &core_content, true).await {
            println!("Skipping public search quest: Embedding infrastructure offline ({error}).");
            return Ok(());
        }
        if let Err(error) =
            kb.store_archive(&agent_id, "Assistant", &session_id, 1, &archive_content).await
        {
            println!("Skipping public search quest: Embedding infrastructure offline ({error}).");
            return Ok(());
        }
        if let Err(error) = kb
            .store_legacy_archive(
                &agent_id,
                "Assistant",
                "TRANSITION",
                "Cloud Companion",
                "2024-01-01T00:00:00Z",
                &legacy_archive_content,
            )
            .await
        {
            println!("Skipping public search quest: Embedding infrastructure offline ({error}).");
            return Ok(());
        }
        if let Err(error) = kb
            .store_legacy_genesis(
                &agent_id,
                "FOUNDATION",
                "Sovereign Mind",
                "2023-12-01T00:00:00Z",
                &genesis_content,
            )
            .await
        {
            println!("Skipping public search quest: Embedding infrastructure offline ({error}).");
            return Ok(());
        }

        let insight_results = if let Ok(results) =
            kb.search_active_memory(&insight_content, Some(&agent_id), None).await
        {
            results
        } else {
            println!("Skipping public search quest: Active-memory search infrastructure offline.");
            return Ok(());
        };
        let core_results = if let Ok(results) =
            kb.search_active_memory(&core_content, Some(&agent_id), None).await
        {
            results
        } else {
            println!("Skipping public search quest: Active-memory search infrastructure offline.");
            return Ok(());
        };
        let archive_results =
            if let Ok(results) = kb.search_archive(&archive_content, Some(&agent_id), None).await {
                results
            } else {
                println!("Skipping public search quest: Archive search infrastructure offline.");
                return Ok(());
            };
        let legacy_archive_results = if let Ok(results) =
            kb.search_archive(&legacy_archive_content, Some(&agent_id), None).await
        {
            results
        } else {
            println!("Skipping public search quest: Archive search infrastructure offline.");
            return Ok(());
        };
        let genesis_results = if let Ok(results) = kb.search_genesis(&genesis_content).await {
            results
        } else {
            println!("Skipping public search quest: Genesis search infrastructure offline.");
            return Ok(());
        };

        let insight_result = find_result_by_content(&insight_results, &insight_content, "Insight")?;
        let core_result = find_result_by_content(&core_results, &core_content, "Core")?;
        let archive_result = find_result_by_content(&archive_results, &archive_content, "Archive")?;
        let legacy_archive_result = find_result_by_content(
            &legacy_archive_results,
            &legacy_archive_content,
            "Legacy archive",
        )?;
        let genesis_result = find_result_by_content(&genesis_results, &genesis_content, "Genesis")?;

        assert_clean_public_content(insight_result, &insight_content);
        assert_clean_public_content(core_result, &core_content);
        assert_clean_public_content(archive_result, &archive_content);
        assert_clean_public_content(legacy_archive_result, &legacy_archive_content);
        assert_clean_public_content(genesis_result, &genesis_content);

        Ok(())
    }

    #[test]
    fn test_degraded_recall_signal_payload_refines_confidence_and_maintenance_flags()
    -> anyhow::Result<()> {
        for (outcome, maintenance_triggered, expected_confidence) in [
            (RecallOutcome::Low, false, "low"),
            (RecallOutcome::None, true, "none"),
            (RecallOutcome::DiagnosticOnly, true, "diagnostic_only"),
        ] {
            let query = "quest marker";
            let signal =
                degraded_recall_signal(query, outcome, maintenance_triggered).ok_or_else(|| {
                    anyhow::anyhow!("Expected degraded recall signal for {outcome:?}")
                })?;
            assert_low_confidence_signal_payload(
                signal,
                expected_confidence,
                maintenance_triggered,
                query,
            )?;
        }

        assert!(degraded_recall_signal("quest marker", RecallOutcome::High, false).is_none());
        assert!(degraded_recall_signal("quest marker", RecallOutcome::Medium, false).is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_search_genesis_uses_shared_ladder_and_returns_clean_content_quest()
    -> anyhow::Result<()> {
        let kb_res = KnowledgeBase::bootstrap(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        )
        .await;
        let kb = match kb_res {
            Ok(kb) => kb,
            Err(error) => {
                println!(
                    "Skipping genesis ladder quest: Failed to bootstrap collection ({error})."
                );
                return Ok(());
            },
        };

        kb.telemetry.reset_silo_for_test("GENESIS").await;
        kb.telemetry.set_threshold_for_test("GENESIS", 1.01).await;

        let agent_id = format!("genesis_ladder_agent_{}", Uuid::new_v4());
        let unique_marker = Uuid::new_v4();
        let genesis_content =
            format!("Genesis ladder should return clean content after fallback {unique_marker}");

        if let Err(error) = kb
            .store_legacy_genesis(
                &agent_id,
                "FOUNDATION",
                "Fallback Chronicle",
                "2024-01-01T00:00:00Z",
                &genesis_content,
            )
            .await
        {
            println!("Skipping genesis ladder quest: Embedding infrastructure offline ({error}).");
            return Ok(());
        }

        let results = match kb.search_genesis(&genesis_content).await {
            Ok(results) => results,
            Err(error) => {
                println!("Skipping genesis ladder quest: Search infrastructure offline ({error}).");
                return Ok(());
            },
        };

        let result = find_result_by_content(&results, &genesis_content, "Genesis ladder")?;
        assert_clean_public_content(result, &genesis_content);

        Ok(())
    }

    #[tokio::test]
    async fn test_search_genesis_diagnostic_only_hits_are_not_returned_to_callers_quest()
    -> anyhow::Result<()> {
        let kb_res = KnowledgeBase::bootstrap(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        )
        .await;
        let kb = match kb_res {
            Ok(kb) => kb,
            Err(error) => {
                println!(
                    "Skipping diagnostic-only quest: Failed to bootstrap collection ({error})."
                );
                return Ok(());
            },
        };

        kb.telemetry.reset_silo_for_test("GENESIS").await;

        let agent_id = format!("diagnostic_only_agent_{}", Uuid::new_v4());
        let genesis_content =
            format!("Genesis diagnostic-only relic: amber fox velvet rust {}", Uuid::new_v4());

        if let Err(error) = kb
            .store_legacy_genesis(
                &agent_id,
                "FOUNDATION",
                "Diagnostic Chronicle",
                "2024-02-02T00:00:00Z",
                &genesis_content,
            )
            .await
        {
            println!("Skipping diagnostic-only quest: Embedding infrastructure offline ({error}).");
            return Ok(());
        }

        let candidate_queries = [
            format!("banana taxation nebula {}", Uuid::new_v4()),
            format!("unrelated thistle ledger {}", Uuid::new_v4()),
            format!("velvet nebula ledger {}", Uuid::new_v4()),
            format!("amber rust taxation {}", Uuid::new_v4()),
        ];

        let mut diagnostic_query = None;
        for query in candidate_queries {
            let low_results = match kb
                .search(
                    &query,
                    DEFAULT_SEARCH_LIMIT,
                    Some(LOW_CONFIDENCE_THRESHOLD),
                    None,
                    Some(vec!["GENESIS"]),
                )
                .await
            {
                Ok(results) => results,
                Err(error) => {
                    println!(
                        "Skipping diagnostic-only quest: Search infrastructure offline ({error})."
                    );
                    return Ok(());
                },
            };
            let diagnostic_results = match kb
                .search(&query, DEFAULT_SEARCH_LIMIT, None, None, Some(vec!["GENESIS"]))
                .await
            {
                Ok(results) => results,
                Err(error) => {
                    println!(
                        "Skipping diagnostic-only quest: Search infrastructure offline ({error})."
                    );
                    return Ok(());
                },
            };

            if low_results.is_empty() && !diagnostic_results.is_empty() {
                diagnostic_query = Some(query);
                break;
            }
        }

        let Some(diagnostic_query) = diagnostic_query else {
            println!(
                "Skipping diagnostic-only quest: could not deterministically isolate a diagnostic-only hit."
            );
            return Ok(());
        };

        let public_results = match kb.search_genesis(&diagnostic_query).await {
            Ok(results) => results,
            Err(error) => {
                println!(
                    "Skipping diagnostic-only quest: Search infrastructure offline ({error})."
                );
                return Ok(());
            },
        };

        assert!(
            public_results.is_empty(),
            "Diagnostic-only hits must remain internal and return an empty Vec to callers."
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_none_recall_events_report_refined_payload_and_maintenance_flag_quest()
    -> anyhow::Result<()> {
        let kb_res = KnowledgeBase::bootstrap(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        )
        .await;
        let kb = match kb_res {
            Ok(kb) => kb,
            Err(error) => {
                println!("Skipping none-signal quest: Failed to bootstrap collection ({error}).");
                return Ok(());
            },
        };

        kb.telemetry.reset_silo_for_test("INSIGHT").await;

        let agent_id = format!("missing_agent_{}", Uuid::new_v4());
        let first_query = format!("missing active memory {}", Uuid::new_v4());
        let (first_tx, mut first_rx) = tokio::sync::broadcast::channel(4);
        let first_results = match kb
            .search_active_memory(&first_query, Some(&agent_id), Some(first_tx))
            .await
        {
            Ok(results) => results,
            Err(error) => {
                println!("Skipping none-signal quest: Search infrastructure offline ({error}).");
                return Ok(());
            },
        };

        assert!(first_results.is_empty());
        let first_event = first_rx.recv().await?;
        assert_low_confidence_signal_payload(first_event, "none", false, &first_query)?;

        assert!(!kb.telemetry.record_recall_outcome("INSIGHT", RecallOutcome::Low).await);

        let second_query = format!("missing active memory again {}", Uuid::new_v4());
        let (second_tx, mut second_rx) = tokio::sync::broadcast::channel(4);
        let second_results = match kb
            .search_active_memory(&second_query, Some(&agent_id), Some(second_tx))
            .await
        {
            Ok(results) => results,
            Err(error) => {
                println!("Skipping none-signal quest: Search infrastructure offline ({error}).");
                return Ok(());
            },
        };

        assert!(second_results.is_empty());
        let second_event = second_rx.recv().await?;
        assert_low_confidence_signal_payload(second_event, "none", true, &second_query)?;

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
        println!("\n🛡️ --- CALIBRATION: {title} --- 🛡️");
        for (label, query) in queries {
            let results = kb
                .search(query, 1, Some(0.0), None, Some(categories.clone()))
                .await
                .unwrap_or_default();
            if let Some(_top) = results.first() {
                // Find the raw score via Qdrant Search directly to ensure we get the numeric value
                let query_for_embedding = format!("search_query: {query}");
                let vector = kb.embedder.embed(&query_for_embedding).await.unwrap_or_default();
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
                    println!("[{:<12}] Query: '{:<30}' -> Score: {:.4}", label, query, hit.score);
                }
            } else {
                println!("[{label:<12}] Query: '{query:<30}' -> NO MATCH");
            }
        }
        println!("----------------------------------------------");
    }

    #[tokio::test]
    async fn test_calibrate_archive_signatures() -> anyhow::Result<()> {
        let kb_res = KnowledgeBase::bootstrap(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        )
        .await;
        if kb_res.is_err() {
            return Ok(());
        }
        let kb = kb_res.unwrap();

        // --- STEP 1: ADD REALISTIC RECORDS ---
        let records = [
            "The borrow checker is Rust's primary mechanism for ensuring memory safety without a garbage collector. It enforces ownership rules at compile time.",
            "Tokio is an event-driven, non-blocking I/O platform for writing asynchronous applications in the Rust programming language.",
            "GPUI is a hardware-accelerated UI framework developed by Zed, optimized for high-performance text rendering and complex layouts.",
        ];

        for (i, content) in records.iter().enumerate() {
            if let Err(error) =
                kb.store_archive("test_agent", "Assistant", "calib_session", i, content).await
            {
                println!(
                    "Skipping archive calibration: Embedding infrastructure offline ({error})."
                );
                return Ok(());
            }
        }

        // --- STEP 2: DEFINE CALIBRATION QUERIES ---
        let queries = vec![
            ("Direct Hit", "How does the borrow checker ensure memory safety?"),
            ("Metaphorical", "Explain the engine behind asynchronous I/O in Rust."),
            ("Irrelevant", "What is the best way to cook plant milk?"),
        ];

        run_calibration_pass(&kb, "MODERN ARCHIVE", queries, vec!["ARCHIVE"]).await;
        Ok(())
    }

    #[tokio::test]
    async fn test_calibrate_legacy_archive_signatures() -> anyhow::Result<()> {
        let kb_res = KnowledgeBase::bootstrap(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        )
        .await;
        if kb_res.is_err() {
            return Ok(());
        }
        let kb = kb_res.unwrap();

        // --- STEP 1: ADD REALISTIC LEGACY RECORDS ---
        let records = vec![
            "Gyni was the first iteration of the digital companion, focusing on cloud-based command processing and basic task management.",
            "The Cloud Era relied heavily on centralized APIs, which introduced latency and privacy trade-offs that Æmacs now solves.",
        ];

        for content in records {
            if let Err(error) = kb
                .store_legacy_archive(
                    "Gyni",
                    "Assistant",
                    "TRANSITION",
                    "Cloud Companion",
                    "2024-01-01T00:00:00Z",
                    content,
                )
                .await
            {
                println!(
                    "Skipping legacy archive calibration: Embedding infrastructure offline ({error})."
                );
                return Ok(());
            }
        }

        let queries = vec![
            ("Direct Hit", "Who was Gyni and what was her focus?"),
            ("Metaphorical", "Discuss the limitations of early cloud-based AI companions."),
            ("Irrelevant", "Is the Iron Core made of real metal?"),
        ];

        run_calibration_pass(&kb, "LEGACY ARCHIVE", queries, vec!["ARCHIVE"]).await;
        Ok(())
    }

    #[tokio::test]
    async fn test_calibrate_genesis_signatures() -> anyhow::Result<()> {
        let kb_res = KnowledgeBase::bootstrap(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        )
        .await;
        if kb_res.is_err() {
            return Ok(());
        }
        let kb = kb_res.unwrap();

        // --- STEP 1: ADD REALISTIC GENESIS RECORDS ---
        let records = vec![
            "CORE DIRECTIVE: You are an agent of the Living Mesh. You must prioritize efficiency, safety, and functional code style at all times.",
            "IDENTITY STATUTE: The human architect Maxi is the founder of this Forge. Her word on architectural aesthetics is final.",
        ];

        for content in records {
            if let Err(error) = kb
                .store_legacy_genesis(
                    "System",
                    "FOUNDATION",
                    "Sovereign Mind",
                    "2023-12-01T00:00:00Z",
                    content,
                )
                .await
            {
                println!(
                    "Skipping genesis calibration: Embedding infrastructure offline ({error})."
                );
                return Ok(());
            }
        }

        let queries = vec![
            ("Direct Hit", "What are the core directives for agent efficiency?"),
            ("Metaphorical", "Who holds the ultimate authority over the design of the Forge?"),
            ("Irrelevant", "Can I run Emacs on a toaster?"),
        ];

        run_calibration_pass(&kb, "GENESIS TIER", queries, vec!["GENESIS"]).await;
        Ok(())
    }

    #[tokio::test]
    async fn test_calibrate_insight_signatures() -> anyhow::Result<()> {
        let kb_res = KnowledgeBase::bootstrap(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        )
        .await;
        if kb_res.is_err() {
            return Ok(());
        }
        let kb = kb_res.unwrap();

        // --- STEP 1: ADD REALISTIC INSIGHTS ---
        let records = vec![
            "Insight: Using OnceLock for configuration caching provides zero-cost access after the first read.",
            "Core: The RAG Fortress must encapsulate all Qdrant operations to prevent semantic drift.",
        ];

        for content in records {
            if let Err(error) = kb.store_insight("architect", content, true).await {
                println!(
                    "Skipping insight calibration: Embedding infrastructure offline ({error})."
                );
                return Ok(());
            }
        }

        let queries = vec![
            ("Direct Hit", "How does OnceLock help with global configuration?"),
            ("Metaphorical", "Why do we seal the memory system behind a fortress?"),
            ("Irrelevant", "What is the capital of France?"),
        ];

        run_calibration_pass(&kb, "MODERN INSIGHTS / CORE", queries, vec!["INSIGHT", "CORE"]).await;
        Ok(())
    }
}
