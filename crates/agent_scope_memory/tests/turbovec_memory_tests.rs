//! Tests for TurbovecMemory — TurboVec-backed long-term memory.
//!
//! Uses a mock `EmbeddingModel` and an in-memory `MemoryVectorIndex` for
//! deterministic, fast tests without external dependencies.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use agent_scope_embedding::{
    EmbeddingError, EmbeddingInput, EmbeddingModel, EmbeddingModelCard, EmbeddingResponse,
    EmbeddingUsage,
};
use agent_scope_memory::{
    Memory, MemoryEntry, MemoryType, MemoryVectorHit, MemoryVectorIndex, MemoryVectorRecord,
    TurbovecMemory, TurbovecMemoryConfig,
};

// ── Mock Embedding Model ──────────────────────────────────────────────────

/// Deterministic mock: produces a vector from the byte sum of the input text.
struct MockEmbedder {
    dims: u32,
}

impl MockEmbedder {
    fn new(dims: u32) -> Self {
        Self { dims }
    }
}

/// Simple deterministic embedding: sum of bytes → normalized vector.
fn embed_text(text: &str, dims: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity(dims as usize);
    let bytes: Vec<u8> = text.bytes().collect();
    for i in 0..dims as usize {
        let val = if bytes.is_empty() {
            (i as f32) / (dims as f32)
        } else {
            let b = bytes[i % bytes.len()] as f32;
            (b / 255.0 + (i as f32) / (dims as f32 * 2.0)) % 1.0
        };
        v.push(val);
    }
    // L2 normalize
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-10 {
        v.iter_mut().for_each(|x| *x /= norm);
    }
    v
}

#[async_trait::async_trait]
impl EmbeddingModel for MockEmbedder {
    fn model_card(&self) -> &EmbeddingModelCard {
        // Arc-like, but we return a static ref via leak for test simplicity.
        Box::leak(Box::new(EmbeddingModelCard::new("mock", self.dims, false)))
    }

    async fn embed(
        &self,
        inputs: Vec<EmbeddingInput>,
    ) -> Result<EmbeddingResponse, EmbeddingError> {
        let mut embeddings = Vec::new();
        for input in &inputs {
            match input {
                EmbeddingInput::Text(t) => embeddings.push(embed_text(t, self.dims)),
                EmbeddingInput::DataBlock(_) => {
                    return Err(EmbeddingError::MultimodalNotSupported);
                }
            }
        }
        Ok(EmbeddingResponse {
            embeddings,
            usage: EmbeddingUsage { total_tokens: 0 },
        })
    }
}

// ── In-Memory Mock Vector Index ───────────────────────────────────────────

struct InMemIndex {
    collections: Mutex<HashMap<String, Vec<MemoryVectorRecord>>>,
}

impl InMemIndex {
    fn new() -> Self {
        Self {
            collections: Mutex::new(HashMap::new()),
        }
    }

    fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if na < 1e-10 || nb < 1e-10 {
            0.0
        } else {
            dot / (na * nb)
        }
    }
}

#[async_trait::async_trait]
impl MemoryVectorIndex for InMemIndex {
    async fn has_collection(&self, name: &str) -> Result<bool, String> {
        Ok(self.collections.lock().unwrap().contains_key(name))
    }

    async fn create_collection(&self, name: &str, _dimensions: u32) -> Result<(), String> {
        self.collections
            .lock()
            .unwrap()
            .entry(name.to_string())
            .or_default();
        Ok(())
    }

    async fn search(
        &self,
        collection: &str,
        query_vector: Vec<f32>,
        top_k: usize,
        metadata_filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<MemoryVectorHit>, String> {
        let guard = self.collections.lock().unwrap();
        let records = guard.get(collection).ok_or("collection not found")?;

        let mut scored: Vec<(f32, &MemoryVectorRecord)> = records
            .iter()
            .filter(|r| {
                if let Some(filter) = &metadata_filter {
                    filter.iter().all(|(k, v)| r.metadata.get(k) == Some(v))
                } else {
                    true
                }
            })
            .map(|r| (Self::cosine_sim(&query_vector, &r.vector), r))
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scored
            .into_iter()
            .take(top_k)
            .map(|(score, r)| MemoryVectorHit {
                score,
                document_id: r.document_id.clone(),
                metadata: r.metadata.clone(),
                content: r.content.clone(),
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
        let mut guard = self.collections.lock().unwrap();
        let entry = guard.entry(collection.to_string()).or_default();
        // Delete old records for each document_id first (upsert).
        for r in &records {
            entry.retain(|existing| existing.document_id != r.document_id);
        }
        entry.extend(records);
        Ok(())
    }

    async fn delete(&self, collection: &str, document_id: &str) -> Result<(), String> {
        let mut guard = self.collections.lock().unwrap();
        if let Some(entry) = guard.get_mut(collection) {
            entry.retain(|r| r.document_id != document_id);
        }
        Ok(())
    }

    async fn save(&self, _path: &str) -> Result<(), String> {
        Ok(())
    }

    async fn load(&self, _path: &str) -> Result<(), String> {
        Ok(())
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

async fn make_memory(
    workdir: &str,
    dims: u32,
    auto_rebuild: bool,
) -> (TurbovecMemory, Arc<InMemIndex>) {
    let embedding = Arc::new(MockEmbedder::new(dims));
    let index = Arc::new(InMemIndex::new());
    let config = TurbovecMemoryConfig {
        memory_dir: ".memory".into(),
        auto_rebuild,
        retrieval_top_k: 10,
        ..Default::default()
    };
    let memory = TurbovecMemory::new(
        workdir,
        config,
        embedding,
        index.clone() as Arc<dyn MemoryVectorIndex>,
        None,
    )
    .await
    .expect("create TurbovecMemory");
    (memory, index)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_write_read_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let dir_s = dir.path().to_string_lossy().to_string();
    let (mem, _idx) = make_memory(&dir_s, 8, false).await;

    let entry = MemoryEntry::new(
        "test-entry",
        "a test memory",
        MemoryType::User,
        "hello world",
    );
    mem.write(entry).await.expect("write");

    let got = mem.read("test-entry").await.expect("read").expect("found");
    assert_eq!(got.name, "test-entry");
    assert_eq!(got.description, "a test memory");
    assert_eq!(got.metadata.mem_type, MemoryType::User);
    assert_eq!(got.content, "hello world");
}

#[tokio::test]
async fn test_semantic_search_returns_ranked_results() {
    let dir = tempfile::tempdir().unwrap();
    let dir_s = dir.path().to_string_lossy().to_string();
    let (mem, _idx) = make_memory(&dir_s, 8, false).await;

    mem.write(MemoryEntry::new(
        "deploy",
        "deployment config",
        MemoryType::Project,
        "kubernetes deployment with helm charts for production",
    ))
    .await
    .unwrap();

    mem.write(MemoryEntry::new(
        "lunch",
        "lunch preference",
        MemoryType::User,
        "the user likes sushi on fridays",
    ))
    .await
    .unwrap();

    mem.write(MemoryEntry::new(
        "cicd",
        "CI/CD pipeline",
        MemoryType::Project,
        "github actions for ci/cd with docker build and push to ECR",
    ))
    .await
    .unwrap();

    let results = mem
        .semantic_search("kubernetes deployment pipeline", None, 3)
        .await
        .expect("search");

    assert!(!results.is_empty(), "should return results");
    // The first result should be deploy-related, not lunch.
    assert!(
        results[0].memory_name == "deploy" || results[0].memory_name == "cicd",
        "top result should be project-related, got: {}",
        results[0].memory_name
    );
}

#[tokio::test]
async fn test_upsert_replaces_old_content() {
    let dir = tempfile::tempdir().unwrap();
    let dir_s = dir.path().to_string_lossy().to_string();
    let (mem, _idx) = make_memory(&dir_s, 8, false).await;

    mem.write(MemoryEntry::new(
        "note",
        "v1",
        MemoryType::User,
        "original content",
    ))
    .await
    .unwrap();

    // Upsert
    mem.write(MemoryEntry::new(
        "note",
        "v2",
        MemoryType::User,
        "updated content",
    ))
    .await
    .unwrap();

    let got = mem.read("note").await.unwrap().unwrap();
    assert_eq!(got.content, "updated content");
    assert_eq!(got.description, "v2");
}

#[tokio::test]
async fn test_delete_removes_from_file_and_index() {
    let dir = tempfile::tempdir().unwrap();
    let dir_s = dir.path().to_string_lossy().to_string();
    let (mem, _idx) = make_memory(&dir_s, 8, false).await;

    mem.write(MemoryEntry::new(
        "del-me",
        "to delete",
        MemoryType::Reference,
        "garbage data",
    ))
    .await
    .unwrap();

    mem.delete("del-me").await.unwrap();
    assert!(mem.read("del-me").await.unwrap().is_none());

    // Search should not return deleted.
    let results = mem.semantic_search("garbage", None, 5).await.unwrap();
    assert!(
        !results.iter().any(|r| r.memory_name == "del-me"),
        "deleted entry should not appear in search"
    );
}

#[tokio::test]
async fn test_save_and_reload_preserves_data() {
    let dir = tempfile::tempdir().unwrap();
    let dir_s = dir.path().to_string_lossy().to_string();
    let (mem1, idx1) = make_memory(&dir_s, 8, false).await;

    mem1.write(MemoryEntry::new(
        "persist",
        "persistent entry",
        MemoryType::User,
        "important data that must survive",
    ))
    .await
    .unwrap();

    mem1.save_index().await.unwrap();

    // Create a second instance with the same index.
    let embedding2: Arc<dyn EmbeddingModel> = Arc::new(MockEmbedder::new(8));
    let idx2: Arc<dyn MemoryVectorIndex> = idx1; // reuse same index
    let config2 = TurbovecMemoryConfig {
        memory_dir: ".memory".into(),
        auto_rebuild: true,
        retrieval_top_k: 10,
        ..Default::default()
    };
    let mem2 = TurbovecMemory::new(&dir_s, config2, embedding2, idx2, None)
        .await
        .unwrap();

    let got = mem2.read("persist").await.unwrap().unwrap();
    assert_eq!(got.name, "persist");
    assert_eq!(got.content, "important data that must survive");
}

#[tokio::test]
async fn test_empty_store_search_returns_empty_vec() {
    let dir = tempfile::tempdir().unwrap();
    let dir_s = dir.path().to_string_lossy().to_string();
    let (mem, _idx) = make_memory(&dir_s, 8, false).await;

    let results = mem.semantic_search("anything", None, 5).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_empty_query_returns_validation_error() {
    let dir = tempfile::tempdir().unwrap();
    let dir_s = dir.path().to_string_lossy().to_string();
    let (mem, _idx) = make_memory(&dir_s, 8, false).await;

    let err = mem.semantic_search("   ", None, 5).await.unwrap_err();
    assert!(
        matches!(err, agent_scope_memory::MemoryError::ValidationError { .. }),
        "expected ValidationError, got: {err:?}"
    );
}

// ── User Story 2 tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_retrieve_relevant_respects_max_results() {
    let dir = tempfile::tempdir().unwrap();
    let dir_s = dir.path().to_string_lossy().to_string();
    let (mem, _idx) = make_memory(&dir_s, 8, false).await;

    for i in 0..10 {
        mem.write(MemoryEntry::new(
            format!("entry-{i}"),
            format!("entry {i}"),
            MemoryType::User,
            format!("content for entry number {i} in the system"),
        ))
        .await
        .unwrap();
    }

    let results = mem.semantic_search("entry number", None, 3).await.unwrap();
    assert!(results.len() <= 3, "should respect top_k=3");
}

#[tokio::test]
async fn test_content_truncation() {
    let dir = tempfile::tempdir().unwrap();
    let dir_s = dir.path().to_string_lossy().to_string();
    let (mem, _idx) = make_memory(&dir_s, 8, false).await;

    let long_content = "x".repeat(10000);
    mem.write(MemoryEntry::new(
        "long",
        "long entry",
        MemoryType::User,
        &long_content,
    ))
    .await
    .unwrap();

    let results = mem
        .semantic_search("long entry content", None, 1)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    // Content should be truncated (retrieval_max_tokens_per_file=2000 → ~8000 chars)
    assert!(results[0].content.len() < long_content.len());
}

// ── User Story 3 tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_rebuild_index() {
    let dir = tempfile::tempdir().unwrap();
    let dir_s = dir.path().to_string_lossy().to_string();
    let (mem, idx) = make_memory(&dir_s, 8, false).await;

    mem.write(MemoryEntry::new(
        "rb-1",
        "rebuild test 1",
        MemoryType::User,
        "first memory for rebuild test",
    ))
    .await
    .unwrap();

    mem.write(MemoryEntry::new(
        "rb-2",
        "rebuild test 2",
        MemoryType::Project,
        "second memory for rebuild test",
    ))
    .await
    .unwrap();

    // Clear the index to simulate corruption.
    idx.delete(mem.file_memory().root_dir(), "rb-1").await.ok();
    idx.delete(mem.file_memory().root_dir(), "rb-2").await.ok();

    // Rebuild.
    let report = mem.rebuild_index().await.unwrap();
    assert_eq!(report.total_scanned, 2);
    assert_eq!(report.indexed, 2);
    assert_eq!(report.skipped, 0);
    assert!(report.errors.is_empty());

    // Search should work after rebuild.
    let results = mem.semantic_search("rebuild test", None, 5).await.unwrap();
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_rebuild_report_contains_correct_counts() {
    let dir = tempfile::tempdir().unwrap();
    let dir_s = dir.path().to_string_lossy().to_string();
    let (mem, _idx) = make_memory(&dir_s, 8, false).await;

    mem.write(MemoryEntry::new(
        "cnt-1",
        "count test",
        MemoryType::User,
        "content one",
    ))
    .await
    .unwrap();

    mem.write(MemoryEntry::new(
        "cnt-2",
        "count test 2",
        MemoryType::Project,
        "content two",
    ))
    .await
    .unwrap();

    let report = mem.rebuild_index().await.unwrap();
    assert_eq!(report.total_scanned, 2);
    assert_eq!(report.indexed, 2);
    assert_eq!(report.skipped, 0);
}

#[tokio::test]
async fn test_rebuild_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let dir_s = dir.path().to_string_lossy().to_string();
    let (mem, _idx) = make_memory(&dir_s, 8, false).await;

    mem.write(MemoryEntry::new(
        "idem-1",
        "idempotent test",
        MemoryType::User,
        "test content",
    ))
    .await
    .unwrap();

    let r1 = mem.rebuild_index().await.unwrap();
    let r2 = mem.rebuild_index().await.unwrap();
    assert_eq!(r1.total_scanned, r2.total_scanned);
    assert_eq!(r1.indexed, r2.indexed);
}

#[tokio::test]
async fn test_type_filter_restricts_results() {
    let dir = tempfile::tempdir().unwrap();
    let dir_s = dir.path().to_string_lossy().to_string();
    let (mem, _idx) = make_memory(&dir_s, 8, false).await;

    mem.write(MemoryEntry::new(
        "user-1",
        "user entry",
        MemoryType::User,
        "alice likes rust programming",
    ))
    .await
    .unwrap();

    mem.write(MemoryEntry::new(
        "proj-1",
        "project entry",
        MemoryType::Project,
        "rust project uses cargo and clippy",
    ))
    .await
    .unwrap();

    // Search with User type filter.
    let user_results = mem
        .semantic_search("rust programming", Some(MemoryType::User), 5)
        .await
        .unwrap();
    assert!(
        user_results.iter().all(|r| r.memory_name == "user-1"),
        "type filter should restrict to User type"
    );

    // Search with Project type filter.
    let proj_results = mem
        .semantic_search("rust programming", Some(MemoryType::Project), 5)
        .await
        .unwrap();
    assert!(
        proj_results.iter().all(|r| r.memory_name == "proj-1"),
        "type filter should restrict to Project type"
    );
}

#[tokio::test]
async fn test_vector_index_status() {
    let dir = tempfile::tempdir().unwrap();
    let dir_s = dir.path().to_string_lossy().to_string();
    let (mem, _idx) = make_memory(&dir_s, 8, false).await;

    // Status should be Missing at first (no .turbovec directory).
    let status = mem.vector_index_status().await.unwrap();
    // With the in-memory index, there's no actual directory, so Clean or Missing
    assert!(matches!(
        status,
        agent_scope_memory::VectorIndexStatus::Missing
            | agent_scope_memory::VectorIndexStatus::Clean
    ));
}
