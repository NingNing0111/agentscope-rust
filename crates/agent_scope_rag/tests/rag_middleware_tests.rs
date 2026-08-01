//! Tests for the RAGMiddleware in static and agentic modes.
use std::collections::HashMap;
use std::sync::Arc;

use agent_scope_agent::middleware::Middleware;
use agent_scope_embedding::{
    EmbeddingError, EmbeddingInput, EmbeddingModel, EmbeddingModelCard, EmbeddingResponse,
    EmbeddingUsage,
};
use agent_scope_message::{ContentBlock, Msg, Role, TextBlock};
use agent_scope_model::{ChatModel, ModelCallResult, ModelError, ToolChoice};
use agent_scope_rag::chunker::Chunk;
use agent_scope_rag::knowledge_base::KnowledgeBase;
use agent_scope_rag::rag_middleware::{RAGMiddleware, RAGMode};
use agent_scope_rag::vector_store::{
    DocumentSummary, VectorRecord, VectorSearchResult, VectorStore,
};
use serde_json::Value as JsonValue;

// --- Mock Embedder ---
struct MockEmbedder {
    card: EmbeddingModelCard,
}

impl MockEmbedder {
    fn new(dims: u32) -> Self {
        Self {
            card: EmbeddingModelCard::new("mock", dims, false),
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
        Ok(EmbeddingResponse {
            embeddings: inputs.iter().map(|_| vec![0.0_f32; dim]).collect(),
            usage: EmbeddingUsage {
                total_tokens: inputs.len() as u32,
            },
        })
    }

    fn model_card(&self) -> &EmbeddingModelCard {
        &self.card
    }
}

// --- Mock VectorStore ---
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

    async fn create_collection(
        &self,
        name: &str,
        _dimensions: u32,
    ) -> Result<(), agent_scope_rag::error::VectorStoreError> {
        self.data
            .write()
            .unwrap()
            .entry(name.to_string())
            .or_default();
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
            .map(|r| VectorSearchResult {
                score: 0.95,
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
        self.data
            .write()
            .unwrap()
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
        if let Some(records) = self.data.write().unwrap().get_mut(collection) {
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

// --- Helpers ---
fn make_chunk(content: &str, source: &str, idx: usize, total: usize) -> Chunk {
    Chunk {
        content: content.to_string(),
        source: source.to_string(),
        chunk_index: idx,
        total_chunks: total,
        metadata: HashMap::new(),
    }
}

async fn make_kb_with_data(name: &str, chunks: Vec<Chunk>) -> Arc<KnowledgeBase> {
    let kb = Arc::new(KnowledgeBase::new(
        name.to_string(),
        format!("Knowledge base: {name}"),
        Arc::new(MockEmbedder::new(4)),
        Arc::new(MockVectorStore::new()),
        format!("col-{name}"),
        None,
    ));
    if !chunks.is_empty() {
        kb.insert_document(chunks, None, None)
            .await
            .expect("insert in setup");
    }
    kb
}

fn make_user_msg(text: &str) -> Msg {
    Msg::new(
        "user".into(),
        vec![ContentBlock::Text(TextBlock::new(text.to_string()))],
        Role::User,
    )
    .expect("create user msg")
}

// --- Static Mode Tests ---

#[tokio::test]
async fn test_static_mode_injects_context() {
    let chunks = vec![make_chunk(
        "critical infrastructure info",
        "policy.txt",
        0,
        1,
    )];
    let kb = make_kb_with_data("policies", chunks).await;

    let mw = RAGMiddleware::new(vec![kb], RAGMode::Static, 5, None);

    let mut input = Some(vec![make_user_msg("What is the policy?")]);
    let model: Arc<dyn agent_scope_model::ChatModel> = Arc::new(MockChatModel::new("test-model"));

    mw.pre_reply("agent", &mut input, &model)
        .await
        .expect("pre_reply should succeed");

    // The input should now have an extra system message from RAGMiddleware
    let msgs = input.expect("input still present");
    let rag_msg = msgs.iter().find(|m| m.name == "RAGMiddleware");
    assert!(
        rag_msg.is_some(),
        "RAGMiddleware should have injected a message"
    );
}

#[tokio::test]
async fn test_static_mode_empty_results_no_injection() {
    let kb = Arc::new(KnowledgeBase::new(
        "empty-kb".into(),
        "Empty".into(),
        Arc::new(MockEmbedder::new(4)),
        Arc::new(MockVectorStore::new()),
        "empty-col".into(),
        None,
    ));

    let mw = RAGMiddleware::new(vec![kb], RAGMode::Static, 5, None);

    let mut input = Some(vec![make_user_msg("random query")]);
    let model: Arc<dyn agent_scope_model::ChatModel> = Arc::new(MockChatModel::new("test-model"));

    mw.pre_reply("agent", &mut input, &model)
        .await
        .expect("pre_reply should succeed");

    // Should not have added RAGMiddleware message since no results
    let msgs = input.expect("input still present");
    let rag_msg = msgs.iter().find(|m| m.name == "RAGMiddleware");
    assert!(rag_msg.is_none(), "should not inject for empty results");
}

#[tokio::test]
async fn test_static_mode_multiple_kbs_aggregates() {
    let kb1 = make_kb_with_data("kb1", vec![make_chunk("info from kb1", "doc1.txt", 0, 1)]).await;
    let kb2 = make_kb_with_data("kb2", vec![make_chunk("info from kb2", "doc2.txt", 0, 1)]).await;

    let mw = RAGMiddleware::new(vec![kb1, kb2], RAGMode::Static, 5, None);

    let mut input = Some(vec![make_user_msg("query")]);
    let model: Arc<dyn agent_scope_model::ChatModel> = Arc::new(MockChatModel::new("test-model"));

    mw.pre_reply("agent", &mut input, &model)
        .await
        .expect("pre_reply should succeed");

    let msgs = input.expect("input still present");
    let rag_msg = msgs.iter().find(|m| m.name == "RAGMiddleware");
    assert!(
        rag_msg.is_some(),
        "should aggregate results from multiple KBs"
    );

    // Verify content references both KBs
    if let Some(msg) = rag_msg {
        let text = msg
            .content
            .iter()
            .filter_map(|b| {
                if let ContentBlock::Text(tb) = b {
                    Some(tb.text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");
        assert!(text.contains("doc1.txt"));
        assert!(text.contains("doc2.txt"));
    }
}

// --- Agentic Mode Tests ---

#[tokio::test]
async fn test_agentic_mode_registers_tool_schemas() {
    let kb = make_kb_with_data(
        "hr-policies",
        vec![make_chunk("HR policy content", "hr.txt", 0, 1)],
    )
    .await;

    let mw = RAGMiddleware::new(vec![kb], RAGMode::Agentic, 5, None);

    let mut tools: Option<Vec<JsonValue>> = Some(vec![]);
    let mut messages = vec![];

    mw.pre_reasoning("agent", &mut messages, &mut tools)
        .await
        .expect("pre_reasoning should succeed");

    let tools = tools.expect("tools should still be Some");
    assert!(
        !tools.is_empty(),
        "agentic mode should register tool schemas"
    );

    // Verify the schema has the right structure
    let schema = &tools[0];
    assert_eq!(schema["type"], "function");
    assert_eq!(schema["function"]["name"], "search_hr_policies");
}

#[tokio::test]
async fn test_agentic_tool_execution() {
    let kb = make_kb_with_data(
        "docs",
        vec![make_chunk("important document content", "readme.txt", 0, 1)],
    )
    .await;

    let mw = RAGMiddleware::new(vec![kb.clone()], RAGMode::Agentic, 5, None);
    let search_tools = mw.into_search_tools();
    assert_eq!(search_tools.len(), 1);

    let tool = &search_tools[0];
    assert!(tool.name().contains("search_docs"));

    let result = tool
        .call(serde_json::json!({"query": "important"}))
        .await
        .expect("tool call should succeed");

    match result {
        agent_scope_tool::ToolExecOutput::Complete(block) => {
            if let agent_scope_message::ToolOutput::Text(ref t) = block.output {
                assert!(
                    t.contains("document content"),
                    "should contain chunk content: {t}"
                );
            } else {
                panic!("expected Text output");
            }
        }
        other => panic!("expected Complete, got {other:?}"),
    }
}

#[tokio::test]
async fn test_agentic_multi_kb_tools() {
    let kb1 = make_kb_with_data("alpha", vec![make_chunk("alpha content", "a.txt", 0, 1)]).await;
    let kb2 = make_kb_with_data("beta-docs", vec![make_chunk("beta content", "b.txt", 0, 1)]).await;

    let mw = RAGMiddleware::new(vec![kb1, kb2], RAGMode::Agentic, 5, None);
    let tools = mw.into_search_tools();
    assert_eq!(tools.len(), 2);

    let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
    assert!(names.iter().any(|n| n.contains("alpha")));
    assert!(names.iter().any(|n| n.contains("beta")));
}

// --- Mock ChatModel (minimal for test compilation) ---

struct MockChatModel {
    name: String,
}

impl MockChatModel {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ChatModel for MockChatModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn stream_enabled(&self) -> bool {
        false
    }

    async fn call_api(
        &self,
        _model_name: &str,
        _messages: &[Msg],
        _tools: Option<&[JsonValue]>,
        _tool_choice: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError> {
        unimplemented!()
    }
}

impl std::fmt::Debug for MockChatModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockChatModel")
            .field("name", &self.name)
            .finish()
    }
}
