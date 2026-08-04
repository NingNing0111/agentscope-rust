//! Vector store trait and associated data types.
//!
//! Defines the [`VectorStore`] async trait — the abstract contract
//! for vector database backends (Qdrant, Milvus, MongoDB, etc.).
//!
//! No concrete implementations are provided in this crate.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::chunker::Chunk;
use crate::error::VectorStoreError;

// ---------------------------------------------------------------------------
// VectorRecord
// ---------------------------------------------------------------------------

/// A single record to store in the vector database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRecord {
    /// The embedding vector.
    pub vector: Vec<f32>,
    /// Owning document identifier.
    pub document_id: String,
    /// The original chunk (content + metadata).
    pub chunk: Chunk,
}

// ---------------------------------------------------------------------------
// VectorSearchResult
// ---------------------------------------------------------------------------

/// A search result from a vector store query.
///
/// Results are sorted by `score` descending (higher = more similar).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    /// Similarity score (higher = more similar).
    pub score: f32,
    /// Owning document identifier.
    pub document_id: String,
    /// The matched chunk.
    pub chunk: Chunk,
}

// ---------------------------------------------------------------------------
// DocumentSummary
// ---------------------------------------------------------------------------

/// Summary metadata for a document stored in a vector collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    /// Unique document identifier.
    pub document_id: String,
    /// Original source filename.
    pub source: String,
    /// Number of chunks in the store for this document.
    pub chunk_count: usize,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// VectorStore trait
// ---------------------------------------------------------------------------

/// Abstract trait for vector database backends.
///
/// Semantically equivalent to Python AgentScope `VectorStoreBase`.
///
/// No concrete implementation is provided in this crate — downstream crates
/// implement this trait for specific backends.
///
/// # Contract
///
/// - `has_collection(name)` — returns `true` if collection exists, case-sensitive
/// - `create_collection(name, dimensions)` — idempotent when dimensions match
/// - `search(...)` — results sorted by score descending, metadata_filter does exact match
/// - `insert(...)` — empty records is a no-op
/// - `delete(collection, document_id)` — idempotent (document-not-found is OK)
/// - `list_documents(...)` — returns distinct document summaries, filtered by metadata
#[async_trait::async_trait]
pub trait VectorStore: Send + Sync {
    /// Check if a collection exists.
    async fn has_collection(&self, name: &str) -> Result<bool, VectorStoreError>;

    /// Return the stored vector dimension of a collection, or `Ok(None)` if
    /// the collection does not exist (round-4 M38).
    async fn collection_dimension(&self, name: &str) -> Result<Option<u32>, VectorStoreError>;

    /// Create a collection with the given vector dimensions.
    async fn create_collection(&self, name: &str, dimensions: u32) -> Result<(), VectorStoreError>;

    /// Search for similar vectors.
    ///
    /// # Arguments
    /// * `collection` — collection name
    /// * `query_vector` — query embedding vector
    /// * `top_k` — maximum number of results to return
    /// * `metadata_filter` — optional exact-match filter on metadata fields
    ///
    /// # Returns
    /// Results sorted by score descending.
    async fn search(
        &self,
        collection: &str,
        query_vector: Vec<f32>,
        top_k: usize,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<VectorSearchResult>, VectorStoreError>;

    /// Insert vector records into a collection.
    ///
    /// Empty `records` is a no-op and returns `Ok(())`.
    async fn insert(
        &self,
        collection: &str,
        records: Vec<VectorRecord>,
    ) -> Result<(), VectorStoreError>;

    /// Delete all records for a document.
    ///
    /// Idempotent — if the document doesn't exist, still returns `Ok(())`.
    async fn delete(&self, collection: &str, document_id: &str) -> Result<(), VectorStoreError>;

    /// List distinct documents in a collection.
    ///
    /// # Arguments
    /// * `collection` — collection name
    /// * `metadata_filter` — optional exact-match filter
    async fn list_documents(
        &self,
        collection: &str,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<DocumentSummary>, VectorStoreError>;
}
