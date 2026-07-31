//! Knowledge base — runtime wrapper combining embedding + vector store.
//!
//! Provides search/insert/delete/list operations with lazy collection initialization.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use agent_scope_embedding::{EmbeddingInput, EmbeddingModel, EmbeddingModelCard};
use tokio::sync::OnceCell;

use crate::chunker::Chunk;
use crate::error::KnowledgeBaseError;
use crate::vector_store::{DocumentSummary, VectorRecord, VectorSearchResult, VectorStore};

// ---------------------------------------------------------------------------
// KnowledgeBase
// ---------------------------------------------------------------------------

/// Runtime knowledge base handle.
///
/// Wraps an embedding model + vector store + collection into a simple
/// search/insert/delete/list API.
///
/// # Lazy Initialization
///
/// The underlying vector store collection is created lazily on the first
/// operation (search, insert, delete, or list). This uses [`OnceCell`]
/// to guarantee thread-safe one-time initialization.
pub struct KnowledgeBase {
    /// Human-readable name (used as Tool name in agentic mode).
    pub name: String,
    /// Description for Agent context (e.g., "Company HR policies").
    pub description: String,
    /// Embedding model for query/document vectorization.
    embedding_model: Arc<dyn EmbeddingModel>,
    /// Vector store backend.
    vector_store: Arc<dyn VectorStore>,
    /// Collection name in the backing store.
    collection: String,
    /// Mandatory metadata filter — all operations are scoped to this filter.
    metadata_filter: Option<HashMap<String, String>>,
    /// Lazy init state.
    initialized: OnceCell<()>,
}

impl KnowledgeBase {
    /// Create a new knowledge base.
    ///
    /// The underlying collection is NOT created at construction time.
    /// It will be lazily created on the first operation.
    pub fn new(
        name: String,
        description: String,
        embedding_model: Arc<dyn EmbeddingModel>,
        vector_store: Arc<dyn VectorStore>,
        collection: String,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Self {
        Self {
            name,
            description,
            embedding_model,
            vector_store,
            collection,
            metadata_filter,
            initialized: OnceCell::new(),
        }
    }

    /// Return the embedding model card.
    pub fn model_card(&self) -> &EmbeddingModelCard {
        self.embedding_model.model_card()
    }

    // ── Lazy initialization ─────────────────────────────────────────

    /// Ensure the vector store collection exists.
    ///
    /// Called automatically on the first operation. Thread-safe via `OnceCell`.
    async fn ensure_initialized(&self) -> Result<(), KnowledgeBaseError> {
        self.initialized
            .get_or_try_init(|| async {
                let exists = self
                    .vector_store
                    .has_collection(&self.collection)
                    .await
                    .map_err(|e| KnowledgeBaseError::VectorStoreError(e.to_string()))?;

                if !exists {
                    let dims = self.embedding_model.model_card().dimensions;
                    self.vector_store
                        .create_collection(&self.collection, dims)
                        .await
                        .map_err(|e| KnowledgeBaseError::VectorStoreError(e.to_string()))?;
                }
                Ok(())
            })
            .await
            .copied()
    }

    // ── Search ──────────────────────────────────────────────────────

    /// Search the knowledge base with one or more queries.
    ///
    /// Results are deduplicated by `(document_id, chunk_index)` and
    /// sorted by score descending.
    pub async fn search(
        &self,
        queries: Vec<EmbeddingInput>,
        top_k: usize,
        score_threshold: Option<f32>,
    ) -> Result<Vec<VectorSearchResult>, KnowledgeBaseError> {
        self.ensure_initialized().await?;

        if queries.is_empty() {
            return Ok(vec![]);
        }

        // Embed all queries
        let response = self
            .embedding_model
            .embed(queries)
            .await
            .map_err(|e| KnowledgeBaseError::EmbeddingError(e.to_string()))?;

        // Search per query vector concurrently
        let mut futures = Vec::new();
        for vec in response.embeddings {
            let vs = Arc::clone(&self.vector_store);
            let collection = self.collection.clone();
            let filter = self.metadata_filter.clone();
            futures.push(tokio::spawn(async move {
                vs.search(&collection, vec, top_k.max(1), filter).await
            }));
        }

        let mut all_results: Vec<VectorSearchResult> = Vec::new();
        let mut seen: HashSet<(String, usize)> = HashSet::new();

        for handle in futures {
            match handle.await {
                Ok(Ok(results)) => {
                    for r in results {
                        let key = (r.document_id.clone(), r.chunk.chunk_index);
                        if seen.contains(&key) {
                            // Keep the one with higher score — since we process serially
                            // and results come pre-sorted, first wins
                            continue;
                        }
                        seen.insert(key);
                        all_results.push(r);
                    }
                }
                Ok(Err(e)) => {
                    return Err(KnowledgeBaseError::VectorStoreError(e.to_string()));
                }
                Err(e) => {
                    return Err(KnowledgeBaseError::VectorStoreError(format!(
                        "join error: {e}"
                    )));
                }
            }
        }

        // Sort by score descending
        all_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply score threshold
        if let Some(threshold) = score_threshold {
            all_results.retain(|r| r.score >= threshold);
        }

        // Apply top_k truncation (per-query top_k was already applied, this is cross-query)
        if top_k > 0 && all_results.len() > top_k {
            all_results.truncate(top_k);
        }

        Ok(all_results)
    }

    // ── Insert ──────────────────────────────────────────────────────

    /// Insert chunks into the knowledge base.
    ///
    /// Returns the document ID (auto-generated UUID v4 if not provided).
    pub async fn insert_document(
        &self,
        chunks: Vec<Chunk>,
        document_id: Option<String>,
        document_metadata: Option<HashMap<String, String>>,
    ) -> Result<String, KnowledgeBaseError> {
        self.ensure_initialized().await?;

        if chunks.is_empty() {
            return Ok(String::new());
        }

        let doc_id = document_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // Apply metadata merge: document_metadata < chunk.metadata < metadata_filter
        let mut merged_chunks: Vec<Chunk> = Vec::new();
        for mut chunk in chunks {
            let mut meta = document_metadata.clone().unwrap_or_default();
            meta.extend(chunk.metadata.clone());
            if let Some(ref filter) = self.metadata_filter {
                meta.extend(filter.clone());
            }
            chunk.metadata = meta;
            merged_chunks.push(chunk);
        }

        // Extract text for embedding
        let inputs: Vec<EmbeddingInput> = merged_chunks
            .iter()
            .map(|c| EmbeddingInput::Text(c.content.clone()))
            .collect();

        let response = self
            .embedding_model
            .embed(inputs)
            .await
            .map_err(|e| KnowledgeBaseError::EmbeddingError(e.to_string()))?;

        // Verify count match
        if response.embeddings.len() != merged_chunks.len() {
            return Err(KnowledgeBaseError::CountMismatch {
                expected: merged_chunks.len(),
                got: response.embeddings.len(),
            });
        }

        // Build records
        let records: Vec<VectorRecord> = merged_chunks
            .into_iter()
            .zip(response.embeddings.into_iter())
            .map(|(chunk, vector)| VectorRecord {
                vector,
                document_id: doc_id.clone(),
                chunk,
            })
            .collect();

        self.vector_store
            .insert(&self.collection, records)
            .await
            .map_err(|e| KnowledgeBaseError::VectorStoreError(e.to_string()))?;

        Ok(doc_id)
    }

    // ── Delete ──────────────────────────────────────────────────────

    /// Remove a document and all its chunks from the knowledge base.
    ///
    /// Idempotent — if the document doesn't exist, still returns `Ok(())`.
    pub async fn delete_document(&self, document_id: &str) -> Result<(), KnowledgeBaseError> {
        self.ensure_initialized().await?;
        self.vector_store
            .delete(&self.collection, document_id)
            .await
            .map_err(|e| KnowledgeBaseError::VectorStoreError(e.to_string()))
    }

    // ── List ────────────────────────────────────────────────────────

    /// List all documents in this knowledge base.
    pub async fn list_documents(&self) -> Result<Vec<DocumentSummary>, KnowledgeBaseError> {
        self.ensure_initialized().await?;
        self.vector_store
            .list_documents(&self.collection, self.metadata_filter.clone())
            .await
            .map_err(|e| KnowledgeBaseError::VectorStoreError(e.to_string()))
    }
}
