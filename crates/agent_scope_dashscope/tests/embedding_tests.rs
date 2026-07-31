//! Integration tests for DashScope embedding model.
//!
//! Tests requiring DASHSCOPE_API_KEY are marked `#[ignore]` by default.

use agent_scope_dashscope::DashScopeEmbeddingModel;
use agent_scope_embedding::{EmbeddingInput, EmbeddingModel, EmbeddingModelCard};

/// Helper to create a DashScopeEmbeddingModel for testing.
fn make_model(api_key: &str) -> DashScopeEmbeddingModel {
    let card = EmbeddingModelCard::new("text-embedding-v3", 1536, false);
    DashScopeEmbeddingModel::new(api_key.into(), card)
}

/// Integration test requiring DashScope API key.
/// Run with: `cargo test -p agent_scope_dashscope -- --ignored`
#[tokio::test]
#[ignore = "requires DASHSCOPE_API_KEY env var"]
async fn test_dashscope_embedding_basic() {
    let api_key =
        std::env::var("DASHSCOPE_API_KEY").expect("DASHSCOPE_API_KEY must be set to run this test");

    let model = make_model(&api_key);

    let inputs: Vec<EmbeddingInput> = vec!["你好，世界".into()];
    let response = model.embed(inputs).await.expect("embed should succeed");

    assert_eq!(response.embeddings.len(), 1);
    assert_eq!(
        response.embeddings[0].len(),
        model.model_card().dimensions as usize
    );
    assert!(response.usage.total_tokens > 0);
}

#[tokio::test]
#[ignore = "requires DASHSCOPE_API_KEY env var"]
async fn test_dashscope_embedding_multiple_inputs() {
    let api_key =
        std::env::var("DASHSCOPE_API_KEY").expect("DASHSCOPE_API_KEY must be set to run this test");

    let model = make_model(&api_key);

    let inputs: Vec<EmbeddingInput> = vec!["hello".into(), "world".into(), "test".into()];
    let response = model.embed(inputs).await.expect("embed should succeed");

    assert_eq!(response.embeddings.len(), 3);
    for emb in &response.embeddings {
        assert_eq!(emb.len(), model.model_card().dimensions as usize);
    }
}
