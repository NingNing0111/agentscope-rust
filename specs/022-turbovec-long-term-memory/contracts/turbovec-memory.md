# Contract: TurbovecMemory

**Feature**: 022-turbovec-long-term-memory
**Contract Type**: Rust struct + Memory trait implementation
**Stability**: New (may evolve)
**Depends on**: `agent_scope_memory::Memory`, `agent_scope_embedding::EmbeddingModel`, `agent_scope_rag::TurbovecVectorStore`

## Core API

```rust
pub struct TurbovecMemory { /* private fields */ }

impl TurbovecMemory {
    /// Create a new TurboVec-backed long-term memory store.
    ///
    /// - `workdir` — working directory for resolving relative memory_dir paths
    /// - `config` — memory + vector index configuration
    /// - `embedding_model` — model for text→vector conversion
    /// - `backend` — optional storage backend (defaults to `LocalBackend`)
    ///
    /// On construction:
    /// - Creates memory_dir if it doesn't exist (via FileMemory delegate)
    /// - Attempts to load existing `.turbovec/` vector index
    /// - If index is missing and `config.auto_rebuild` is true, triggers rebuild
    pub async fn new(
        workdir: &str,
        config: TurbovecMemoryConfig,
        embedding_model: Arc<dyn EmbeddingModel>,
        backend: Option<Arc<dyn Backend>>,
    ) -> Result<Self, MemoryError>;

    /// Semantic search via TurboVec vector index.
    ///
    /// Embeds the query, searches the vector index, deduplicates by memory name,
    /// and returns ranked results with bounded content.
    ///
    /// **Preconditions**:
    /// - `query` is non-empty
    /// - `top_k > 0`
    ///
    /// **Postconditions**:
    /// - Results sorted by cosine similarity score descending
    /// - Equal scores tie-break by `memory_name` ascending
    /// - Content truncated to `retrieval_max_tokens_per_file`
    /// - Empty collection returns empty Vec (not error)
    ///
    /// **Errors**:
    /// - `ValidationError` if query is empty or top_k is 0
    /// - `SemanticIndexError` if embedding model fails or vector store is corrupted
    pub async fn semantic_search(
        &self,
        query: &str,
        type_filter: Option<MemoryType>,
        top_k: usize,
    ) -> Result<Vec<MemorySearchResult>, MemoryError>;

    /// Rebuild the TurboVec vector index from Markdown memory files.
    ///
    /// Reads all `.md` files via FileMemory, generates embeddings, and replaces
    /// the entire vector collection. Markdown files that fail to parse are
    /// skipped and reported in the rebuild report.
    ///
    /// **Postconditions**:
    /// - Previous collection (if any) is dropped and replaced
    /// - New collection contains one vector per successfully indexed memory entry
    /// - `MemoryRebuildReport` details scanned/indexed/skipped/error counts
    ///
    /// **Errors**:
    /// - `SemanticIndexError` if the embedding model fails for all entries
    pub async fn rebuild_index(&self) -> Result<MemoryRebuildReport, MemoryError>;

    /// Persist the vector index to `{memory_dir}/.turbovec/`.
    ///
    /// Delegates to `TurbovecVectorStore::save()`.
    /// Directory is created if it doesn't exist.
    pub async fn save_index(&self) -> Result<(), MemoryError>;

    /// Access the underlying FileMemory delegate.
    pub fn file_memory(&self) -> &FileMemory;
}

// Memory trait implementation
#[async_trait::async_trait]
impl Memory for TurbovecMemory {
    // All methods delegate to FileMemory for Markdown storage,
    // plus vector index synchronization.
}
```

## Memory trait method contracts (TurbovecMemory implementation)

### write()

**Preconditions**:
- `entry.name` is non-empty and matches `[A-Za-z0-9_-]+`
- `entry.description` is non-empty

**Postconditions**:
- Markdown file written via `FileMemory::write()` (includes MEMORY.md index update)
- Memory content embedded via `EmbeddingModel::embed()`
- Old vector records for `document_id = entry.name` deleted from TurboVec collection
- New vector record inserted with metadata: `memory_name`, `memory_type`, `source`, `updated_at`

**Errors**:
- `ValidationError` if name/description invalid
- `IoError` if file write fails
- `SemanticIndexError` if embedding model fails

### delete()

**Preconditions**: None

**Postconditions**:
- Markdown file removed via `FileMemory::delete()`
- All vector records for `document_id = <name>` removed from TurboVec collection
- If file doesn't exist, operation is idempotent (no-op for both file and vector)

**Errors**:
- `IoError` if file deletion fails for non-existence reasons
- `SemanticIndexError` if vector store operation fails

### retrieve_relevant()

**Preconditions**:
- `query` is non-empty
- `max_results` is in `1..=retrieval_max_files`

**Postconditions**:
- Calls `semantic_search(query, None, max_results)` internally
- Formats results as markdown string ready for `HintBlock` injection
- Returns `None` if no relevant memories found
- Embedding model failure returns `None` (fail-open per Constitution §13)

**This method diverges from FileMemory**: TurboVec uses vector similarity instead of LLM file selection. The `model` parameter is accepted for trait compatibility but not used for file selection in this implementation.
