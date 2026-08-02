# RAG Retrieval-Augmented Generation / RAG

> One-liner: `agent_scope_rag` provides a complete RAG pipeline covering document parsing, chunking, vector storage, and knowledge-base retrieval — using `Parser` to parse documents, `Chunker` to split text, `VectorStore` for vector storage, `KnowledgeBase` to manage knowledge bases, and `RAGMiddleware` to inject retrieval into the Agent reply lifecycle.

## 1. Module Overview (Overview)

| Component | Responsibility |
|-----------|---------------|
| `Parser` / `TextParser` | Document parsing: plain text, Markdown, outputting structured `Section` lists |
| `Chunker` / `ApproxTokenChunker` | Splitting Sections into model-friendly `Chunk`s |
| `VectorStore` trait / `TurboVecVectorStore` | Vector storage backend: insert, search, delete, list |
| `KnowledgeBase` | Runtime knowledge base wrapping EmbeddingModel + VectorStore |
| `RAGMiddleware` | Agent middleware triggering knowledge base retrieval at `pre_reply` |
| `TurbovecIndexAdapter` | Bridges TurboVec memory index to the VectorStore trait |

**When to use**: document-based Q&A; injecting private knowledge bases into Agents; providing domain knowledge retrieval for Agents.

**Prerequisites**: [Model Abstraction](./model.md), [Agent System](./agent.md), [Memory](./memory.md)

## 2. Core Concepts & Main Public Types (Core Concepts)

### 2.1 Document Parsing (`parser`)

`Parser` trait definition:

```rust
pub trait Parser: Send + Sync {
    fn parse(&self, content: &str) -> Result<Vec<Section>, ParserError>;
}
```

`TextParser` supports two modes:
- **Plain text**: splits by natural paragraphs
- **Markdown**: splits by heading levels (`#`, `##`), preserving structure

`Section` contains `SectionContent` enum: `Title` (heading node) and `Body` (content node).

### 2.2 Text Chunking (`chunker`)

`Chunker` trait:

```rust
pub trait Chunker: Send + Sync {
    fn chunk(&self, sections: Vec<Section>) -> Result<Vec<Chunk>, ChunkerError>;
}
```

`ApproxTokenChunker` splits based on approximate token counts:
- `chunk_size`: target token count per chunk
- `chunk_overlap`: overlap token count between chunks
- Splits at paragraph boundaries (paragraph-aware), avoiding mid-sentence breaks

`Chunk` contains:
| Field | Description |
|-------|-------------|
| `id` | Unique chunk ID |
| `content` | Chunk text content |
| `metadata` | Source document info |

### 2.3 Vector Storage (`vector_store`)

```rust
pub trait VectorStore: Send + Sync {
    async fn insert(&self, records: Vec<VectorRecord>) -> Result<(), VectorStoreError>;
    async fn search(&self, query: &[f64], top_k: usize) -> Result<Vec<VectorSearchResult>, VectorStoreError>;
    async fn delete(&self, ids: &[String]) -> Result<(), VectorStoreError>;
    async fn list(&self) -> Result<Vec<DocumentSummary>, VectorStoreError>;
}
```

`TurboVecVectorStore` is a high-performance implementation based on the `turbovec` library, supporting:
- Lazy collection creation
- Vector similarity search
- Calibration state management (`CalibrationState`)

### 2.4 Knowledge Base (`KnowledgeBase`)

```rust
pub struct KnowledgeBase {
    pub name: String,           // Knowledge base name
    pub description: String,    // Description (used in Agent context)
    // embedding_model + vector_store + collection + metadata_filter
}
```

Core methods:
| Method | Description |
|--------|-------------|
| `search(query, top_k)` | Natural language query with auto embedding + vector search |
| `insert_documents(sections)` | Chunk → vectorize → store |
| `delete_documents(source)` | Delete documents by source |
| `list_documents()` | List all document summaries in the knowledge base |

### 2.5 `RAGMiddleware`

Connects knowledge bases to the Agent lifecycle:

| Mode | Behavior |
|------|----------|
| `Static` | Injects knowledge base content into system prompt at initialization |
| `Dynamic` | Retrieves with user query at each `pre_reply`, injects as `HintBlock` |

### 2.6 `TurbovecIndexAdapter`

Bridges `agent_scope_memory`'s `MemoryVectorIndex` to the `VectorStore` trait, enabling interoperability between the memory system and the RAG system.

## 3. Quick Example (Quick Example)

```rust
use agent_scope_rag::{TextParser, ApproxTokenChunker, KnowledgeBase, RAGMiddleware};
use agent_scope_embedding::EmbeddingModel; // your embedding impl

// 1. Create parser and chunker
let parser = TextParser::new();
let chunker = ApproxTokenChunker::new(512, 64);

// 2. Load documents
let content = std::fs::read_to_string("docs/company-policy.md")?;
let sections = parser.parse(&content)?;
let chunks = chunker.chunk(sections)?;

// 3. Build knowledge base
let kb = KnowledgeBase::new(
    "company-policy",
    "Company HR policies and guidelines",
    embedding_model,
    vector_store,
    "default".into(),
    None,
);
kb.insert_documents(chunks).await?;

// 4. Query
let results = kb.search("What is our remote work policy?", 3).await?;
```

## 4. Key Usage Patterns (Usage Patterns)

### 4.1 Document Preprocessing Pipeline

```
Raw doc → Parser → [Section, Section, ...] → Chunker → [Chunk, Chunk, ...]
→ EmbeddingModel → [VectorRecord, ...] → VectorStore.insert()
```

### 4.2 Connecting RAG to an Agent

```rust
use agent_scope_rag::{RAGMiddleware, RAGMode};
use std::sync::Arc;

let rag = Arc::new(RAGMiddleware::new(
    RAGMode::Dynamic,
    vec![kb],
    embedding_model,
));
let agent = ReActAgent::new(
    config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![rag],
)?;
```

### 4.3 Multi-Knowledge-Base Support

`RAGMiddleware` can manage multiple knowledge bases, generating a separate Tool for each:

```rust
let kbs = vec![hr_kb, tech_kb, legal_kb];
let rag = RAGMiddleware::new(RAGMode::Dynamic, kbs, embedding_model);
// Agent sees tools like kb_search_hr, kb_search_tech, kb_search_legal
```

### 4.4 Static vs Dynamic Mode

| Mode | When to use | Trade-off |
|------|-------------|-----------|
| `Static` | Small, unchanging knowledge base | Full knowledge in every request, no retrieval latency |
| `Dynamic` | Large or frequently changing knowledge base | On-demand retrieval, saves context, adds one embedding call |

## 5. Errors & Unsupported Capabilities (Errors & Unsupported)

| Error | Cause |
|-------|-------|
| `ParserError::UnsupportedFormat` | Document format not supported |
| `ParserError::ParseError` | Parse failure |
| `ChunkerError::EmptyInput` | Empty input |
| `ChunkerError::ChunkSizeTooSmall` | Invalid chunk_size configuration |
| `VectorStoreError::CollectionNotFound` | Collection not created |
| `VectorStoreError::InsertError` | Vector insertion failure |
| `KnowledgeBaseError::NotInitialized` | Knowledge base not initialized |

**Unsupported**:
- Multi-modal documents (PDF, images) parsing is not supported in the current version
- Incremental updates (updating only changed documents) are not implemented
- Vector storage is in-process; distributed storage is not supported

## 6. Compatibility (Compatibility)

- **Compatibility Level**: **L2** (core RAG behavior)
- **Authority**: `specs/011-rag-system/spec.md`, `specs/016-turbovec-rag/spec.md`
- **Known Deviations**:
  - `TurboVecVectorStore` is a Rust-specific high-performance implementation
  - Rust-side Parser/Chunker currently only supports text and Markdown; Python side supports more formats

## 7. See Also (Related Modules)

- [Model Abstraction](./model.md) — EmbeddingModel and ChatModel usage in RAG
- [Agent System](./agent.md) — RAGMiddleware's position in the Agent lifecycle
- [Memory](./memory.md) — Memory vs RAG (long-term memory vs document knowledge base)
- [DashScope Provider](./dashscope.md) — embedding model source
