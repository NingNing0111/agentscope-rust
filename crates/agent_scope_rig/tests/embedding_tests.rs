//! T020 — `RigEmbeddingModel` embedding 测试。
//!
//! 对照 `specs/034-rig-llm-integration/contracts/rig-mapping.md` §7 与
//! `contracts/provider-adapter.md` §2：
//! 1. `embed(Vec<Text>)` 每输入一个向量，长度 = `model_card().dimensions`；
//! 2. `DataBlock` → `EmbeddingError::MultimodalNotSupported`（契约 §4：OpenAI
//!    embedding 无多模态）；
//! 3. `model_card` 生命周期内稳定；
//! 4. 与 `agent_scope_embedding::cache::FileEmbeddingCache` 集成往返（embed →
//!    store → lookup 一致）。
//!
//! 后端用可编程 mock（`from_backend_for_testing` 注入），确定性、不依赖网络。

use std::sync::{Arc, Mutex};

use agent_scope_embedding::error::EmbeddingError;
use agent_scope_embedding::{
    EmbeddingInput, EmbeddingModel as EmbeddingModelTrait, EmbeddingModelCard,
};
use agent_scope_rig::RigEmbeddingModel;
use agent_scope_rig::backend::RigEmbeddingBackend;

const N: u32 = 1536; // 模拟 text-embedding-3-small 维度

/// 可编程 mock embedding backend：记录收到的文本，按 ndims 生成确定性向量。
///
/// 向量值 = `text.len() as f32` 均匀填充，便于断言"不同输入不同向量、维度一致"。
struct MockEmbeddingBackend {
    ndims: u32,
    calls: Arc<Mutex<Vec<Vec<String>>>>,
}

impl MockEmbeddingBackend {
    fn new(ndims: u32) -> (Self, Arc<Mutex<Vec<Vec<String>>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let backend = Self {
            ndims,
            calls: calls.clone(),
        };
        (backend, calls)
    }

    fn vec_for(text: &str, ndims: u32) -> Vec<f32> {
        vec![text.len() as f32; ndims as usize]
    }
}

#[async_trait::async_trait]
impl RigEmbeddingBackend for MockEmbeddingBackend {
    fn ndims(&self) -> u32 {
        self.ndims
    }

    async fn embed_texts(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        self.calls.lock().unwrap().push(texts.clone());
        Ok(texts.iter().map(|t| Self::vec_for(t, self.ndims)).collect())
    }

    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(Self::vec_for(text, self.ndims))
    }
}

/// 每输入一向量，长度 = model_card().dimensions。
#[tokio::test]
async fn embed_text_returns_one_vector_per_input_with_card_dimensions() {
    let (backend, calls) = MockEmbeddingBackend::new(N);
    let model =
        RigEmbeddingModel::from_backend_for_testing("text-embedding-3-small", N, Arc::new(backend));

    let resp = model
        .embed(vec![
            EmbeddingInput::Text("hello".to_string()),
            EmbeddingInput::Text("world".to_string()),
            EmbeddingInput::Text("rust".to_string()),
        ])
        .await
        .expect("embed must succeed");

    // 每输入一个向量。
    assert_eq!(resp.embeddings.len(), 3);
    // 每个向量长度 = model card 维度。
    for v in &resp.embeddings {
        assert_eq!(v.len(), N as usize);
    }
    // 全部输入一次性批量送到底层（一次 embed_texts 调用，3 条文本）。
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(calls.lock().unwrap()[0], vec!["hello", "world", "rust"]);
}

/// 维度由 backend ndims 探知，嵌入结果与之严格一致。
#[tokio::test]
async fn embed_text_length_matches_model_card_dimensions() {
    let (backend, _calls) = MockEmbeddingBackend::new(N);
    let model =
        RigEmbeddingModel::from_backend_for_testing("text-embedding-3-small", N, Arc::new(backend));

    let resp = model
        .embed(vec![EmbeddingInput::Text("short".to_string())])
        .await
        .expect("embed must succeed");

    assert_eq!(
        resp.embeddings[0].len() as u32,
        model.model_card().dimensions
    );
    assert_eq!(model.model_card().dimensions, N);
    // 向量内容非零（确定性填充值 = 文本长度），确保不是"占位空向量"。
    assert_eq!(resp.embeddings[0][0], 5.0_f32);
}

/// DataBlock（多模态）输入 → 明确拒绝（契约 §4 能力矩阵）。
#[tokio::test]
async fn data_block_input_returns_multimodal_not_supported() {
    let (backend, calls) = MockEmbeddingBackend::new(N);
    let model =
        RigEmbeddingModel::from_backend_for_testing("text-embedding-3-small", N, Arc::new(backend));

    let err = model
        .embed(vec![EmbeddingInput::DataBlock("base64image".to_string())])
        .await
        .expect_err("DataBlock must be rejected");

    assert!(
        matches!(err, EmbeddingError::MultimodalNotSupported),
        "expected MultimodalNotSupported, got {err}"
    );
    // 拒绝发生在调用底层之前——零后端调用。
    assert!(
        calls.lock().unwrap().is_empty(),
        "backend must not be invoked for DataBlock input"
    );
}

/// 混合输入含任一 DataBlock → 整体拒绝（不含糊丢弃部分输入）。
#[tokio::test]
async fn mixed_input_with_data_block_is_rejected() {
    let (backend, _calls) = MockEmbeddingBackend::new(N);
    let model =
        RigEmbeddingModel::from_backend_for_testing("text-embedding-3-small", N, Arc::new(backend));

    let err = model
        .embed(vec![
            EmbeddingInput::Text("text ok".to_string()),
            EmbeddingInput::DataBlock("image".to_string()),
        ])
        .await
        .expect_err("mixed input with DataBlock must be rejected");

    assert!(
        matches!(err, EmbeddingError::MultimodalNotSupported),
        "expected MultimodalNotSupported, got {err}"
    );
}

/// model_card 生命周期内稳定：多次调用返回一致元数据。
#[tokio::test]
async fn model_card_is_stable_across_calls() {
    let (backend, _calls) = MockEmbeddingBackend::new(N);
    let model =
        RigEmbeddingModel::from_backend_for_testing("text-embedding-3-small", N, Arc::new(backend));

    let card1: &EmbeddingModelCard = model.model_card();
    let card2: &EmbeddingModelCard = model.model_card();
    assert_eq!(card1.name, card2.name);
    assert_eq!(card1.dimensions, card2.dimensions);
    assert_eq!(card1.supports_multimodal, card2.supports_multimodal);
    // 语义断言：RigEmbeddingModel 无多模态能力。
    assert!(!model.supports_multimodal());
    assert_eq!(card1.name, "text-embedding-3-small");
    assert_eq!(card1.dimensions, N);
}

/// 与 FileEmbeddingCache 集成往返：embed → store(hash_key) → lookup 一致。
#[tokio::test]
async fn integrates_with_file_embedding_cache_round_trip() {
    use agent_scope_embedding::EmbeddingCache;
    use agent_scope_embedding::cache::{FileEmbeddingCache, hash_key};

    // 唯一临时目录（std 临时目录 + pid + 计数器），避免并行测试串扰。
    let dir = std::env::temp_dir().join(format!(
        "agentscope-rig-embed-cache-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let cache = FileEmbeddingCache::new(dir.clone()).expect("create cache");

    let (backend, _calls) = MockEmbeddingBackend::new(N);
    let model =
        RigEmbeddingModel::from_backend_for_testing("text-embedding-3-small", N, Arc::new(backend));

    let text = "cacheable knowledge chunk".to_string();
    let resp = model
        .embed(vec![EmbeddingInput::Text(text.clone())])
        .await
        .expect("embed must succeed");
    let embeddings = resp.embeddings.clone();

    // 内容寻址存储 → 命中。
    let key = hash_key(&text);
    cache.store(&key, embeddings.clone());
    let cached = cache.lookup(&key).expect("cache hit after store");

    assert_eq!(cached.len(), embeddings.len());
    assert_eq!(cached[0].len(), N as usize);
    assert_eq!(cached, embeddings, "cache round-trip must be lossless");

    // 清理临时目录。
    let _ = std::fs::remove_dir_all(&dir);
}

/// 空输入：embed 返回空向量集；底层收到一次空批量调用（不做特殊短路）。
#[tokio::test]
async fn empty_inputs_returns_empty_embeddings() {
    let (backend, calls) = MockEmbeddingBackend::new(N);
    let model =
        RigEmbeddingModel::from_backend_for_testing("text-embedding-3-small", N, Arc::new(backend));

    let resp = model
        .embed(vec![])
        .await
        .expect("empty input must be valid");
    assert!(resp.embeddings.is_empty());
    let recorded = calls.lock().unwrap();
    assert_eq!(recorded.len(), 1, "one empty batch call expected");
    assert!(recorded[0].is_empty(), "batch must carry zero texts");
}
