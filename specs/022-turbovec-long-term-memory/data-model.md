# Data Model: TurboVec Long-Term Memory

**Feature**: 022-turbovec-long-term-memory | **Date**: 2026-08-02

## Entity Relationship

```
MemoryEntry (from Feature 009)
    │
    ▼
TurbovecMemory ──────────────────────────────────────
    │                                                 │
    ├── FileMemory (delegate)                        │
    │   ├── Backend (trait)                          │
    │   └── MemoryConfig                             │
    │                                                 │
    ├── EmbeddingModel (trait, from embedding crate) │
    │   └── EmbeddingModelCard                       │
    │                                                 │
    └── TurbovecVectorStore (from rag crate)          │
        ├── collection: "memories"                   │
        ├── bit_width: 2|3|4                         │
        └── CollectionInner                          │
            ├── IdMapIndex (turbovec)                │
            ├── chunk_meta: HashMap<u64, ChunkMetaEntry>
            └── doc_index: HashMap<String, Vec<u64>>
```

## Core Entities

### TurbovecMemory

TurboVec-backed long-term memory implementation of `Memory` trait.

| Field | Type | Description |
|-------|------|-------------|
| `file_memory` | `FileMemory` | Delegate for CRUD on Markdown files |
| `vector_store` | `Arc<TurbovecVectorStore>` | Shared vector index |
| `embedding_model` | `Arc<dyn EmbeddingModel>` | Model for text→vector |
| `config` | `TurbovecMemoryConfig` | Extended memory config |
| `collection_name` | `String` | TurboVec collection name (default: "memories") |

**Lifecycle**:
1. Construct with memory dir, embedding model, config
2. `load_or_init()` — load existing `.turbovec/` index or initialize empty
3. On `write()` — persist Markdown via `FileMemory`, then embed + insert to TurboVec
4. On `delete()` — remove Markdown file, delete TurboVec document_id
5. On `retrieve_relevant()` — embed query → TurboVec search → format results
6. On `rebuild_index()` — read all Markdown files → re-embed → replace TurboVec collection

**Validation**:
- `TurbovecMemoryConfig::validate()` — checks positive limits, valid bit_width, non-empty collection_name

### TurbovecMemoryConfig

Extended configuration for TurboVec-backed memory.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `memory_dir` | `String` | `"Memory"` | Memory files directory |
| `max_index_tokens` | `usize` | `4000` | Max MEMORY.md index tokens |
| `retrieval_async` | `bool` | `true` | Async retrieval toggle |
| `retrieval_max_files` | `usize` | `200` | Max listed files |
| `retrieval_max_tokens_per_file` | `usize` | `2000` | Max tokens per memory file |
| `retrieval_max_tokens_per_frontmatter` | `usize` | `256` | Max frontmatter tokens |
| `memory_instructions` | `String` | `DEFAULT_MEMORY_INSTRUCTIONS` | System prompt text |
| `retrieval_instructions` | `String` | `DEFAULT_RETRIEVAL_INSTRUCTIONS` | Retrieval prompt text |
| `bit_width` | `usize` | `4` | TurboVec compression (2/3/4) |
| `collection_name` | `String` | `"memories"` | TurboVec collection name |
| `retrieval_top_k` | `usize` | `10` | Max vector search results |
| `retrieval_score_threshold` | `Option<f32>` | `None` | Min similarity threshold |
| `auto_rebuild` | `bool` | `false` | Auto-rebuild on index mismatch |
| `vector_index_dir` | `String` | `".turbovec"` | Vector index subdirectory (relative to memory_dir) |

**Validation rules**:
- `bit_width ∈ {2, 3, 4}`
- `collection_name` non-empty
- `retrieval_top_k > 0`
- `vector_index_dir` non-empty
- Inherits all `MemoryConfig` validations (max_index_tokens > 0, retrieval_max_files > 0, etc.)

### SemanticMemoryIndex (Logical, not a struct)

The searchable mapping from natural-language queries to relevant MemoryEntry records.

| Attribute | Description |
|-----------|-------------|
| **Source** | MemoryEntry.content → EmbeddingModel.embed() → f32 vector |
| **Storage** | `TurbovecVectorStore` collection, one vector per memory entry |
| **Key** | `document_id = memory_entry.name` (maps to stable `u64` internal id via hash) |
| **Metadata** | `memory_name`, `memory_type`, `source`, `updated_at` per chunk record |
| **Rebuild** | `rebuild_index()` reads all `.md` files via `FileMemory`, re-embeds, replaces collection |

### MemorySearchResult

A ranked retrieval result containing memory identity, metadata, score, and bounded content.

| Field | Type | Description |
|-------|------|-------------|
| `memory_name` | `String` | Stable memory entry name |
| `description` | `String` | One-line memory description |
| `memory_type` | `MemoryType` | Category (User/Feedback/Project/Reference/Unknown) |
| `score` | `f32` | Cosine similarity score (higher = more relevant) |
| `content` | `String` | Content truncated to `retrieval_max_tokens_per_file` |
| `updated_at` | `String` | Last update timestamp (RFC 3339) |

**Sorting**: By `score` descending. Equal scores tie-break by `memory_name` ascending (deterministic).

### MemoryRebuildReport

Summary of a rebuild operation outcome.

| Field | Type | Description |
|-------|------|-------------|
| `total_scanned` | `usize` | Markdown files scanned |
| `indexed` | `usize` | Successfully embedded + inserted |
| `skipped` | `usize` | Malformed or empty files skipped |
| `errors` | `Vec<String>` | Per-file error descriptions |
| `duration_ms` | `u64` | Wall-clock rebuild time |

## Entity State Transitions

### MemoryEntry lifecycle in TurbovecMemory

```
                  write()
  [absent] ──────────────────► [present + indexed]
                                  │
                                  │ write() (upsert)
                                  ▼
                              [present + re-indexed]
                                  │
                                  │ delete()
                                  ▼
                              [absent (files + vectors removed)]
```

### Vector Index Consistency States

```
  [Clean] ── write() ──► [Clean]       (synchronous index update)
  [Clean] ── delete() ─► [Clean]       (synchronous index removal)
  [Clean] ── ext. edit ─► [Dirty]      (external file modification)
  [Dirty] ── rebuild_index() ─► [Clean]
  [Missing] ── rebuild_index() ─► [Clean]
  [Missing] ── first retrieve_relevant() ─► [Clean] (auto-rebuild if configured)
  [Corrupted] ── rebuild_index() ─► [Clean]
```

## Relations to Existing Entities

| This Feature | Existing Entity (from Feature) | Relationship |
|-------------|-------------------------------|-------------|
| `TurbovecMemory` | `Memory` trait (009) | Implements |
| `TurbovecMemory` | `FileMemory` (009) | Composes (delegate) |
| `TurbovecMemory` | `EmbeddingModel` (005/011) | Depends on (Arc<dyn>) |
| `TurbovecMemory` | `TurbovecVectorStore` (016) | Composes (Arc) |
| `TurbovecMemory` | `MemoryEntry`, `MemoryType`, `MemoryMetadata` (009) | Reuses |
| `TurbovecMemory` | `MemoryError` (009) | Extends (new variant) |
| `TurbovecMemoryConfig` | `MemoryConfig` (009) | Extends (additional fields) |
| `MemorySearchResult` | `VectorSearchResult` (011) | Analogous structure |
| `MemoryRebuildReport` | — | New |
