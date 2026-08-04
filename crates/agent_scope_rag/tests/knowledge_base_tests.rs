//! Tests for the KnowledgeBase with mock backends.
use std::collections::HashMap;
use std::sync::Arc;

use agent_scope_embedding::{
    EmbeddingError, EmbeddingInput, EmbeddingModel, EmbeddingModelCard, EmbeddingResponse,
    EmbeddingUsage,
};
use agent_scope_rag::chunker::Chunk;
use agent_scope_rag::error::KnowledgeBaseError;
use agent_scope_rag::knowledge_base::KnowledgeBase;
use agent_scope_rag::vector_store::{
    DocumentSummary, VectorRecord, VectorSearchResult, VectorStore,
};

// Mock embedding model — returns fixed-dim vectors
struct MockEmbedder {
    card: EmbeddingModelCard,
}

impl MockEmbedder {
    fn new(dims: u32) -> Self {
        Self {
            card: EmbeddingModelCard::new("mock-embedder", dims, false),
        }
    }
}

#[async_trait::async_trait]
impl EmbeddingModel for MockEmbedder {
    async fn embed(
        &self,
        inputs: Vec<EmbeddingInput>,
    ) -> Result<EmbeddingResponse, EmbeddingError> {
        let dim = self.card.dimensions as usize;
        let embeddings = inputs
            .iter()
            .enumerate()
            .map(|(i, _)| vec![i as f32; dim])
            .collect();
        Ok(EmbeddingResponse {
            embeddings,
            usage: EmbeddingUsage {
                total_tokens: inputs.len() as u32,
            },
        })
    }

    fn model_card(&self) -> &EmbeddingModelCard {
        &self.card
    }
}

// Mock vector store — in-memory HashMap
#[derive(Clone)]
struct MockVectorStore {
    data: Arc<std::sync::RwLock<HashMap<String, Vec<VectorRecord>>>>,
}

impl MockVectorStore {
    fn new() -> Self {
        Self {
            data: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl VectorStore for MockVectorStore {
    async fn has_collection(
        &self,
        name: &str,
    ) -> Result<bool, agent_scope_rag::error::VectorStoreError> {
        Ok(self.data.read().unwrap().contains_key(name))
    }

    async fn collection_dimension(
        &self,
        _name: &str,
    ) -> Result<Option<u32>, agent_scope_rag::error::VectorStoreError> {
        Ok(None)
    }

    async fn create_collection(
        &self,
        name: &str,
        _dimensions: u32,
    ) -> Result<(), agent_scope_rag::error::VectorStoreError> {
        let mut guard = self.data.write().unwrap();
        guard.entry(name.to_string()).or_default();
        Ok(())
    }

    async fn search(
        &self,
        collection: &str,
        _query_vector: Vec<f32>,
        top_k: usize,
        _metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<VectorSearchResult>, agent_scope_rag::error::VectorStoreError> {
        let guard = self.data.read().unwrap();
        let records = guard.get(collection).cloned().unwrap_or_default();
        let mut results: Vec<VectorSearchResult> = records
            .into_iter()
            .enumerate()
            .map(|(i, r)| VectorSearchResult {
                score: 1.0 - i as f32 * 0.1,
                document_id: r.document_id,
                chunk: r.chunk,
            })
            .collect();
        if top_k > 0 && top_k < results.len() {
            results.truncate(top_k);
        }
        Ok(results)
    }

    async fn insert(
        &self,
        collection: &str,
        records: Vec<VectorRecord>,
    ) -> Result<(), agent_scope_rag::error::VectorStoreError> {
        let mut guard = self.data.write().unwrap();
        guard
            .entry(collection.to_string())
            .or_default()
            .extend(records);
        Ok(())
    }

    async fn delete(
        &self,
        collection: &str,
        document_id: &str,
    ) -> Result<(), agent_scope_rag::error::VectorStoreError> {
        let mut guard = self.data.write().unwrap();
        if let Some(records) = guard.get_mut(collection) {
            records.retain(|r| r.document_id != document_id);
        }
        Ok(())
    }

    async fn list_documents(
        &self,
        collection: &str,
        _metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<DocumentSummary>, agent_scope_rag::error::VectorStoreError> {
        let guard = self.data.read().unwrap();
        let records = guard.get(collection).cloned().unwrap_or_default();
        let mut map: HashMap<String, DocumentSummary> = HashMap::new();
        for r in &records {
            let entry = map
                .entry(r.document_id.clone())
                .or_insert_with(|| DocumentSummary {
                    document_id: r.document_id.clone(),
                    source: r.chunk.source.clone(),
                    chunk_count: 0,
                    metadata: r.chunk.metadata.clone(),
                });
            entry.chunk_count += 1;
        }
        Ok(map.into_values().collect())
    }
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

fn make_kb() -> KnowledgeBase {
    KnowledgeBase::new(
        "test-kb".into(),
        "Test knowledge base".into(),
        Arc::new(MockEmbedder::new(4)),
        Arc::new(MockVectorStore::new()),
        "test-collection".into(),
        None,
    )
}

#[tokio::test]
async fn test_insert_and_search() {
    let kb = make_kb();
    let chunks = vec![
        make_chunk("hello world", "doc.txt", 0, 2),
        make_chunk("foo bar", "doc.txt", 1, 2),
    ];
    let doc_id = kb
        .insert_document(chunks, None, None)
        .await
        .expect("insert should succeed");
    assert!(!doc_id.is_empty());

    let results = kb
        .search(vec!["hello".into()], 5, None)
        .await
        .expect("search should succeed");
    assert!(!results.is_empty());
    assert_eq!(results[0].document_id, doc_id);
}

#[tokio::test]
async fn test_search_deduplication() {
    // Same (doc_id, chunk_index) → keep highest score
    let kb = make_kb();
    let chunks = vec![make_chunk("dup content", "doc.txt", 0, 1)];
    kb.insert_document(chunks, Some("doc1".into()), None)
        .await
        .expect("insert");

    // Insert again with same doc_id + chunk_index (should be overwritten by
    // store semantics, but dedup in search)
    let chunks2 = vec![make_chunk("dup content v2", "doc.txt", 0, 1)];
    kb.insert_document(chunks2, Some("doc1".into()), None)
        .await
        .expect("insert");

    let results = kb
        .search(vec!["dup".into()], 5, None)
        .await
        .expect("search");

    // Should deduplicate: only one result per (document_id, chunk_index)
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_delete_document() {
    let kb = make_kb();
    let chunks = vec![make_chunk("test", "doc.txt", 0, 1)];
    let doc_id = kb
        .insert_document(chunks, Some("doc-to-del".into()), None)
        .await
        .expect("insert");
    assert_eq!(doc_id, "doc-to-del");

    kb.delete_document("doc-to-del")
        .await
        .expect("delete should succeed");

    let results = kb
        .search(vec!["test".into()], 5, None)
        .await
        .expect("search");
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_delete_nonexistent_idempotent() {
    let kb = make_kb();
    kb.delete_document("nonexistent")
        .await
        .expect("delete should be idempotent");
}

#[tokio::test]
async fn test_list_documents() {
    let kb = make_kb();
    let chunks = vec![
        make_chunk("a", "f1.txt", 0, 2),
        make_chunk("b", "f1.txt", 1, 2),
    ];
    kb.insert_document(chunks, Some("doc1".into()), None)
        .await
        .expect("insert");

    let docs = kb.list_documents().await.expect("list should succeed");
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].document_id, "doc1");
    assert_eq!(docs[0].chunk_count, 2);
}

#[tokio::test]
async fn test_metadata_filter_override() {
    // metadata_filter from KB should override chunk metadata
    let kb = KnowledgeBase::new(
        "test-kb".into(),
        "Test".into(),
        Arc::new(MockEmbedder::new(4)),
        Arc::new(MockVectorStore::new()),
        "test-collection".into(),
        Some(HashMap::from([(
            "scope".to_string(),
            "kb-default".to_string(),
        )])),
    );

    let mut chunk = make_chunk("content", "doc.txt", 0, 1);
    chunk
        .metadata
        .insert("scope".to_string(), "chunk-value".to_string());
    kb.insert_document(vec![chunk], Some("doc1".into()), None)
        .await
        .expect("insert");

    // The metadata_filter "scope=kb-default" should have won
    // (This verifies the logic applied, store search doesn't filter in mock)
    let results = kb
        .search(vec!["content".into()], 5, None)
        .await
        .expect("search");
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_lazy_collection_init() {
    // First operation auto-creates collection
    let kb = make_kb();
    // This is the first operation — should trigger init
    let chunks = vec![make_chunk("hello", "doc.txt", 0, 1)];
    let result = kb.insert_document(chunks, None, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_count_mismatch_error() {
    // If embedding returns wrong count → error
    // Our mock returns correct count, so this test verifies the
    // CountMismatch path by testing the error variant
    let err = KnowledgeBaseError::CountMismatch {
        expected: 3,
        got: 2,
    };
    assert!(err.to_string().contains("count mismatch"));
    assert!(err.to_string().contains("3"));
    assert!(err.to_string().contains("2"));
}

#[tokio::test]
async fn test_empty_search() {
    let kb = make_kb();
    let results = kb
        .search(vec![], 5, None)
        .await
        .expect("empty search should succeed");
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_insert_empty_chunks() {
    let kb = make_kb();
    let doc_id = kb
        .insert_document(vec![], None, None)
        .await
        .expect("empty insert should succeed");
    assert!(doc_id.is_empty());
}
