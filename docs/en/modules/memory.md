# Memory System / Memory

> One-liner: `agent_scope_memory` provides cross-session persistent long-term memory — using the `Memory` trait to abstract writes, reads, search, indexing, and relevant-memory retrieval, using `FileMemory` to store memories as Markdown files, and injecting them into the Agent reply lifecycle through `MemoryMiddleware`.

## 1. Module Overview (Overview)

This module covers two cooperating parts:

| Part | Responsibility |
|------|----------------|
| `agent_scope_memory` | Long-term memory data model, file storage, index, search, relevant-memory retrieval |
| `agent_scope_agent::MemoryMiddleware` | Connects long-term memory to the Agent lifecycle: system prompt injection, async retrieval, `HintBlock` injection |

**When to use**: saving user preferences, project facts, feedback rules, and external references; letting an Agent use saved facts in later conversations; selecting relevant files through a `MEMORY.md` index and retrieval without putting every memory into context.

**Prerequisites**: read [Agent System](./agent.md), [Message & Basic Types](./message-types.md), and [Model Abstraction](./model.md) first. If you only want to run the integration example, run `examples/memory_test.rs` directly.

## 2. Core Concepts & Main Public Types (Core Concepts)

### 2.1 `Memory` trait

`Memory` is the unified interface for long-term memory backends:

| Method | Description |
|--------|-------------|
| `write(entry)` | Write or update one memory entry; same-name memories use upsert semantics |
| `read(name)` | Read a complete memory by name; returns `Ok(None)` when absent |
| `delete(name)` | Delete the memory file and remove its index line |
| `list()` | List memory headers only, without loading full bodies |
| `search(query, type_filter)` | Case-insensitive substring search over descriptions and bodies, optionally filtered by memory type |
| `get_index_content()` | Read the `MEMORY.md` index content |
| `retrieve_relevant(query, model, max_results)` | Use the bound `ChatModel` to select relevant memory files and return concatenated body sections |

`Memory` requires `Send + Sync`, so it can be injected into middleware as `Arc<dyn Memory>`.

### 2.2 `MemoryEntry` and `MemoryMetadata`

One memory entry contains four core fields:

| Field | Description |
|-------|-------------|
| `name` | Unique slug; `FileMemory` requires `[A-Za-z0-9_-]+` |
| `description` | One-line description used by the `MEMORY.md` index and relevance selection |
| `metadata` | Type, creation time, update time, optional tags |
| `content` | Memory body as Markdown text |

`MemoryType` currently has four built-in categories:

| Type | Use |
|------|-----|
| `User` | User identity, preferences, long-term background |
| `Feedback` | User feedback about working style |
| `Project` | Current project facts, constraints, status |
| `Reference` | External resources, links, document summaries |

Unknown type strings become `MemoryType::Unknown(String)`, enabling future extension.

### 2.3 Markdown File Format

`FileMemory` saves each memory as `<name>.md`, with frontmatter and a Markdown body:

```markdown
---
name: user-favorite-color
description: The user's favorite color preference
type: user
created_at: 2026-08-01T00:00:00Z
updated_at: 2026-08-01T00:00:00Z
---

The user's favorite color is cerulean blue.
```

`MEMORY.md` is the index file in the same directory, one line per memory:

```markdown
- [user-favorite-color](user-favorite-color.md) — The user's favorite color preference
```

### 2.4 `FileMemory`

`FileMemory::new(workdir, config, backend)` is the current built-in implementation:

- If `config.memory_dir` is relative, it resolves to `workdir / memory_dir`
- If it is absolute, that directory is used directly
- `backend = None` uses `LocalBackend`
- `write()` creates parent directories, writes `<name>.md`, and updates `MEMORY.md`
- `delete()` is idempotent for absent files and removes the index line
- `list()` skips `MEMORY.md`, parses only `.md` files with frontmatter, sorts by modification time descending, and truncates to `retrieval_max_files`

### 2.5 `MemoryConfig`

| Field | Default | Description |
|-------|---------|-------------|
| `memory_dir` | `"Memory"` | Memory directory |
| `max_index_tokens` | `4000` | Maximum token budget for the index before it is injected into the system prompt |
| `retrieval_async` | `true` | Whether to start relevant-memory retrieval asynchronously in `pre_reply` |
| `retrieval_max_files` | `200` | Maximum number of files to list/retrieve |
| `retrieval_max_tokens_per_file` | `2000` | Maximum token budget per retrieved file body |
| `retrieval_max_tokens_per_frontmatter` | `256` | Frontmatter token budget (reserved configuration field) |
| `memory_instructions` | Default long-term memory instructions | Injected into the system prompt to tell the model how to use the index |
| `retrieval_instructions` | Default retrieval instructions | Used when selecting relevant files |

`validate()` ensures the directory is non-empty, and all token / file-count limits are greater than 0.

### 2.6 `MemoryMiddleware`

`MemoryMiddleware` connects `Memory` to the Agent lifecycle:

| Hook | Behavior |
|------|----------|
| `pre_reply` | Saves the current `ChatModel` reference; when `retrieval_async = true` and user input is non-empty, starts a relevant-memory retrieval task |
| `on_system_prompt` | Reads `MEMORY.md`; if a model reference is available, truncates it by `max_index_tokens`; appends memory instructions and the index |
| `pre_reasoning` | If the async retrieval task has completed, injects retrieval results as a `HintBlock` into the last user message |

If the index is empty or reading it fails, the middleware injects `Your MEMORY.md is currently empty.` and does not fail the Agent reply flow.

## 3. Quick Example (Quick Example)

The repository example constructs a memory-enabled Agent like this:

<!-- source: examples/common.rs:L368-L406 -->
```rust
pub fn create_memory_agent(
    api_key: &str,
    model_name: &str,
    workdir: &str,
) -> Result<ReActAgent, Box<dyn std::error::Error>> {
    let model = create_model(api_key, model_name);

    // Build FileMemory with default config
    let memory_config = MemoryConfig {
        memory_dir: "memory_data".into(),
        ..Default::default()
    };
    let memory: Arc<dyn agent_scope_memory::Memory> =
        Arc::new(FileMemory::new(workdir, memory_config.clone(), None));

    // Wrap in MemoryMiddleware
    let middleware = Arc::new(MemoryMiddleware::new(memory, memory_config));
```

The full example continues by constructing `AgentConfig` and `ReActAgent`, and injects `middleware` as `vec![middleware]`.

## 4. Usage Patterns (Usage Patterns)

### 4.1 Run the Built-in Memory Integration Example

`examples/memory_test.rs` runs three end-to-end checks: write memory, search memory, and answer questions based on memory.

```bash
cargo run --example memory_test -- --api-key sk-xxxxx
cargo run --example memory_test -- --api-key sk-xxxxx --model qwen-max
cargo run --example memory_test -- --api-key sk-xxxxx --keep-dir
```

This example also supports reading the API key from the environment:

```bash
API_KEY=sk-xxxxx cargo run --example memory_test
```

`--keep-dir` preserves the temporary memory directory so you can inspect the generated `<name>.md` and `MEMORY.md` files.

### 4.2 Write and Read Memories Directly

Use this when application code explicitly saves user preferences, project facts, or external references:

```rust
use agent_scope_memory::{FileMemory, Memory, MemoryConfig, MemoryEntry, MemoryType};

let config = MemoryConfig {
    memory_dir: "memory_data".into(),
    ..Default::default()
};
let memory = FileMemory::new(".", config, None);

let entry = MemoryEntry::new(
    "user-favorite-color",
    "The user's favorite color preference",
    MemoryType::User,
    "The user's favorite color is cerulean blue.",
);

memory.write(entry).await?;
let loaded = memory.read("user-favorite-color").await?;
```

Writing the same `name` overwrites the file and updates the index line; it does not create duplicate index entries.

### 4.3 Search and Type Filtering

`search()` is local substring search: it matches `description + content`, case-insensitively:

```rust
let all = memory.search("Hangzhou", None).await?;
let only_user = memory
    .search("favorite", Some(MemoryType::User))
    .await?;
```

If you need semantic relevance selection instead of simple substring matching, use `retrieve_relevant()` or let `MemoryMiddleware` retrieve automatically.

### 4.4 Connect Memory to an Agent

`MemoryMiddleware` is the recommended Agent integration path:

```rust
use std::sync::Arc;
use agent_scope_agent::MemoryMiddleware;
use agent_scope_memory::{FileMemory, MemoryConfig};

let memory_config = MemoryConfig {
    memory_dir: "memory_data".into(),
    ..Default::default()
};
let memory = Arc::new(FileMemory::new(workdir, memory_config.clone(), None));
let middleware = Arc::new(MemoryMiddleware::new(memory, memory_config));

let agent = ReActAgent::new(
    config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![middleware],
)?;
```

After integration:

1. Before every reply, the Agent system prompt includes long-term memory instructions and the `MEMORY.md` index.
2. When user input is non-empty and `retrieval_async = true`, a relevant-memory retrieval task starts.
3. If retrieval completes before reasoning, the result is appended to the last user message as a `HintBlock`.

### 4.5 Control Index and Body Truncation

`MEMORY.md` may grow as more memories are saved. When a model reference is available, middleware estimates tokens with `model.count_tokens(...)`, and appends this notice when the index exceeds `max_index_tokens`:

```text
<<<TRUNCATED: 12 memory index lines omitted>>>
```

Retrieved file bodies are also truncated by `retrieval_max_tokens_per_file`, with this suffix:

```text
<<<TRUNCATED>>>
```

### 4.6 Custom Storage Backends

`Backend` is the low-level storage abstraction. The current built-in backend is `LocalBackend`. For remote storage, implement:

```rust
#[async_trait::async_trait]
pub trait Backend: Send + Sync {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, MemoryError>;
    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), MemoryError>;
    async fn delete_file(&self, path: &str) -> Result<(), MemoryError>;
    async fn file_exists(&self, path: &str) -> Result<bool, MemoryError>;
    async fn list_dir(&self, path: &str, recursive: bool) -> Result<Vec<String>, MemoryError>;
    fn join_path(&self, a: &str, b: &str) -> String;
    async fn stat_mtime(&self, path: &str) -> Result<Option<f64>, MemoryError>;
    fn normpath(&self, path: &str) -> String;
    fn isabs(&self, path: &str) -> bool;
}
```

Then inject it with `FileMemory::new(workdir, config, Some(Arc::new(my_backend)))`.

## 5. Errors & Unsupported Capabilities (Errors & Unsupported)

| Error | Common Cause | Suggested Handling |
|-------|--------------|--------------------|
| `MemoryError::IoError` | File read/write, directory listing, or metadata read failed | Check path, permissions, and disk state |
| `MemoryError::ParseError` | Reserved parse error variant | Most malformed files are currently skipped rather than thrown |
| `MemoryError::ValidationError` | Invalid `name`/`description`/config value, or empty query | Fix input; use `[A-Za-z0-9_-]+` for `name` |
| `MemoryError::NotFound` | Reserved not-found error variant | Current `read()` returns `Ok(None)` for absent entries |
| `MemoryError::IndexError` | Index management failed | Check whether `MEMORY.md` is writable |
| `MemoryError::RetrievalError` | Failed to build retrieval prompt or parse retrieval result | Usually degrade to no relevant memory |

**Unsupported capabilities**:

- Only local file backend `LocalBackend` is built in today; remote backends require user-defined `Backend` implementations.
- `search()` is not vector retrieval or semantic search; it only performs local substring matching.
- `retrieve_relevant()` depends on `ChatModel::generate_structured_output()`; if the model or provider does not support structured output, it degrades to no relevant memory rather than fabricating a result.
- Memory writing does not automatically extract facts from conversation; the application or a tool must call `write()` explicitly.

## 6. Compatibility (Compatibility)

- **Compatibility level**: **L1** (Markdown frontmatter, `MEMORY.md` index, and memory-type data protocol); **L2** (write/read/search, index update, middleware injection, and relevant-memory retrieval behavior)
- **Authoritative source**: `specs/001-compatibility-baseline/capability-matrix.json`
- **Known deviations**:
  - The matrix `status` field is currently `NOT_ANALYZED` for all entries; levels on this page are cross-verified against `memory`-related `target_level` entries + `specs/009-memory-system` + current code state.
  - `search()` is currently deterministic local substring search; semantic relevance selection only happens through `retrieve_relevant()` / middleware by model structured output.
  - `MemoryMiddleware` injects `HintBlock` only when the retrieval task has already completed; if it has not completed, the current reasoning step does not block, avoiding reply-path latency.
  - Malformed frontmatter files are usually skipped in `list()`/`search()`, preserving memory-system robustness.
- **Unsupported capabilities**: built-in remote storage backend, automatic conversation fact extraction, and vector semantic search are not built into this module.

## 7. See Also (See Also)

- [Agent System](./agent.md) — where `MemoryMiddleware` runs in the Agent lifecycle
- [Message & Basic Types](./message-types.md) — the `HintBlock` data protocol
- [Model Abstraction](./model.md) — the `ChatModel` and structured output used by `retrieve_relevant()`
- Session management — the boundary between short-term context and long-term memory
- RAG — how this differs from, and composes with, document knowledge bases and vector retrieval
