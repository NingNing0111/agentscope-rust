use std::collections::HashMap;
use std::sync::Arc;

use agent_scope_embedding::{
    EmbeddingError, EmbeddingInput, EmbeddingModel, EmbeddingModelCard, EmbeddingResponse,
    EmbeddingUsage,
};
use agent_scope_rag::chunker::Chunk;
use agent_scope_rag::turbovec_store::{CalibrationState, TurbovecVectorStore};
use agent_scope_rag::vector_store::{VectorRecord, VectorStore};

struct MockEmbedder {
    card: EmbeddingModelCard,
}

impl MockEmbedder {
    fn new(dimensions: u32) -> Self {
        Self {
            card: EmbeddingModelCard::new("mock-embedder", dimensions, false),
        }
    }
}

#[async_trait::async_trait]
impl EmbeddingModel for MockEmbedder {
    async fn embed(
        &self,
        inputs: Vec<EmbeddingInput>,
    ) -> Result<EmbeddingResponse, EmbeddingError> {
        let embeddings = inputs
            .into_iter()
            .map(|input| match input {
                EmbeddingInput::Text(text) => vector_for_text(&text, self.card.dimensions as usize),
                EmbeddingInput::DataBlock(text) => {
                    vector_for_text(&text, self.card.dimensions as usize)
                }
            })
            .collect();
        Ok(EmbeddingResponse {
            embeddings,
            usage: EmbeddingUsage { total_tokens: 0 },
        })
    }

    fn model_card(&self) -> &EmbeddingModelCard {
        &self.card
    }
}

fn vector_for_text(text: &str, dim: usize) -> Vec<f32> {
    let mut vector = vec![0.0; dim];
    let slot = text.bytes().fold(0_usize, |acc, b| acc + b as usize) % dim;
    vector[slot] = 1.0;
    vector
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

fn make_record(document_id: &str, chunk_index: usize, vector: Vec<f32>) -> VectorRecord {
    VectorRecord {
        vector,
        document_id: document_id.to_string(),
        chunk: make_chunk(
            &format!("content-{document_id}-{chunk_index}"),
            &format!("{document_id}.md"),
            chunk_index,
            1,
        ),
    }
}

fn unit_vector(dim: usize, active: usize) -> Vec<f32> {
    let mut vector = vec![0.0; dim];
    vector[active] = 1.0;
    vector
}

fn records(count: usize, dim: usize) -> Vec<VectorRecord> {
    (0..count)
        .map(|i| make_record(&format!("doc-{i}"), 0, unit_vector(dim, i % dim)))
        .collect()
}

#[tokio::test]
async fn test_create_and_has_collection() {
    let store = TurbovecVectorStore::new(4).expect("store should construct");
    assert!(!store.has_collection("test").await.expect("has_collection"));
    store
        .create_collection("test", 16)
        .await
        .expect("create should succeed");
    assert!(store.has_collection("test").await.expect("has_collection"));
}

#[tokio::test]
async fn test_insert_and_search() {
    let store = TurbovecVectorStore::new(4).expect("store should construct");
    store
        .insert("test", records(100, 16))
        .await
        .expect("insert should succeed");

    let results = store
        .search("test", unit_vector(16, 3), 10, None)
        .await
        .expect("search should succeed");
    assert_eq!(results.len(), 10);
    assert!(
        results
            .windows(2)
            .all(|pair| pair[0].score >= pair[1].score)
    );
}

#[tokio::test]
async fn test_delete_then_search_empty() {
    let store = TurbovecVectorStore::new(4).expect("store should construct");
    store
        .insert("test", vec![make_record("doc-1", 0, unit_vector(16, 0))])
        .await
        .expect("insert should succeed");
    store
        .delete("test", "doc-1")
        .await
        .expect("delete should succeed");
    let results = store
        .search("test", unit_vector(16, 0), 5, None)
        .await
        .expect("search should succeed");
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_delete_nonexistent_idempotent() {
    let store = TurbovecVectorStore::new(4).expect("store should construct");
    store
        .create_collection("test", 16)
        .await
        .expect("create should succeed");
    store
        .delete("test", "missing")
        .await
        .expect("delete should be idempotent");
}

#[tokio::test]
async fn test_list_documents() {
    let store = TurbovecVectorStore::new(4).expect("store should construct");
    store
        .insert(
            "test",
            vec![
                make_record("doc-a", 0, unit_vector(16, 0)),
                make_record("doc-a", 1, unit_vector(16, 1)),
                make_record("doc-b", 0, unit_vector(16, 2)),
            ],
        )
        .await
        .expect("insert should succeed");

    let docs = store
        .list_documents("test", None)
        .await
        .expect("list should succeed");
    assert_eq!(docs.len(), 2);
    assert_eq!(
        docs.iter()
            .find(|doc| doc.document_id == "doc-a")
            .expect("doc-a should exist")
            .chunk_count,
        2
    );
}

#[tokio::test]
async fn test_metadata_filter_search() {
    let store = TurbovecVectorStore::new(4).expect("store should construct");
    let mut keep = make_record("doc-keep", 0, unit_vector(16, 0));
    keep.chunk.metadata.insert("tenant".into(), "a".into());
    let mut drop = make_record("doc-drop", 0, unit_vector(16, 0));
    drop.chunk.metadata.insert("tenant".into(), "b".into());
    store
        .insert("test", vec![keep, drop])
        .await
        .expect("insert should succeed");

    let results = store
        .search(
            "test",
            unit_vector(16, 0),
            5,
            Some(HashMap::from([("tenant".to_string(), "a".to_string())])),
        )
        .await
        .expect("search should succeed");
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .all(|result| result.document_id == "doc-keep")
    );
}

#[tokio::test]
async fn test_dimension_mismatch_error() {
    let store = TurbovecVectorStore::new(4).expect("store should construct");
    store
        .create_collection("test", 16)
        .await
        .expect("create should succeed");
    let err = store
        .insert("test", vec![make_record("doc", 0, unit_vector(8, 0))])
        .await
        .expect_err("insert should fail");
    assert!(err.to_string().contains("dimension mismatch"));
}

#[tokio::test]
async fn test_empty_search_on_empty_collection() {
    let store = TurbovecVectorStore::new(4).expect("store should construct");
    store
        .create_collection("test", 16)
        .await
        .expect("create should succeed");
    let results = store
        .search("test", unit_vector(16, 0), 5, None)
        .await
        .expect("search should succeed");
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_bit_width_validation() {
    assert!(TurbovecVectorStore::new(0).is_err());
    assert!(TurbovecVectorStore::new(1).is_err());
    assert!(TurbovecVectorStore::new(5).is_err());
    assert!(TurbovecVectorStore::new(2).is_ok());
    assert!(TurbovecVectorStore::new(3).is_ok());
    assert!(TurbovecVectorStore::new(4).is_ok());
}

#[tokio::test]
async fn test_concurrent_search() {
    let store = Arc::new(TurbovecVectorStore::new(4).expect("store should construct"));
    store
        .insert("test", records(100, 16))
        .await
        .expect("insert should succeed");

    let mut handles = Vec::new();
    for _ in 0..8 {
        let store = Arc::clone(&store);
        handles.push(tokio::spawn(async move {
            store.search("test", unit_vector(16, 0), 5, None).await
        }));
    }

    for handle in handles {
        let results = handle.await.expect("join should succeed").expect("search");
        assert!(!results.is_empty());
        assert!(results.len() <= 5);
    }
}

#[tokio::test]
async fn test_save_load_roundtrip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = TurbovecVectorStore::new(4).expect("store should construct");
    store
        .insert("test", records(100, 16))
        .await
        .expect("insert should succeed");
    let before = store
        .search("test", unit_vector(16, 0), 10, None)
        .await
        .expect("search should succeed");
    store.save(dir.path()).await.expect("save should succeed");

    let loaded = TurbovecVectorStore::load(dir.path())
        .await
        .expect("load should succeed");
    assert!(loaded.has_collection("test").await.expect("has_collection"));
    let after = loaded
        .search("test", unit_vector(16, 0), 10, None)
        .await
        .expect("search should succeed");
    assert_eq!(before.len(), after.len());
    assert_eq!(before[0].document_id, after[0].document_id);
    assert_eq!(
        store
            .list_documents("test", None)
            .await
            .expect("list should succeed")
            .len(),
        loaded
            .list_documents("test", None)
            .await
            .expect("list should succeed")
            .len()
    );
}

#[tokio::test]
async fn test_save_empty_store() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = TurbovecVectorStore::new(4).expect("store should construct");
    store.save(dir.path()).await.expect("save should succeed");
    let loaded = TurbovecVectorStore::load(dir.path())
        .await
        .expect("load should succeed");
    assert!(!loaded.has_collection("test").await.expect("has_collection"));
}

#[tokio::test]
async fn test_save_load_append_more() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = TurbovecVectorStore::new(4).expect("store should construct");
    store
        .insert("test", records(10, 16))
        .await
        .expect("insert should succeed");
    store.save(dir.path()).await.expect("save should succeed");

    let loaded = TurbovecVectorStore::load(dir.path())
        .await
        .expect("load should succeed");
    loaded
        .insert(
            "test",
            records(10, 16)
                .into_iter()
                .enumerate()
                .map(|(i, mut record)| {
                    record.document_id = format!("extra-{i}");
                    record
                })
                .collect(),
        )
        .await
        .expect("append should succeed");
    loaded.save(dir.path()).await.expect("save should succeed");
    let reloaded = TurbovecVectorStore::load(dir.path())
        .await
        .expect("reload should succeed");
    assert_eq!(
        reloaded
            .list_documents("test", None)
            .await
            .expect("list should succeed")
            .len(),
        20
    );
}

#[tokio::test]
async fn test_load_corrupted_manifest_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("manifest.json"), b"not-json").expect("write manifest");
    let result = TurbovecVectorStore::load(dir.path()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_save_multiple_collections() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = TurbovecVectorStore::new(4).expect("store should construct");
    for name in ["a", "b", "c"] {
        store
            .insert(name, records(5, 16))
            .await
            .expect("insert should succeed");
    }
    store.save(dir.path()).await.expect("save should succeed");
    let loaded = TurbovecVectorStore::load(dir.path())
        .await
        .expect("load should succeed");
    for name in ["a", "b", "c"] {
        assert!(loaded.has_collection(name).await.expect("has_collection"));
        assert_eq!(
            loaded
                .search(name, unit_vector(16, 0), 5, None)
                .await
                .expect("search should succeed")
                .len(),
            5
        );
    }
}

#[tokio::test]
async fn test_knowledge_base_with_turbovec_store() {
    let store = Arc::new(TurbovecVectorStore::new(4).expect("store should construct"));
    let kb = agent_scope_rag::knowledge_base::KnowledgeBase::new(
        "kb".into(),
        "test kb".into(),
        Arc::new(MockEmbedder::new(16)),
        store,
        "kb_collection".into(),
        None,
    );
    let doc_id = kb
        .insert_document(
            vec![make_chunk("apple", "fruit.md", 0, 1)],
            Some("doc-apple".into()),
            None,
        )
        .await
        .expect("insert should succeed");
    assert_eq!(doc_id, "doc-apple");
    let results = kb
        .search(vec![EmbeddingInput::Text("apple".into())], 1, None)
        .await
        .expect("search should succeed");
    assert_eq!(results[0].document_id, "doc-apple");
    assert_eq!(results[0].chunk.content, "apple");
}

#[tokio::test]
async fn test_calibration_state_tracking() {
    let store = TurbovecVectorStore::new(4).expect("store should construct");
    store
        .insert("test", records(10, 16))
        .await
        .expect("insert should succeed");
    assert_eq!(
        store.calibration_state("test").await.expect("state"),
        CalibrationState::WarmingUp
    );

    store
        .insert(
            "test",
            (0..1000)
                .map(|i| make_record(&format!("fit-{i}"), 0, unit_vector(16, i % 16)))
                .collect(),
        )
        .await
        .expect("insert should succeed");
    assert_eq!(
        store.calibration_state("test").await.expect("state"),
        CalibrationState::Fitted
    );
}

#[tokio::test]
async fn test_metadata_filter_enforced_by_kb() {
    let store = Arc::new(TurbovecVectorStore::new(4).expect("store should construct"));
    let kb = agent_scope_rag::knowledge_base::KnowledgeBase::new(
        "kb".into(),
        "test kb".into(),
        Arc::new(MockEmbedder::new(16)),
        store,
        "kb_filter_collection".into(),
        Some(HashMap::from([("tenant".to_string(), "a".to_string())])),
    );
    kb.insert_document(
        vec![make_chunk("apple", "fruit.md", 0, 1)],
        Some("doc-a".into()),
        None,
    )
    .await
    .expect("insert should succeed");
    let results = kb
        .search(vec![EmbeddingInput::Text("apple".into())], 5, None)
        .await
        .expect("search should succeed");
    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .all(|result| result.chunk.metadata.get("tenant") == Some(&"a".to_string()))
    );
}
