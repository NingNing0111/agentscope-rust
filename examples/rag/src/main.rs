//! RAG example: knowledge base + retrieval-augmented generation.
//!
//! Builds a `KnowledgeBase` over a couple of in-memory documents, wraps it in a
//! `RAGMiddleware` (Static mode), and runs a ReActAgent whose system prompt
//! instructs it to answer from the knowledge base. Requires `DEFAULT_API_KEY`.

use std::sync::Arc;

use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_event::AgentEvent;
use agent_scope_message::factory::user_msg;
use agent_scope_rag::{Chunk, KnowledgeBase, RAGMiddleware, RAGMode, TurbovecVectorStore};
use agent_scope_rig::{RigChatModel, RigEmbeddingModel};
use clap::Parser;
use futures::StreamExt;

#[derive(Parser)]
struct Cli {
    #[arg(
        short,
        long,
        default_value = "项目使用的编程语言是什么？请基于知识库回答。"
    )]
    prompt: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    dotenv::dotenv().ok();

    let api_key = std::env::var("DEFAULT_API_KEY")
        .map_err(|_| anyhow::anyhow!("error: 缺少环境变量 DEFAULT_API_KEY。请设置后重试。"))?;

    // 1. Embedding model + vector store + knowledge base.
    //    模型名从 DEFAULT_EMBEDDING_MODEL 读取（fallback text-embedding-v4）；
    //    DEFAULT_URL 可选覆盖端点（与聊天模型一致）。
    let embedding_model_name = std::env::var("DEFAULT_EMBEDDING_MODEL")
        .unwrap_or_else(|_| "text-embedding-v4".to_string());
    let mut embedding_model = RigEmbeddingModel::openai(&api_key, &embedding_model_name)?;
    if let Ok(base_url) = std::env::var("DEFAULT_URL") {
        embedding_model = embedding_model.with_base_url(base_url);
    }
    let embedding = Arc::new(embedding_model);
    let vector_store = Arc::new(TurbovecVectorStore::new(4)?);
    let kb = Arc::new(KnowledgeBase::new(
        "project".into(),
        "Project documents for RAG retrieval.".into(),
        embedding,
        vector_store,
        "project".into(),
        None,
    ));

    // 2. Insert documents (as chunks).
    kb.insert_document(
        vec![Chunk {
            content: "AgentScope Rust uses the Rust programming language. The workspace is organized into agent_scope_* crates.".into(),
            source: "readme".into(),
            chunk_index: 0,
            total_chunks: 1,
            metadata: Default::default(),
        }],
        Some("readme".into()),
        None,
    )
    .await?;
    kb.insert_document(
        vec![Chunk {
            content: "The framework separates model, tool, memory, rag, and workspace concerns into independent crates.".into(),
            source: "architecture".into(),
            chunk_index: 0,
            total_chunks: 1,
            metadata: Default::default(),
        }],
        Some("architecture".into()),
        None,
    )
    .await?;
    println!("inserted 2 documents into knowledge base");

    // 3. RAG middleware (Static: inject retrieved context on every turn).
    let rag = Arc::new(RAGMiddleware::new(
        vec![kb.clone()],
        RAGMode::Static,
        3,
        None,
    ));

    // 4. Chat model + agent.
    //    模型名从 DEFAULT_CHAT_MODEL 读取（fallback qwen3.7-plus）；DEFAULT_URL 可选覆盖端点。
    let model_name =
        std::env::var("DEFAULT_CHAT_MODEL").unwrap_or_else(|_| "qwen3.7-plus".to_string());
    let mut model = RigChatModel::openai(&api_key, &model_name)?;
    if let Ok(base_url) = std::env::var("DEFAULT_URL") {
        model = model.with_base_url(base_url);
    }
    let model = Arc::new(model.with_stream(true));
    let config = AgentConfig::builder()
        .name("assistant")
        .system_prompt("你是基于本地知识库回答的助手。请优先根据知识库内容回答。")
        .model(model)
        .build()?;

    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![rag],
    )?;

    // 5. Reply and stream.
    let msg = user_msg("user", &cli.prompt).map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let mut stream = agent.reply_stream(Some(vec![msg])).await?;
    while let Some(event) = stream.next().await {
        if let AgentEvent::TextBlockDelta(d) = &event {
            print!("{}", d.delta);
        }
    }
    println!();

    Ok(())
}
