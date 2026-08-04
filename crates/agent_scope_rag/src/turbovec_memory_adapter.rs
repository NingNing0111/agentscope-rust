//! Adapter that implements [`agent_scope_memory::MemoryVectorIndex`] using
//! [`TurbovecVectorStore`](crate::turbovec_store::TurbovecVectorStore).
//!
//! This bridges the gap between the `agent_scope_memory` crate (which defines
//! the abstract vector-index trait) and `agent_scope_rag` (which provides the
//! concrete TurboVec-backed implementation), without introducing a circular
//! dependency.

use std::collections::HashMap;
use std::sync::Arc;

use agent_scope_memory::{MemoryVectorHit, MemoryVectorIndex, MemoryVectorRecord};
use tokio::sync::RwLock;

use crate::chunker::Chunk;
use crate::turbovec_store::TurbovecVectorStore;
use crate::vector_store::{VectorRecord, VectorSearchResult, VectorStore};

/// A `MemoryVectorIndex` implementation backed by [`TurbovecVectorStore`].
///
/// # Examples
///
/// ```rust,no_run
/// use agent_scope_rag::turbovec_memory_adapter::TurbovecIndexAdapter;
/// use agent_scope_memory::MemoryVectorIndex;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let adapter = TurbovecIndexAdapter::new(4)?;
/// // Pass to TurbovecMemory::new() as Arc<dyn MemoryVectorIndex>
/// # Ok(())
/// # }
/// ```
pub struct TurbovecIndexAdapter {
    /// The underlying store. A `RwLock<Arc<...>>` so `load` can atomically
    /// swap in a store freshly loaded from disk (previously `load` read the
    /// file and then silently discarded it, making restarts lose the index).
    store: RwLock<Arc<TurbovecVectorStore>>,
    /// Track save/load state.
    saved_path: RwLock<Option<String>>,
}

impl TurbovecIndexAdapter {
    /// Create a new adapter wrapping an empty turbovec store.
    ///
    /// `bit_width` must be 2, 3, or 4.
    pub fn new(bit_width: usize) -> Result<Self, String> {
        let store = Arc::new(TurbovecVectorStore::new(bit_width).map_err(|e| e.to_string())?);
        Ok(Self {
            store: RwLock::new(store),
            saved_path: RwLock::new(None),
        })
    }

    /// Create an adapter from an existing [`TurbovecVectorStore`].
    pub fn from_store(store: Arc<TurbovecVectorStore>) -> Self {
        Self {
            store: RwLock::new(store),
            saved_path: RwLock::new(None),
        }
    }

    /// Get a clone of the underlying store.
    pub async fn store(&self) -> Arc<TurbovecVectorStore> {
        self.store.read().await.clone()
    }
}

#[async_trait::async_trait]
impl MemoryVectorIndex for TurbovecIndexAdapter {
    async fn has_collection(&self, name: &str) -> Result<bool, String> {
        self.store
            .read()
            .await
            .has_collection(name)
            .await
            .map_err(|e| e.to_string())
    }

    async fn collection_dimension(&self, name: &str) -> Result<Option<u32>, String> {
        self.store
            .read()
            .await
            .collection_dimension(name)
            .await
            .map_err(|e| e.to_string())
    }

    async fn create_collection(&self, name: &str, dimensions: u32) -> Result<(), String> {
        self.store
            .read()
            .await
            .create_collection(name, dimensions)
            .await
            .map_err(|e| e.to_string())
    }

    async fn search(
        &self,
        collection: &str,
        query_vector: Vec<f32>,
        top_k: usize,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<MemoryVectorHit>, String> {
        let results: Vec<VectorSearchResult> = self
            .store
            .read()
            .await
            .search(collection, query_vector, top_k, metadata_filter)
            .await
            .map_err(|e| e.to_string())?;

        Ok(results
            .into_iter()
            .map(|r| MemoryVectorHit {
                score: r.score,
                document_id: r.document_id,
                metadata: r.chunk.metadata,
                content: r.chunk.content,
            })
            .collect())
    }

    async fn insert(
        &self,
        collection: &str,
        records: Vec<MemoryVectorRecord>,
    ) -> Result<(), String> {
        if records.is_empty() {
            return Ok(());
        }

        let vr: Vec<VectorRecord> = records
            .into_iter()
            .map(|r| {
                let chunk = Chunk {
                    content: r.content,
                    source: r
                        .metadata
                        .get("source")
                        .cloned()
                        .unwrap_or_else(|| format!("{}.md", r.document_id)),
                    chunk_index: 0,
                    total_chunks: 1,
                    metadata: r.metadata,
                };
                VectorRecord {
                    vector: r.vector,
                    document_id: r.document_id,
                    chunk,
                }
            })
            .collect();

        self.store
            .read()
            .await
            .insert(collection, vr)
            .await
            .map_err(|e| e.to_string())
    }

    async fn delete(&self, collection: &str, document_id: &str) -> Result<(), String> {
        self.store
            .read()
            .await
            .delete(collection, document_id)
            .await
            .map_err(|e| e.to_string())
    }

    async fn list_documents(&self, collection: &str) -> Result<Vec<String>, String> {
        self.store
            .read()
            .await
            .list_documents(collection, None)
            .await
            .map(|summaries| {
                summaries
                    .into_iter()
                    .map(|s| s.document_id)
                    .collect::<Vec<_>>()
            })
            .map_err(|e| e.to_string())
    }

    async fn save(&self, path: &str) -> Result<(), String> {
        self.store
            .read()
            .await
            .save(path)
            .await
            .map_err(|e| e.to_string())?;
        *self.saved_path.write().await = Some(path.to_string());
        Ok(())
    }

    async fn load(&self, path: &str) -> Result<(), String> {
        let loaded = TurbovecVectorStore::load(path)
            .await
            .map_err(|e| e.to_string())?;
        // Swap the freshly-loaded store into place so subsequent operations
        // (has_collection, search, insert) see the persisted index instead of
        // silently discarding it and rebuilding an empty one.
        *self.store.write().await = Arc::new(loaded);
        *self.saved_path.write().await = Some(path.to_string());
        Ok(())
    }
}

/// Load a [`TurbovecIndexAdapter`] from a previously saved store.
pub async fn load_turbovec_adapter(path: &str) -> Result<TurbovecIndexAdapter, String> {
    let store = TurbovecVectorStore::load(path)
        .await
        .map_err(|e| e.to_string())?;
    Ok(TurbovecIndexAdapter::from_store(Arc::new(store)))
}
