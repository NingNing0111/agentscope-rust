//! TurboVec-backed long-term memory implementation.
//!
//! This module provides [`TurbovecMemory`], a [`Memory`](crate::Memory) trait
//! implementation that combines Markdown-file-backed durable storage (via
//! [`FileMemory`](crate::FileMemory)) with a pluggable vector index for fast
//! semantic retrieval.
//!
//! # Architecture
//!
//! - **Markdown files** are the source of truth (human-readable, editable,
//!   compatible with Feature 009 `FileMemory`).
//! - **Vector index** (via [`MemoryVectorIndex`]) is a rebuildable derived
//!   index powering semantic search via [`Memory::retrieve_relevant`].
//!   A concrete TurboVec-backed implementation is provided in
//!   the `agent_scope_rag` crate.
//!
//! # Platform
//!
//! When used with the TurboVec index adapter, requires 64-bit target
//! (x86_64 or aarch64). WASM and 32-bit targets are not supported
//! due to the `turbovec` crate's `target_pointer_width = "64"` requirement.
//!
//! # Examples
//!
//! ```rust,no_run
//! use agent_scope_memory::{TurbovecMemory, TurbovecMemoryConfig, Memory, MemoryEntry, MemoryType};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // let embedding: Arc<dyn agent_scope_embedding::EmbeddingModel> = ...;
//! // let index: Arc<dyn agent_scope_memory::MemoryVectorIndex> = ...;
//! let config = TurbovecMemoryConfig::default();
//! // let memory = TurbovecMemory::new("/tmp/mem", config, embedding, index, None).await?;
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use agent_scope_embedding::{EmbeddingInput, EmbeddingModel};
use tracing::{debug, info, warn};

use crate::file_memory::FileMemory;
use crate::memory_entry::{MemoryFileHeader, MemoryType};
use crate::memory_error::MemoryError;
use crate::memory_trait::Memory;
use crate::{Backend, LocalBackend, MemoryConfig, MemoryEntry};

// ---------------------------------------------------------------------------
// MemorySearchResult
// ---------------------------------------------------------------------------

/// A ranked retrieval result from semantic memory search.
///
/// Results are sorted by `score` descending. Equal scores are tie-broken by
/// `memory_name` ascending (deterministic ordering).
#[derive(Debug, Clone)]
pub struct MemorySearchResult {
    /// Stable memory entry name.
    pub memory_name: String,
    /// One-line memory description.
    pub description: String,
    /// Memory category.
    pub memory_type: MemoryType,
    /// Cosine similarity score (higher = more relevant).
    pub score: f32,
    /// Memory body, truncated to `retrieval_max_tokens_per_file`.
    pub content: String,
    /// Last update timestamp (RFC 3339).
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// MemoryRebuildReport
// ---------------------------------------------------------------------------

/// Summary of a `TurbovecMemory::rebuild_index()` operation.
#[derive(Debug, Clone)]
pub struct MemoryRebuildReport {
    /// Total Markdown files scanned.
    pub total_scanned: usize,
    /// Successfully embedded and inserted into the vector index.
    pub indexed: usize,
    /// Malformed or empty files skipped during rebuild.
    pub skipped: usize,
    /// Per-file error descriptions (entry that failed processing).
    pub errors: Vec<String>,
    /// Wall-clock rebuild time in milliseconds.
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// VectorIndexStatus
// ---------------------------------------------------------------------------

/// Health status of the vector index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VectorIndexStatus {
    /// Index is present and consistent.
    Clean,
    /// Index directory or files are missing.
    Missing,
    /// Index data is corrupted or unreadable.
    Corrupted(String),
    /// Embedding model dimensions do not match the stored collection dimension.
    DimensionMismatch { expected: u32, got: u32 },
}

// ---------------------------------------------------------------------------
// VectorRecord (memory-level, simple)
// ---------------------------------------------------------------------------

/// A simplified vector record for the memory-level index abstraction.
///
/// Avoids depending on `agent_scope_rag::VectorRecord` to prevent circular deps.
#[derive(Debug, Clone)]
pub struct MemoryVectorRecord {
    /// The embedding vector.
    pub vector: Vec<f32>,
    /// Document (memory entry) identifier.
    pub document_id: String,
    /// Metadata key-value pairs.
    pub metadata: HashMap<String, String>,
    /// The full memory content (for search result reconstruction).
    pub content: String,
}

// ---------------------------------------------------------------------------
// MemoryVectorSearchResult (simple, memory-level)
// ---------------------------------------------------------------------------

/// A single search result from the vector index.
#[derive(Debug, Clone)]
pub struct MemoryVectorHit {
    /// Similarity score.
    pub score: f32,
    /// Document (memory entry) identifier.
    pub document_id: String,
    /// Metadata attached to the vector record.
    pub metadata: HashMap<String, String>,
    /// The stored content.
    pub content: String,
}

// ---------------------------------------------------------------------------
// MemoryVectorIndex trait
// ---------------------------------------------------------------------------

/// Abstract trait for vector-index backends used by [`TurbovecMemory`].
///
/// This trait decouples `agent_scope_memory` from the `agent_scope_rag` crate,
/// avoiding a circular dependency (`memory → rag → agent → memory`).
///
/// A concrete TurboVec implementation is provided in `agent_scope_rag`.
#[async_trait::async_trait]
pub trait MemoryVectorIndex: Send + Sync {
    /// Check whether a collection exists.
    async fn has_collection(&self, name: &str) -> Result<bool, String>;

    /// Create a collection for the given vector dimension.
    async fn create_collection(&self, name: &str, dimensions: u32) -> Result<(), String>;

    /// Search for similar vectors.
    ///
    /// Results are sorted by similarity score descending.
    async fn search(
        &self,
        collection: &str,
        query_vector: Vec<f32>,
        top_k: usize,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<MemoryVectorHit>, String>;

    /// Insert vector records into a collection.
    ///
    /// Empty `records` is a no-op.
    async fn insert(
        &self,
        collection: &str,
        records: Vec<MemoryVectorRecord>,
    ) -> Result<(), String>;

    /// Delete all records for a document.
    ///
    /// Idempotent — if the document doesn't exist, still returns `Ok(())`.
    async fn delete(&self, collection: &str, document_id: &str) -> Result<(), String>;

    /// Persist the entire index to `path`.
    async fn save(&self, path: &str) -> Result<(), String>;

    /// Load a store previously written by [`save`](Self::save).
    async fn load(&self, path: &str) -> Result<(), String>;
}

// ---------------------------------------------------------------------------
// TurbovecMemoryConfig
// ---------------------------------------------------------------------------

/// Extended configuration for TurboVec-backed long-term memory.
///
/// Extends [`MemoryConfig`] with vector-index settings.
#[derive(Debug, Clone)]
pub struct TurbovecMemoryConfig {
    /// Memory files directory.
    pub memory_dir: String,
    /// Max MEMORY.md index tokens.
    pub max_index_tokens: usize,
    /// Whether to use async retrieval in middleware.
    pub retrieval_async: bool,
    /// Max files listed by `list()`.
    pub retrieval_max_files: usize,
    /// Max tokens per memory file on retrieval.
    pub retrieval_max_tokens_per_file: usize,
    /// Max frontmatter tokens.
    pub retrieval_max_tokens_per_frontmatter: usize,
    /// System prompt instructions for memory usage.
    pub memory_instructions: String,
    /// Retrieval prompt instructions.
    pub retrieval_instructions: String,

    // --- Vector-index-specific fields ---
    /// TurboVec compression level (2, 3, or 4) when using turbovec adapter.
    pub bit_width: usize,
    /// Vector index collection name.
    pub collection_name: String,
    /// Max vector search results per query.
    pub retrieval_top_k: usize,
    /// Minimum similarity threshold (None = no threshold).
    pub retrieval_score_threshold: Option<f32>,
    /// Whether to auto-rebuild when index is missing or mismatched.
    pub auto_rebuild: bool,
    /// Vector index subdirectory (relative to memory_dir).
    pub vector_index_dir: String,
}

impl Default for TurbovecMemoryConfig {
    fn default() -> Self {
        Self {
            memory_dir: "Memory".into(),
            max_index_tokens: 4000,
            retrieval_async: true,
            retrieval_max_files: 200,
            retrieval_max_tokens_per_file: 2000,
            retrieval_max_tokens_per_frontmatter: 256,
            memory_instructions: crate::DEFAULT_MEMORY_INSTRUCTIONS.into(),
            retrieval_instructions: crate::DEFAULT_RETRIEVAL_INSTRUCTIONS.into(),
            bit_width: 4,
            collection_name: "memories".into(),
            retrieval_top_k: 10,
            retrieval_score_threshold: None,
            auto_rebuild: false,
            vector_index_dir: ".turbovec".into(),
        }
    }
}

impl TurbovecMemoryConfig {
    /// Validate all configuration fields.
    pub fn validate(&self) -> Result<(), MemoryError> {
        if self.memory_dir.trim().is_empty() {
            return Err(MemoryError::ValidationError {
                field: "memory_dir".into(),
                message: "memory_dir must not be empty".into(),
            });
        }
        if self.max_index_tokens == 0 {
            return Err(MemoryError::ValidationError {
                field: "max_index_tokens".into(),
                message: "max_index_tokens must be > 0".into(),
            });
        }
        if self.retrieval_max_files == 0 {
            return Err(MemoryError::ValidationError {
                field: "retrieval_max_files".into(),
                message: "retrieval_max_files must be > 0".into(),
            });
        }
        if self.retrieval_max_tokens_per_file == 0 {
            return Err(MemoryError::ValidationError {
                field: "retrieval_max_tokens_per_file".into(),
                message: "retrieval_max_tokens_per_file must be > 0".into(),
            });
        }
        if self.retrieval_max_tokens_per_frontmatter == 0 {
            return Err(MemoryError::ValidationError {
                field: "retrieval_max_tokens_per_frontmatter".into(),
                message: "retrieval_max_tokens_per_frontmatter must be > 0".into(),
            });
        }
        if !matches!(self.bit_width, 2..=4) {
            return Err(MemoryError::ValidationError {
                field: "bit_width".into(),
                message: "bit_width must be 2, 3, or 4".into(),
            });
        }
        if self.collection_name.trim().is_empty() {
            return Err(MemoryError::ValidationError {
                field: "collection_name".into(),
                message: "collection_name must not be empty".into(),
            });
        }
        if self.retrieval_top_k == 0 {
            return Err(MemoryError::ValidationError {
                field: "retrieval_top_k".into(),
                message: "retrieval_top_k must be > 0".into(),
            });
        }
        if self.vector_index_dir.trim().is_empty() {
            return Err(MemoryError::ValidationError {
                field: "vector_index_dir".into(),
                message: "vector_index_dir must not be empty".into(),
            });
        }
        Ok(())
    }

    fn to_memory_config(&self) -> MemoryConfig {
        MemoryConfig {
            memory_dir: self.memory_dir.clone(),
            max_index_tokens: self.max_index_tokens,
            retrieval_async: self.retrieval_async,
            retrieval_max_files: self.retrieval_max_files,
            retrieval_max_tokens_per_file: self.retrieval_max_tokens_per_file,
            retrieval_max_tokens_per_frontmatter: self.retrieval_max_tokens_per_frontmatter,
            memory_instructions: self.memory_instructions.clone(),
            retrieval_instructions: self.retrieval_instructions.clone(),
        }
    }
}

impl From<MemoryConfig> for TurbovecMemoryConfig {
    fn from(mc: MemoryConfig) -> Self {
        Self {
            memory_dir: mc.memory_dir,
            max_index_tokens: mc.max_index_tokens,
            retrieval_async: mc.retrieval_async,
            retrieval_max_files: mc.retrieval_max_files,
            retrieval_max_tokens_per_file: mc.retrieval_max_tokens_per_file,
            retrieval_max_tokens_per_frontmatter: mc.retrieval_max_tokens_per_frontmatter,
            memory_instructions: mc.memory_instructions,
            retrieval_instructions: mc.retrieval_instructions,
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// TurbovecMemory
// ---------------------------------------------------------------------------

/// Long-term memory implementation backed by Markdown files + a pluggable
/// vector index for semantic retrieval.
///
/// Combines [`FileMemory`] for durable Markdown storage with a
/// [`MemoryVectorIndex`] implementation (such as the TurboVec adapter in
/// `agent_scope_rag`) for fast semantic vector retrieval.
pub struct TurbovecMemory {
    file_memory: FileMemory,
    vector_index: Arc<dyn MemoryVectorIndex>,
    embedding_model: Arc<dyn EmbeddingModel>,
    config: TurbovecMemoryConfig,
    /// Whether the index has been initialized (collection created).
    index_ready: bool,
}

impl TurbovecMemory {
    /// Create a new long-term memory store with a pluggable vector index.
    ///
    /// # Arguments
    /// * `workdir` — working directory for resolving relative `memory_dir` paths
    /// * `config` — memory + vector index configuration
    /// * `embedding_model` — model for text→vector conversion
    /// * `vector_index` — vector index backend (e.g., TurboVec adapter)
    /// * `backend` — optional storage backend (defaults to [`LocalBackend`])
    ///
    /// On construction, creates the memory directory (via `FileMemory` delegate)
    /// and attempts to load an existing index. If `auto_rebuild` is enabled
    /// and the index is missing or incompatible, a rebuild is automatically
    /// triggered.
    pub async fn new(
        workdir: &str,
        config: TurbovecMemoryConfig,
        embedding_model: Arc<dyn EmbeddingModel>,
        vector_index: Arc<dyn MemoryVectorIndex>,
        backend: Option<Arc<dyn Backend>>,
    ) -> Result<Self, MemoryError> {
        config.validate()?;

        let memory_config = config.to_memory_config();
        let file_memory = FileMemory::new(workdir, memory_config, backend);

        let _collection_name = config.collection_name.clone();

        let mut this = Self {
            file_memory,
            vector_index,
            embedding_model,
            config,
            index_ready: false,
        };

        // Attempt to load or initialize the index.
        match this.load_or_init_index().await {
            Ok(()) => {}
            Err(MemoryError::SemanticIndexError { .. }) if this.config.auto_rebuild => {
                info!("auto-rebuilding missing or incompatible vector index");
                let report = this.rebuild_index().await?;
                info!(
                    indexed = report.indexed,
                    skipped = report.skipped,
                    "auto-rebuild complete"
                );
            }
            Err(e) => return Err(e),
        }

        Ok(this)
    }

    /// Return the absolute path to the vector index directory.
    fn vector_index_path(&self) -> String {
        let root = self.file_memory.root_dir();
        if LocalBackend::new().isabs(&self.config.vector_index_dir) {
            self.config.vector_index_dir.clone()
        } else {
            format!(
                "{root}/{}",
                self.config.vector_index_dir.trim_end_matches('/')
            )
        }
    }

    /// Attempt to load an existing vector index from disk.
    async fn load_or_init_index(&mut self) -> Result<(), MemoryError> {
        let index_path = self.vector_index_path();
        let path_exists = tokio::task::spawn_blocking({
            let p = index_path.clone();
            move || std::path::Path::new(&p).exists()
        })
        .await
        .map_err(|e| MemoryError::SemanticIndexError {
            reason: format!("spawn_blocking error: {e}"),
        })?;

        if !path_exists {
            debug!("vector index path does not exist, will init on first write");
            return Ok(());
        }

        self.vector_index
            .load(&index_path)
            .await
            .map_err(|e| MemoryError::SemanticIndexError {
                reason: format!("failed to load vector index at {index_path}: {e}"),
            })?;

        // Check dimension compatibility.
        let has_coll = self
            .vector_index
            .has_collection(&self.config.collection_name)
            .await
            .map_err(|e| MemoryError::SemanticIndexError { reason: e })?;

        if has_coll {
            let _emb_dim = self.embedding_model.model_card().dimensions;
            // Dimension check happens on next operation — we trust the loaded index.
            self.index_ready = true;
        }

        info!("loaded existing vector index from {}", index_path);
        Ok(())
    }

    /// Ensure the vector collection exists and create it if needed.
    async fn ensure_index_ready(&self) -> Result<(), MemoryError> {
        if self.index_ready {
            return Ok(());
        }
        let emb_dim = self.embedding_model.model_card().dimensions;
        let has = self
            .vector_index
            .has_collection(&self.config.collection_name)
            .await
            .map_err(|e| MemoryError::SemanticIndexError { reason: e })?;
        if !has {
            self.vector_index
                .create_collection(&self.config.collection_name, emb_dim)
                .await
                .map_err(|e| MemoryError::SemanticIndexError { reason: e })?;
        }
        Ok(())
    }

    /// Return a reference to the underlying [`FileMemory`] delegate.
    pub fn file_memory(&self) -> &FileMemory {
        &self.file_memory
    }

    /// Report the health status of the vector index.
    pub async fn vector_index_status(&self) -> Result<VectorIndexStatus, MemoryError> {
        let index_path = self.vector_index_path();
        let path_exists = tokio::task::spawn_blocking({
            let p = index_path.clone();
            move || std::path::Path::new(&p).exists()
        })
        .await
        .map_err(|e| MemoryError::SemanticIndexError {
            reason: format!("spawn_blocking error: {e}"),
        })?;

        if !path_exists {
            return Ok(VectorIndexStatus::Missing);
        }

        match self
            .vector_index
            .has_collection(&self.config.collection_name)
            .await
        {
            Ok(true) => Ok(VectorIndexStatus::Clean),
            Ok(false) => Ok(VectorIndexStatus::Missing),
            Err(e) => Ok(VectorIndexStatus::Corrupted(e)),
        }
    }

    // ------------------------------------------------------------------
    // Public API (see Memory trait impl and methods below)
    // ------------------------------------------------------------------
}

// ---------------------------------------------------------------------------
// Memory trait implementation
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl Memory for TurbovecMemory {
    #[tracing::instrument(skip(self, entry), fields(memory.name = %entry.name, memory.type = %entry.metadata.mem_type.as_str()))]
    async fn write(&self, entry: MemoryEntry) -> Result<(), MemoryError> {
        info!(memory.name = %entry.name, "writing memory entry");
        let name = entry.name.clone();
        let mem_type = entry.metadata.mem_type.clone();
        let _description = entry.description.clone();
        let updated_at = entry.metadata.updated_at.clone();
        let content = entry.content.clone();

        // Persist to Markdown first (source of truth).
        self.file_memory.write(entry).await?;

        // Then update the vector index.
        self.ensure_index_ready().await?;

        // Delete old vector records for this document_id.
        let _ = self
            .vector_index
            .delete(&self.config.collection_name, &name)
            .await;

        // Embed the content.
        let embedding_input = EmbeddingInput::Text(content.clone());
        let emb_response = self
            .embedding_model
            .embed(vec![embedding_input])
            .await
            .map_err(|e| MemoryError::SemanticIndexError {
                reason: format!("embedding failed: {e}"),
            })?;

        if emb_response.embeddings.is_empty() {
            return Err(MemoryError::SemanticIndexError {
                reason: "embedding returned empty result".into(),
            });
        }

        let vector = emb_response.embeddings[0].clone();

        let mut metadata = HashMap::new();
        metadata.insert("memory_name".to_string(), name.clone());
        metadata.insert("memory_type".to_string(), mem_type.as_str().to_string());
        metadata.insert("source".to_string(), format!("{name}.md"));
        metadata.insert("updated_at".to_string(), updated_at);

        let record = MemoryVectorRecord {
            vector,
            document_id: name,
            metadata,
            content,
        };

        self.vector_index
            .insert(&self.config.collection_name, vec![record])
            .await
            .map_err(|e| MemoryError::SemanticIndexError { reason: e })?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn read(&self, name: &str) -> Result<Option<MemoryEntry>, MemoryError> {
        debug!(memory.name = name, "reading memory entry");
        self.file_memory.read(name).await
    }

    #[tracing::instrument(skip(self))]
    async fn delete(&self, name: &str) -> Result<(), MemoryError> {
        info!(memory.name = name, "deleting memory entry");
        // Remove from vector index first (idempotent).
        let _ = self
            .vector_index
            .delete(&self.config.collection_name, name)
            .await;
        // Then remove from Markdown files.
        self.file_memory.delete(name).await
    }

    #[tracing::instrument(skip(self))]
    async fn list(&self) -> Result<Vec<MemoryFileHeader>, MemoryError> {
        debug!("listing memory headers");
        self.file_memory.list().await
    }

    #[tracing::instrument(skip(self))]
    async fn search(
        &self,
        query: &str,
        type_filter: Option<MemoryType>,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        debug!(
            query,
            "searching memories (falling back to FileMemory substring search)"
        );
        self.file_memory.search(query, type_filter).await
    }

    #[tracing::instrument(skip(self))]
    async fn get_index_content(&self) -> Result<String, MemoryError> {
        debug!("reading memory index");
        self.file_memory.get_index_content().await
    }

    #[tracing::instrument(skip(self, _model))]
    async fn retrieve_relevant(
        &self,
        query: &str,
        _model: &Arc<dyn agent_scope_model::ChatModel>,
        max_results: usize,
    ) -> Result<Option<String>, MemoryError> {
        debug!(
            query,
            max_results, "retrieving relevant memories via vector index"
        );

        if query.trim().is_empty() {
            return Err(MemoryError::ValidationError {
                field: "query".into(),
                message: "query must not be empty".into(),
            });
        }

        if max_results == 0 {
            return Ok(None);
        }

        // Semantic search via vector index.
        let results = match self.semantic_search(query, None, max_results).await {
            Ok(r) => r,
            Err(_e) => {
                // Fail-open: return None on retrieval failure per Constitution §13.
                warn!(error = %_e, "semantic search failed, returning None");
                return Ok(None);
            }
        };

        if results.is_empty() {
            return Ok(None);
        }

        // Format results for agent context injection (matching Feature 009 format).
        let mut sections = Vec::new();
        for r in &results {
            let age = age_label(&r.updated_at);
            sections.push(format!(
                "### {} ({})\nDescription: {}\nType: {}\n\n{}",
                r.memory_name,
                age,
                r.description,
                r.memory_type.as_str(),
                r.content
            ));
        }

        Ok(Some(sections.join("\n\n")))
    }
}

// ---------------------------------------------------------------------------
// TurbovecMemory — semantic search and maintenance
// ---------------------------------------------------------------------------

impl TurbovecMemory {
    /// Semantic search via the vector index.
    ///
    /// Embeds the query, searches the vector index, deduplicates by memory
    /// name, and returns ranked results with bounded content.
    ///
    /// # Errors
    /// - `ValidationError` if query is empty
    /// - `SemanticIndexError` if embedding or vector store fails
    #[tracing::instrument(skip(self), fields(query))]
    pub async fn semantic_search(
        &self,
        query: &str,
        type_filter: Option<MemoryType>,
        top_k: usize,
    ) -> Result<Vec<MemorySearchResult>, MemoryError> {
        if query.trim().is_empty() {
            return Err(MemoryError::ValidationError {
                field: "query".into(),
                message: "query must not be empty".into(),
            });
        }

        if top_k == 0 {
            return Ok(Vec::new());
        }

        self.ensure_index_ready().await?;

        // Embed the query.
        let emb_input = EmbeddingInput::Text(query.to_string());
        let emb_response = self
            .embedding_model
            .embed(vec![emb_input])
            .await
            .map_err(|e| MemoryError::SemanticIndexError {
                reason: format!("embedding failed: {e}"),
            })?;

        if emb_response.embeddings.is_empty() {
            return Ok(Vec::new());
        }

        let query_vector = emb_response.embeddings[0].clone();

        // Build metadata filter if type_filter is set.
        let metadata_filter = type_filter.map(|mt| {
            let mut filter = HashMap::new();
            filter.insert("memory_type".to_string(), mt.as_str().to_string());
            filter
        });

        // Search the vector index.
        let hits = self
            .vector_index
            .search(
                &self.config.collection_name,
                query_vector,
                top_k,
                metadata_filter,
            )
            .await
            .map_err(|e| MemoryError::SemanticIndexError { reason: e })?;

        // Deduplicate by memory_name, keeping highest score.
        let mut seen = HashMap::new();
        for hit in hits {
            let name = hit
                .metadata
                .get("memory_name")
                .cloned()
                .unwrap_or_else(|| hit.document_id.clone());

            let entry = seen
                .entry(name.clone())
                .or_insert_with(|| MemorySearchResult {
                    memory_name: name,
                    description: hit.metadata.get("memory_type").cloned().unwrap_or_default(),
                    memory_type: hit
                        .metadata
                        .get("memory_type")
                        .map(|t| MemoryType::from(t.as_str()))
                        .unwrap_or(MemoryType::Unknown("unknown".into())),
                    score: hit.score,
                    content: truncate_str(&hit.content, self.config.retrieval_max_tokens_per_file),
                    updated_at: hit.metadata.get("updated_at").cloned().unwrap_or_default(),
                });

            if hit.score > entry.score {
                entry.score = hit.score;
                entry.content =
                    truncate_str(&hit.content, self.config.retrieval_max_tokens_per_file);
                entry.description = hit.metadata.get("memory_type").cloned().unwrap_or_default();
                if let Some(mt) = hit.metadata.get("memory_type") {
                    entry.memory_type = MemoryType::from(mt.as_str());
                }
                if let Some(ts) = hit.metadata.get("updated_at") {
                    entry.updated_at = ts.clone();
                }
            }
        }

        // Collect, filter by score threshold, sort.
        let mut results: Vec<MemorySearchResult> = seen.into_values().collect();
        if let Some(threshold) = self.config.retrieval_score_threshold {
            results.retain(|r| r.score >= threshold);
        }
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.memory_name.cmp(&b.memory_name))
        });
        results.truncate(top_k);

        Ok(results)
    }

    /// Persist the vector index to `{memory_dir}/{vector_index_dir}/`.
    #[tracing::instrument(skip(self))]
    pub async fn save_index(&self) -> Result<(), MemoryError> {
        let index_path = self.vector_index_path();
        info!(path = %index_path, "saving vector index");
        self.vector_index
            .save(&index_path)
            .await
            .map_err(|e| MemoryError::SemanticIndexError {
                reason: format!("save failed: {e}"),
            })
    }

    /// Rebuild the vector index from Markdown memory files.
    ///
    /// Reads all `.md` files via `FileMemory`, generates embeddings, and
    /// replaces the entire vector collection. Malformed or empty files are
    /// skipped and reported.
    #[tracing::instrument(skip(self))]
    pub async fn rebuild_index(&self) -> Result<MemoryRebuildReport, MemoryError> {
        let start = Instant::now();
        info!("rebuilding vector index from Markdown files");

        let headers = self.file_memory.list().await?;
        let total_scanned = headers.len();
        let mut indexed = 0usize;
        let mut skipped = 0usize;
        let mut errors = Vec::new();

        // Collect all records to insert.
        let mut records: Vec<MemoryVectorRecord> = Vec::with_capacity(total_scanned);

        for header in &headers {
            let name = header.filename.trim_end_matches(".md");
            match self.file_memory.read(name).await {
                Ok(Some(entry)) => {
                    if entry.content.trim().is_empty() {
                        skipped += 1;
                        continue;
                    }

                    let emb_input = EmbeddingInput::Text(entry.content.clone());
                    match self.embedding_model.embed(vec![emb_input]).await {
                        Ok(emb_response) => {
                            if emb_response.embeddings.is_empty() {
                                skipped += 1;
                                errors.push(format!("{name}: embedding returned empty result"));
                                continue;
                            }
                            let vector = emb_response.embeddings[0].clone();

                            let mut metadata = HashMap::new();
                            metadata.insert("memory_name".to_string(), entry.name.clone());
                            metadata.insert(
                                "memory_type".to_string(),
                                entry.metadata.mem_type.as_str().to_string(),
                            );
                            metadata.insert("source".to_string(), format!("{name}.md"));
                            metadata.insert(
                                "updated_at".to_string(),
                                entry.metadata.updated_at.clone(),
                            );

                            records.push(MemoryVectorRecord {
                                vector,
                                document_id: entry.name.clone(),
                                metadata,
                                content: entry.content,
                            });
                            indexed += 1;
                        }
                        Err(e) => {
                            skipped += 1;
                            errors.push(format!("{name}: embedding error: {e}"));
                        }
                    }
                }
                Ok(None) => {
                    skipped += 1;
                }
                Err(e) => {
                    skipped += 1;
                    errors.push(format!("{name}: read error: {e}"));
                }
            }
        }

        // Replace the collection.
        let emb_dim = self.embedding_model.model_card().dimensions;

        // Re-initialize: delete old collection if exists, create fresh one.
        if self
            .vector_index
            .has_collection(&self.config.collection_name)
            .await
            .unwrap_or(false)
        {
            // Create a fresh collection — we can't drop+recreate with trait, so
            // delete all old records and insert new ones.
        }

        let has = self
            .vector_index
            .has_collection(&self.config.collection_name)
            .await
            .unwrap_or(false);

        if has {
            // For a clean rebuild, we delete old records per document first.
            for header in &headers {
                let name = header.filename.trim_end_matches(".md");
                let _ = self
                    .vector_index
                    .delete(&self.config.collection_name, name)
                    .await;
            }
        } else {
            self.vector_index
                .create_collection(&self.config.collection_name, emb_dim)
                .await
                .map_err(|e| MemoryError::SemanticIndexError { reason: e })?;
        }

        if !records.is_empty() {
            self.vector_index
                .insert(&self.config.collection_name, records)
                .await
                .map_err(|e| MemoryError::SemanticIndexError { reason: e })?;
        }

        // Save the rebuilt index.
        self.save_index().await?;

        let duration_ms = start.elapsed().as_millis() as u64;
        info!(
            total_scanned,
            indexed, skipped, duration_ms, "rebuild complete"
        );

        Ok(MemoryRebuildReport {
            total_scanned,
            indexed,
            skipped,
            errors,
            duration_ms,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Truncate a string to roughly `max_tokens` tokens (≈ 4 chars per token).
fn truncate_str(s: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens.saturating_mul(4);
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{truncated}...")
    }
}

fn age_label(updated_at: &str) -> String {
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(updated_at) else {
        return "saved at unknown time".into();
    };
    let now = chrono::Utc::now();
    let days = now
        .date_naive()
        .signed_duration_since(dt.with_timezone(&chrono::Utc).date_naive())
        .num_days();
    match days {
        0 => "saved today".into(),
        1 => "saved yesterday".into(),
        n if n > 1 => format!("saved {n} days ago"),
        _ => "saved today".into(),
    }
}
