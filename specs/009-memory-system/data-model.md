# Data Model: Memory System

**Feature**: 009-memory-system  
**Date**: 2026-07-30  

## Entity Diagram

```text
┌──────────────────────────────────────────────────────────────────┐
│ MemoryConfig                                                     │
│   memory_dir: String                                             │
│   max_index_tokens: usize (default 4000)                         │
│   retrieval_async: bool (default true)                           │
│   retrieval_max_files: usize (default 200)                       │
│   retrieval_max_tokens_per_file: usize (default 2000)            │
│   retrieval_max_tokens_per_frontmatter: usize (default 256)      │
│   memory_instructions: String                                    │
│   retrieval_instructions: String                                 │
└──────────────────────────────────────────────────────────────────┘
                              │
                              │ configures
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│ <<trait>> Memory                                                 │
│   + write(entry: MemoryEntry) -> Result<(), MemoryError>         │
│   + read(name: &str) -> Result<Option<MemoryEntry>, MemoryError> │
│   + delete(name: &str) -> Result<(), MemoryError>                │
│   + list() -> Result<Vec<MemoryFileHeader>, MemoryError>         │
│   + search(query: &str, type_filter: Option<MemoryType>)         │
│         -> Result<Vec<MemoryEntry>, MemoryError>                 │
│   + get_index_content() -> Result<String, MemoryError>           │
│   + retrieve_relevant(query: &str, model: &Arc<dyn ChatModel>)   │
│         -> Result<Option<String>, MemoryError>                   │
└──────────────────────────────────────────────────────────────────┘
                              △
                              │ implements
                              │
┌──────────────────────────────────────────────────────────────────┐
│ FileMemory                                                       │
│   - backend: Arc<dyn Backend>                                    │
│   - config: MemoryConfig                                         │
│   - index_lock: Mutex<()>                                        │
│   ─────────────────────────────────────────────────────────────  │
│   + new(workdir: &str, config: MemoryConfig,                     │
│         backend: Option<Arc<dyn Backend>>) -> Self               │
└──────────────────────────────────────────────────────────────────┘
                              │
                              │ uses
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│ <<trait>> Backend                                                │
│   + read_file(path: &str) -> Result<Vec<u8>, MemoryError>        │
│   + write_file(path: &str, data: &[u8])                          │
│         -> Result<(), MemoryError>                               │
│   + delete_file(path: &str) -> Result<(), MemoryError>           │
│   + file_exists(path: &str) -> Result<bool, MemoryError>         │
│   + list_dir(path: &str, recursive: bool)                        │
│         -> Result<Vec<String>, MemoryError>                      │
│   + join_path(a: &str, b: &str) -> String                        │
│   + stat_mtime(path: &str) -> Result<Option<f64>, MemoryError>   │
│   + normpath(path: &str) -> String                               │
│   + isabs(path: &str) -> bool                                    │
└──────────────────────────────────────────────────────────────────┘
                              △
                              │ implements
                              │
┌──────────────────────────────────────────────────────────────────┐
│ LocalBackend                                                     │
│   (uses tokio::fs for all operations)                            │
└──────────────────────────────────────────────────────────────────┘
```

## Core Entities

### MemoryType (enum)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    User,
    Feedback,
    Project,
    Reference,
    #[serde(untagged)]
    Unknown(String),
}
```

**Validation**: No validation beyond what `#[serde(rename_all = "lowercase")]` provides. `Unknown(String)` catch-all per Constitution §12.

**State transitions**: None (value type, immutable once created).

---

### MemoryMetadata

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetadata {
    pub mem_type: MemoryType,
    pub created_at: String,    // ISO 8601
    pub updated_at: String,    // ISO 8601
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}
```

**Validation**:
- `mem_type`: must be valid `MemoryType` variant (handled by serde).
- `created_at` / `updated_at`: ISO 8601 strings; format validated on construction.
- `tags`: optional, no duplicate validation (consumers deduplicate if needed).

---

### MemoryEntry

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub name: String,              // unique slug (filename without .md)
    pub description: String,       // retrieval trigger
    pub metadata: MemoryMetadata,
    pub content: String,           // body text after frontmatter
}
```

**Validation**:
- `name`: non-empty, matches `[a-zA-Z0-9_-]+` (safe for filenames).
- `description`: non-empty (required for retrieval to work).
- `content`: can be empty (allowed for placeholder entries).

**Relationships**:
- One `MemoryEntry` → one `.md` file.
- One `MemoryEntry` → one line in `MEMORY.md` index.

---

### MemoryFileHeader

```rust
#[derive(Debug, Clone)]
pub struct MemoryFileHeader {
    pub filename: String,          // e.g., "user_role.md"
    pub path: String,              // absolute backend path
    pub description: Option<String>,
    pub mem_type: Option<MemoryType>,
    pub mtime: Option<f64>,        // Unix timestamp, None when unavailable
}
```

**Purpose**: Lightweight metadata for `list()` and retrieval candidate selection. Does NOT include content.

---

### MemoryConfig

```rust
#[derive(Debug, Clone)]
pub struct MemoryConfig {
    pub memory_dir: String,                                  // default: "Memory"
    pub max_index_tokens: usize,                             // default: 4000
    pub retrieval_async: bool,                               // default: true
    pub retrieval_max_files: usize,                          // default: 200
    pub retrieval_max_tokens_per_file: usize,                // default: 2000
    pub retrieval_max_tokens_per_frontmatter: usize,         // default: 256
    pub memory_instructions: String,                         // default: DEFAULT_MEMORY_INSTRUCTIONS
    pub retrieval_instructions: String,                      // default: DEFAULT_RETRIEVAL_INSTRUCTIONS
}
```

**Validation** (FR-022):
- `max_index_tokens > 0`
- `retrieval_max_files > 0`
- `retrieval_max_tokens_per_file > 0`
- `retrieval_max_tokens_per_frontmatter > 0`

---

### MemoryError (enum)

```rust
#[derive(Debug)]
pub enum MemoryError {
    IoError { path: String, message: String },
    ParseError { filename: String, message: String },
    ValidationError { field: String, message: String },
    NotFound { name: String },
    IndexError { message: String },
    RetrievalError { reason: String },
}
```

Per Constitution §13: typed errors, no API key exposure, sufficient debug context.

---

## File Format Specification

### Memory File (`.md`)

```markdown
---
name: user-role
description: When you are about to make assumptions about the user's expertise level
type: user
created_at: 2026-07-30T12:00:00Z
updated_at: 2026-07-30T12:00:00Z
tags: role, expertise
---

The user is a senior Rust developer with 10 years of experience. They prefer
concise explanations with code examples over verbose tutorials.
```

### MEMORY.md Index File

```markdown
- [user-role](user-role.md) — When you are about to make assumptions about the user's expertise level
- [project-deadline](project-deadline.md) — The Q3 release deadline is September 30
- [feedback-code-style](feedback-code-style.md) — User prefers .await on its own line for readability
```

**Constraints**:
- One line per memory entry, format: `- [Title](file.md) — one-line description`
- Max `max_index_tokens` tokens; truncated from the end when exceeded.
- Lines after 200 are silently ignored by context loading (consumer responsibility).

---

## State Management

### MemoryMiddleware Internal State

```rust
pub struct MemoryMiddleware {
    memory: Arc<dyn Memory>,
    config: MemoryConfig,
    // In-flight async retrieval task (spawned in pre_reply, polled in pre_reasoning)
    retrieval_handle: Mutex<Option<tokio::task::JoinHandle<Result<Option<String>, MemoryError>>>>,
    cached_user_input: Mutex<Option<String>>,
}
```

**Lifecycle**:
1. `pre_reply` → spawn `tokio::spawn(retrieval_task)` → store handle in `retrieval_handle`
2. `pre_reasoning` → poll `retrieval_handle` → if done, inject `HintBlock` and clear handle
3. On error/cancellation: clear handle, no block injection
4. `on_system_prompt` → read index, append instructions + truncated index

**Concurrency**: `Mutex<Option<JoinHandle>>` on middleware instance; one reply at a time per agent guarantees no contention.
