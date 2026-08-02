# Contract: Semantic Memory Index

**Feature**: 022-turbovec-long-term-memory
**Contract Type**: Data mapping + index contract
**Stability**: New (may evolve)
**Depends on**: `agent_scope_rag::TurbovecVectorStore`, `agent_scope_embedding::EmbeddingModel`, `agent_scope_memory::MemoryEntry`

## Mapping: MemoryEntry → VectorRecord

Each `MemoryEntry` maps to exactly one `VectorRecord` in the TurboVec collection.

### document_id

`document_id = memory_entry.name`

- Stable across upserts: writing the same name reuses the same document_id
- On write: old records with this document_id are deleted, then new record is inserted
- On delete: all records with this document_id are removed

### VectorRecord Fields

| VectorRecord field | Source | Description |
|-------------------|--------|-------------|
| `vector` | `EmbeddingModel::embed(entry.content)` | Normalized f32 embedding vector |
| `document_id` | `entry.name` | Stable memory identity |
| `chunk.document_id` | `entry.name` | Same as above (Chunk convention) |
| `chunk.content` | `entry.content` | Full memory body |
| `chunk.metadata` | See below | Memory identity and filtering metadata |

### chunk.metadata Schema

| Key | Value Source | Required | Description |
|-----|-------------|----------|-------------|
| `memory_name` | `entry.name` | Yes | Stable memory name |
| `memory_type` | `entry.metadata.mem_type.as_str()` | Yes | One of: user, feedback, project, reference |
| `source` | `"{entry.name}.md"` | Yes | Markdown filename |
| `updated_at` | `entry.metadata.updated_at` | Yes | RFC 3339 timestamp |

### Type Filter Mapping

When `type_filter: Some(MemoryType)` is specified:

```rust
let mut metadata_filter = HashMap::new();
metadata_filter.insert("memory_type".to_string(), type_filter.as_str().to_string());
```

This becomes the `metadata_filter` parameter to `VectorStore::search()`, which performs exact-match AND filtering.

### Dimension Contract

- `embedding_model.model_card().dimensions` determines the TurboVec collection dimension
- Collection is created lazily on first `write()` (via `VectorStore::insert()` auto-create)
- If embedding model dimensions change (e.g., after config update), existing index is incompatible
- Detection: `ensure_collection()` checks `dim` vs stored; mismatch → `SemanticIndexError` with "rebuild needed"

## Consistency Guarantees

### write() Consistency

1. FileMemory::write() succeeds → Markdown file persisted
2. Old vector records deleted (if any)
3. New embedding generated + inserted
4. If step 2-3 fails: Markdown file still valid (source of truth), vector may be stale
5. Caller can detect via `rebuild_index()` → restores consistency

### delete() Consistency

1. TurboVec document_id records deleted
2. FileMemory::delete() removes Markdown file
3. If step 1 fails: file still deleted, orphan vectors remain (harmless — filtered by file existence check on rebuild)
4. If step 2 fails (file delete error): vectors already removed, caller gets error

### Rebuild Behavior

`rebuild_index()`:
1. Lists all `.md` files via `FileMemory::list()`
2. For each file: reads content → parses frontmatter → embeds text → builds VectorRecord
3. Malformed files: skipped, counted in `MemoryRebuildReport.skipped`
4. Embedding failures: per-file error recorded, continues to next file
5. Drops old TurboVec collection, creates new one with all successfully indexed records
6. Saves new index to `.turbovec/`

### Search During Write

- `semantic_search()` reads from `TurbovecVectorStore` with `RwLock::read()`
- `write()` acquires `RwLock::write()` for collection mutation
- Callers observe either pre-write or post-write state, not partial
- No explicit transaction across FileMemory + TurboVec

## Search Result Formatting

### MemorySearchResult → retrieve_relevant() Output

```markdown
### {memory_name} (saved {age_label})
Description: {description}
Type: {memory_type}

{truncated_content}
```

Results are joined with double newline, matching Feature 009 `retrieve_relevant()` output format.
