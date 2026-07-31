//! Mock VectorStore implementation and trait behavior tests.
//!
//! Tests the VectorStore trait contract using an in-memory mock implementation.

use std::collections::HashMap;
use std::sync::RwLock;

use agent_scope_rag::chunker::Chunk;
use agent_scope_rag::error::VectorStoreError;
use agent_scope_rag::vector_store::{
    DocumentSummary, VectorRecord, VectorSearchResult, VectorStore,
};

/// In-memory mock VectorStore for testing.
struct MockVectorStore {
    collections: RwLock<HashMap<String, (u32, Vec<VectorRecord>)>>,
}

impl MockVectorStore {
    fn new() -> Self {
        Self {
            collections: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl VectorStore for MockVectorStore {
    async fn has_collection(&self, name: &str) -> Result<bool, VectorStoreError> {
        let guard = self
            .collections
            .read()
            .map_err(|e| VectorStoreError::BackendError(format!("lock error: {e}")))?;
        Ok(guard.contains_key(name))
    }

    async fn create_collection(&self, name: &str, dimensions: u32) -> Result<(), VectorStoreError> {
        let mut guard = self
            .collections
            .write()
            .map_err(|e| VectorStoreError::BackendError(format!("lock error: {e}")))?;
        if let Some((existing_dim, _)) = guard.get(name) {
            if *existing_dim != dimensions {
                return Err(VectorStoreError::DimensionMismatch {
                    expected: *existing_dim,
                    got: dimensions as usize,
                });
            }
            return Ok(());
        }
        guard.insert(name.to_string(), (dimensions, vec![]));
        Ok(())
    }

    async fn search(
        &self,
        collection: &str,
        query_vector: Vec<f32>,
        top_k: usize,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<VectorSearchResult>, VectorStoreError> {
        let guard = self
            .collections
            .read()
            .map_err(|e| VectorStoreError::BackendError(format!("lock error: {e}")))?;
        let (_dim, records) = guard
            .get(collection)
            .ok_or_else(|| VectorStoreError::CollectionNotFound(collection.to_string()))?;

        // Compute cosine similarity
        let norm_q = dot(&query_vector, &query_vector).sqrt();
        let mut scored: Vec<(f32, &VectorRecord)> = records
            .iter()
            .filter(|r| {
                if let Some(ref filter) = metadata_filter {
                    for (k, v) in filter {
                        if r.chunk.metadata.get(k) != Some(v) {
                            return false;
                        }
                    }
                }
                true
            })
            .map(|r| {
                let norm_r = dot(&r.vector, &r.vector).sqrt();
                let sim = if norm_q > 0.0 && norm_r > 0.0 {
                    dot(&query_vector, &r.vector) / (norm_q * norm_r)
                } else {
                    0.0
                };
                (sim, r)
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        if top_k > 0 && top_k < scored.len() {
            scored.truncate(top_k);
        }

        Ok(scored
            .into_iter()
            .map(|(score, record)| VectorSearchResult {
                score,
                document_id: record.document_id.clone(),
                chunk: record.chunk.clone(),
            })
            .collect())
    }

    async fn insert(
        &self,
        collection: &str,
        records: Vec<VectorRecord>,
    ) -> Result<(), VectorStoreError> {
        if records.is_empty() {
            return Ok(());
        }
        let mut guard = self
            .collections
            .write()
            .map_err(|e| VectorStoreError::BackendError(format!("lock error: {e}")))?;
        let (_dim, existing) = guard
            .get_mut(collection)
            .ok_or_else(|| VectorStoreError::CollectionNotFound(collection.to_string()))?;
        existing.extend(records);
        Ok(())
    }

    async fn delete(&self, collection: &str, document_id: &str) -> Result<(), VectorStoreError> {
        let mut guard = self
            .collections
            .write()
            .map_err(|e| VectorStoreError::BackendError(format!("lock error: {e}")))?;
        let (_dim, records) = guard
            .get_mut(collection)
            .ok_or_else(|| VectorStoreError::CollectionNotFound(collection.to_string()))?;
        records.retain(|r| r.document_id != document_id);
        Ok(())
    }

    async fn list_documents(
        &self,
        collection: &str,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<DocumentSummary>, VectorStoreError> {
        let guard = self
            .collections
            .read()
            .map_err(|e| VectorStoreError::BackendError(format!("lock error: {e}")))?;
        let (_dim, records) = guard
            .get(collection)
            .ok_or_else(|| VectorStoreError::CollectionNotFound(collection.to_string()))?;

        let mut summaries: HashMap<String, DocumentSummary> = HashMap::new();
        for record in records {
            // Apply metadata filter
            if let Some(ref filter) = metadata_filter {
                let matches = filter
                    .iter()
                    .all(|(k, v)| record.chunk.metadata.get(k) == Some(v));
                if !matches {
                    continue;
                }
            }

            let entry = summaries
                .entry(record.document_id.clone())
                .or_insert_with(|| DocumentSummary {
                    document_id: record.document_id.clone(),
                    source: record.chunk.source.clone(),
                    chunk_count: 0,
                    metadata: record.chunk.metadata.clone(),
                });
            entry.chunk_count += 1;
        }

        Ok(summaries.into_values().collect())
    }
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn make_chunk(content: &str, source: &str, idx: usize, total: usize) -> Chunk {
    Chunk {
        content: content.to_string(),
        source: source.to_string(),
        chunk_index: idx,
        total_chunks: total,
        metadata: HashMap::new(),
    }
}

#[tokio::test]
async fn test_has_collection() {
    let store = MockVectorStore::new();
    assert!(!store.has_collection("test").await.expect("should succeed"));
    store.create_collection("test", 4).await.expect("create");
    assert!(store.has_collection("test").await.expect("should succeed"));
}

#[tokio::test]
async fn test_create_collection_idempotent() {
    let store = MockVectorStore::new();
    store.create_collection("coll", 4).await.expect("create");
    // Idempotent: same dims
    store
        .create_collection("coll", 4)
        .await
        .expect("create again");
    // Different dims → error
    let result = store.create_collection("coll", 8).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_insert_and_search() {
    let store = MockVectorStore::new();
    store.create_collection("coll", 2).await.expect("create");

    let chunk = make_chunk("hello", "doc.txt", 0, 1);
    store
        .insert(
            "coll",
            vec![VectorRecord {
                vector: vec![1.0, 0.0],
                document_id: "doc1".into(),
                chunk,
            }],
        )
        .await
        .expect("insert");

    let results = store
        .search("coll", vec![1.0, 0.1], 5, None)
        .await
        .expect("search");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].document_id, "doc1");
    assert!(results[0].score > 0.9);
}

#[tokio::test]
async fn test_delete_then_empty_search() {
    let store = MockVectorStore::new();
    store.create_collection("coll", 2).await.expect("create");

    let chunk = make_chunk("hello", "doc.txt", 0, 1);
    store
        .insert(
            "coll",
            vec![VectorRecord {
                vector: vec![1.0, 0.0],
                document_id: "doc1".into(),
                chunk,
            }],
        )
        .await
        .expect("insert");

    store.delete("coll", "doc1").await.expect("delete");
    let results = store
        .search("coll", vec![1.0, 0.0], 5, None)
        .await
        .expect("search");
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_delete_nonexistent_idempotent() {
    let store = MockVectorStore::new();
    store.create_collection("coll", 2).await.expect("create");
    // Should not error
    store
        .delete("coll", "nonexistent")
        .await
        .expect("delete ok");
}

#[tokio::test]
async fn test_list_documents() {
    let store = MockVectorStore::new();
    store.create_collection("coll", 2).await.expect("create");

    store
        .insert(
            "coll",
            vec![
                VectorRecord {
                    vector: vec![1.0, 0.0],
                    document_id: "doc1".into(),
                    chunk: make_chunk("a", "f1.txt", 0, 2),
                },
                VectorRecord {
                    vector: vec![0.0, 1.0],
                    document_id: "doc1".into(),
                    chunk: make_chunk("b", "f1.txt", 1, 2),
                },
            ],
        )
        .await
        .expect("insert");

    let docs = store
        .list_documents("coll", None)
        .await
        .expect("list_documents");
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].document_id, "doc1");
    assert_eq!(docs[0].chunk_count, 2);
}
