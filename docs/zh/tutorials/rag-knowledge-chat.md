# RAG 知识库问答教程

> 从零构建一个带知识库检索能力的问答 Agent。

## 前提准备

- Rust 工具链（edition 2024）
- DashScope API Key（环境变量 `API_KEY`）
- 一个或多个文本文档作为知识源

## 第一步：创建项目

```bash
cargo new rag-chat
cd rag-chat
```

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
agent_scope_agent = { path = "../../crates/agent_scope_agent" }
agent_scope_dashscope = { path = "../../crates/agent_scope_dashscope" }
agent_scope_rag = { path = "../../crates/agent_scope_rag" }
agent_scope_embedding = { path = "../../crates/agent_scope_embedding" }
agent_scope_message = { path = "../../crates/agent_scope_message" }
tokio = { version = "1", features = ["full"] }
```

假设你的项目在 AgentScope Rust 仓库内部。如果外部使用，请用 git 依赖或本地路径替换。

## 第二步：创建 ChatModel

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

## 第三步：加载文档并构建知识库

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
    // 1. 创建向量存储
    let store = Arc::new(TurbovecVectorStore::in_memory(embedding_model.card().dimension));

    // 2. 读取文档
    let content = tokio::fs::read_to_string("docs/knowledge.md").await?;

    // 3. 解析 + 分块
    let parser = TextParser::new();
    let chunker = ApproxTokenChunker::new(512, 64);
    let sections = parser.parse(&content)?;
    let chunks = chunker.chunk(sections)?;

    // 4. 构建知识库
    let kb = KnowledgeBase::new(
        "my-knowledge".into(),
        "My domain knowledge".into(),
        embedding_model.clone() as Arc<_>,
        store,
        "default".into(),
        None,
    );

    // 5. 插入文档（懒初始化，首次操作时自动创建 collection）
    kb.insert_documents(chunks).await?;
    Ok(kb)
}
```

## 第四步：构建 RAG Middleware

```rust
use agent_scope_rag::{RAGMiddleware, RAGMode};
use std::sync::Arc;

fn create_rag_middleware(
    kb: KnowledgeBase,
    embedding_model: Arc<DashScopeEmbeddingModel>,
) -> Arc<RAGMiddleware> {
    Arc::new(RAGMiddleware::new(
        RAGMode::Dynamic,  // 每次请求动态检索
        vec![kb],
        embedding_model as Arc<dyn agent_scope_embedding::EmbeddingModel>,
    ))
}
```

**模式选择：**
- `RAGMode::Static` — 初始化时将全部知识注入系统提示词，适合小知识库
- `RAGMode::Dynamic` — 每次输入时按需检索，适合大知识库或频繁变化的文档

## 第五步：创建 ReActAgent

```rust
use agent_scope_agent::{AgentConfig, ReActAgent, ReActConfig, ContextConfig, Middleware};

async fn create_agent(
    model: Arc<DashScopeChatModel>,
    rag: Arc<RAGMiddleware>,
) -> Result<ReActAgent, Box<dyn std::error::Error>> {
    let config = AgentConfig::builder()
        .name("rag-assistant")
        .system_prompt("你是一个知识库问答助手。使用 kb_search 工具查询知识库来回答问题。如果知识库中没有相关信息，请如实告知。")
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

## 第六步：提问

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

    let answer = ask(&agent, "我们的远程办公政策是什么？").await;
    println!("回答: {}", answer);

    Ok(())
}
```

## 第七步：观察 RAG 检索过程

使用 `reply_stream()` 可以看到 RAG middleware 的工作流程：

```rust
use futures::StreamExt;
use agent_scope_event::AgentEvent;

let mut stream = agent.reply_stream(Some(vec![user_msg("user", "什么是OKR？").unwrap()]));
while let Some(event) = stream.next().await {
    match event {
        AgentEvent::TextBlockDelta(ev) => {
            print!("{}", ev.delta);
        }
        AgentEvent::ToolCallStart(ev) => {
            println!("\n🔧 调用工具: {}", ev.tool_name);
        }
        AgentEvent::ToolResultBlock(ev) => {
            println!("📄 检索到 {} 条结果", ev.content.len());
        }
        _ => {}
    }
}
```

## 关键要点

1. **懒初始化** — `KnowledgeBase` 在首次 `insert_documents()` 或 `search()` 时自动创建向量存储 collection
2. **多知识库** — `RAGMiddleware` 可以管理多个知识库，每个生成一个独立的 `kb_search_<name>` 工具
3. **工具名称** — 知识库的 `name` 字段直接决定 Agent 看到的工具名称
4. **分块策略** — `chunk_size=512`、`chunk_overlap=64` 是常用起点，按文档特性调整

## 下一步

- [Agent 系统文档](../modules/agent.md) — 深入了解 ReActAgent
- [RAG 模块文档](../modules/rag.md) — RAG 系统完整参考
- [工作空间文档](../modules/workspace.md) — 将知识库文件放入工作空间
