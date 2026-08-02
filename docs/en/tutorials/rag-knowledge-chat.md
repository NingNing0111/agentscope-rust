# RAG Knowledge-Base Chat Tutorial

> Build a Q&A Agent with knowledge-base retrieval from scratch.

## Prerequisites

- Rust toolchain (edition 2024)
- DashScope API Key (set via `API_KEY` environment variable)
- One or more text documents as knowledge sources

## Step 1: Create the Project

```bash
cargo new rag-chat
cd rag-chat
```

Add dependencies to `Cargo.toml`:

```toml
[dependencies]
agent_scope_agent = { path = "../../crates/agent_scope_agent" }
agent_scope_dashscope = { path = "../../crates/agent_scope_dashscope" }
agent_scope_rag = { path = "../../crates/agent_scope_rag" }
agent_scope_embedding = { path = "../../crates/agent_scope_embedding" }
agent_scope_message = { path = "../../crates/agent_scope_message" }
tokio = { version = "1", features = ["full"] }
```

This assumes your project lives inside the AgentScope Rust repository. For external use, replace with git dependencies or local paths.

## Step 2: Create a ChatModel

```rust
use agent_scope_dashscope::DashScopeChatModel;
use std::sync::Arc;
use std::env;

fn create_model() -> Arc<DashScopeChatModel> {
    let api_key = env::var("API_KEY").expect("API_KEY not set");
    Arc::new(DashScopeChatModel::new(
        api_key,
        "qwen-plus".into(),
    ))
}
```

## Step 3: Load Documents and Build a Knowledge Base

```rust
use agent_scope_rag::{
    TextParser, Parser,
    ApproxTokenChunker, Chunker,
    TurbovecVectorStore, KnowledgeBase,
};
use agent_scope_dashscope::DashScopeEmbeddingModel;
use agent_scope_embedding::EmbeddingModelCard;
use std::path::Path;

async fn build_knowledge_base(
    embedding_model: Arc<DashScopeEmbeddingModel>,
) -> Result<KnowledgeBase, Box<dyn std::error::Error>> {
    // 1. Create vector store
    let store = Arc::new(TurbovecVectorStore::in_memory(embedding_model.card().dimension));

    // 2. Read a document
    let content = tokio::fs::read_to_string("docs/knowledge.md").await?;

    // 3. Parse + chunk
    let parser = TextParser::new();
    let chunker = ApproxTokenChunker::new(512, 64);
    let sections = parser.parse(&content)?;
    let chunks = chunker.chunk(sections)?;

    // 4. Build knowledge base
    let kb = KnowledgeBase::new(
        "my-knowledge".into(),
        "My domain knowledge".into(),
        embedding_model.clone() as Arc<_>,
        store,
        "default".into(),
        None,
    );

    // 5. Insert documents (lazy init — collection auto-created on first operation)
    kb.insert_documents(chunks).await?;
    Ok(kb)
}
```

## Step 4: Build RAG Middleware

```rust
use agent_scope_rag::{RAGMiddleware, RAGMode};
use std::sync::Arc;

fn create_rag_middleware(
    kb: KnowledgeBase,
    embedding_model: Arc<DashScopeEmbeddingModel>,
) -> Arc<RAGMiddleware> {
    Arc::new(RAGMiddleware::new(
        RAGMode::Dynamic,  // Dynamic retrieval per request
        vec![kb],
        embedding_model as Arc<dyn agent_scope_embedding::EmbeddingModel>,
    ))
}
```

**Mode selection:**
- `RAGMode::Static` — Inject all knowledge into system prompt at initialization; best for small, static knowledge bases
- `RAGMode::Dynamic` — On-demand retrieval per input; best for large or frequently changing knowledge bases

## Step 5: Create a ReActAgent

```rust
use agent_scope_agent::{AgentConfig, ReActAgent, ReActConfig, ContextConfig, Middleware};

async fn create_agent(
    model: Arc<DashScopeChatModel>,
    rag: Arc<RAGMiddleware>,
) -> Result<ReActAgent, Box<dyn std::error::Error>> {
    let config = AgentConfig::builder()
        .name("rag-assistant")
        .system_prompt("You are a knowledge base Q&A assistant. Use the kb_search tool to query the knowledge base when answering questions. If the knowledge base has no relevant information, say so honestly.")
        .model(model)
        .build()?;

    ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![rag as Arc<dyn Middleware>],
    )
}
```

## Step 6: Ask Questions

```rust
use agent_scope_agent::Agent;
use agent_scope_message::factory::user_msg;

async fn ask(agent: &ReActAgent, question: &str) -> String {
    let reply = agent
        .reply(Some(vec![user_msg("user", question).unwrap()]))
        .await
        .unwrap();
    
    reply.get_text_content(" ").unwrap_or_else(|| "No response".into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("API_KEY")?;
    let model = Arc::new(DashScopeChatModel::new(api_key.clone(), "qwen-plus".into()));
    let embedding = Arc::new(DashScopeEmbeddingModel::new(api_key, "text-embedding-v2".into()));

    let kb = build_knowledge_base(embedding.clone()).await?;
    let rag = create_rag_middleware(kb, embedding);
    let agent = create_agent(model, rag).await?;

    let answer = ask(&agent, "What is our remote work policy?").await;
    println!("Answer: {}", answer);

    Ok(())
}
```

## Step 7: Observe RAG Retrieval in Action

Use `reply_stream()` to see the RAG middleware workflow:

```rust
use futures::StreamExt;
use agent_scope_event::AgentEvent;

let mut stream = agent.reply_stream(Some(vec![user_msg("user", "What are OKRs?").unwrap()]));
while let Some(event) = stream.next().await {
    match event {
        AgentEvent::TextBlockDelta(ev) => {
            print!("{}", ev.delta);
        }
        AgentEvent::ToolCallStart(ev) => {
            println!("\n🔧 Tool call: {}", ev.tool_name);
        }
        AgentEvent::ToolResultBlock(ev) => {
            println!("📄 Retrieved {} results", ev.content.len());
        }
        _ => {}
    }
}
```

## Key Takeaways

1. **Lazy initialization** — `KnowledgeBase` auto-creates the vector store collection on the first `insert_documents()` or `search()` call
2. **Multi-knowledge-base** — `RAGMiddleware` can manage multiple knowledge bases, each generating a separate `kb_search_<name>` tool
3. **Tool naming** — The knowledge base `name` field directly determines the tool name the Agent sees
4. **Chunking strategy** — `chunk_size=512`, `chunk_overlap=64` is a common starting point; adjust based on document characteristics

## Next Steps

- [Agent System Docs](../modules/agent.md) — Deep dive into ReActAgent
- [RAG Module Docs](../modules/rag.md) — Complete RAG system reference
- [Workspace Docs](../modules/workspace.md) — Placing knowledge base files inside a workspace
