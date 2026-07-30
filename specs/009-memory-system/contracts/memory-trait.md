# Contract: Memory Trait

**Feature**: 009-memory-system  
**Contract Type**: Rust trait interface  
**Stability**: New (may evolve)

## Trait Definition

```rust
use std::sync::Arc;
use agent_scope_model::ChatModel;

#[async_trait::async_trait]
pub trait Memory: Send + Sync {
    /// Write a memory entry (upsert).
    /// Creates or updates `MEMORY.md` index line.
    async fn write(&self, entry: MemoryEntry) -> Result<(), MemoryError>;

    /// Read a memory entry by name (filename without .md).
    /// Returns `None` if not found.
    async fn read(&self, name: &str) -> Result<Option<MemoryEntry>, MemoryError>;

    /// Delete a memory entry and its index line.
    /// Returns error if the file cannot be deleted (e.g., permission).
    async fn delete(&self, name: &str) -> Result<(), MemoryError>;

    /// List all memory file headers (metadata only, no content).
    /// Returns newest-first by modification time.
    async fn list(&self) -> Result<Vec<MemoryFileHeader>, MemoryError>;

    /// Search memory entries by substring match on content and description.
    /// Optional type filter restricts results to one memory type.
    async fn search(
        &self,
        query: &str,
        type_filter: Option<MemoryType>,
    ) -> Result<Vec<MemoryEntry>, MemoryError>;

    /// Get the MEMORY.md index content, truncated to max_index_tokens.
    async fn get_index_content(&self) -> Result<String, MemoryError>;

    /// Use a ChatModel to select memory files relevant to a user query.
    /// Returns formatted content of selected files, or None if nothing relevant.
    /// The model is passed explicitly (not stored) to allow the caller to choose.
    async fn retrieve_relevant(
        &self,
        query: &str,
        model: &Arc<dyn ChatModel>,
        max_results: usize,
    ) -> Result<Option<String>, MemoryError>;
}
```

## Method Contracts

### write()

**Preconditions**:
- `entry.name` is non-empty and filesystem-safe (`[a-zA-Z0-9_-]+`).
- `entry.description` is non-empty.

**Postconditions**:
- A `.md` file exists at `<memory_dir>/<entry.name>.md` with frontmatter + content.
- `MEMORY.md` index contains one line: `- [<entry.name>](<entry.name>.md) — <entry.description>`.
- If `entry.name` already exists, the old file is replaced and the index line is updated.

**Errors**:
- `ValidationError` if `name` or `description` is empty.
- `IoError` if file write fails.

### read()

**Preconditions**: None.

**Postconditions**:
- Returns `Some(MemoryEntry)` if a file `<memory_dir>/<name>.md` exists and parses correctly.
- Returns `None` if the file does not exist.
- Returns `None` (not error) if the file exists but frontmatter is malformed (graceful skip).

**Errors**:
- `IoError` if directory traversal fails.

### delete()

**Preconditions**: None.

**Postconditions**:
- File `<memory_dir>/<name>.md` is removed.
- Corresponding index line in `MEMORY.md` is removed.
- If file doesn't exist, operation is a no-op (idempotent).

**Errors**:
- `IoError` if file deletion fails for non-existence reasons (permission).

### list()

**Preconditions**: Memory directory exists (created idempotently on first access).

**Postconditions**:
- Returns metadata for all `.md` files in memory directory (excluding `MEMORY.md`).
- Results sorted newest-first by mtime.
- Capped at `retrieval_max_files` entries.
- Malformed files are skipped (not errored).

**Errors**:
- `IoError` if directory listing fails.

### search()

**Preconditions**: None.

**Postconditions**:
- Returns entries where `query` appears in `content` OR `description` (case-insensitive substring match).
- If `type_filter` is `Some(t)`, only entries of type `t` are returned.
- Results include full content.

**Errors**:
- `IoError` if file reads fail.

### get_index_content()

**Preconditions**: Memory directory exists.

**Postconditions**:
- Returns `MEMORY.md` content as a string.
- If content exceeds `max_index_tokens`, truncated from the end with truncation notice appended.
- Returns empty string if `MEMORY.md` doesn't exist (first access, not an error).

**Errors**:
- `IoError` if file read fails.

### retrieve_relevant()

**Preconditions**:
- `query` is non-empty.
- `model` is a valid `ChatModel` implementation.
- `max_results` is in `1..=retrieval_max_files`.

**Postconditions**:
- Calls `model.generate_structured_output()` with memory file manifest and query.
- Validates returned filenames exist (filters hallucinated names).
- Reads selected files, truncates each to `retrieval_max_tokens_per_file`.
- Returns formatted string ready for `HintBlock` injection, or `None` if nothing selected.

**Errors**:
- `ValidationError` if `query` is empty.
- `RetrievalError` if model call fails (errors are NOT propagated — return `None` instead per FR edge case).
