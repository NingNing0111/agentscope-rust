# Feature Specification: Memory System

**Feature Branch**: `009-memory-system`

**Created**: 2026-07-30

**Status**: Draft

**Input**: User description: "开始实现 Feature 009 Memory 模块"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Save and Retrieve Memories via Trait Interface (Priority: P1)

A developer wants to store information about a user across conversations so that the agent remembers who the user is and past interactions. They use the `Memory` trait to write memory entries and later search for relevant memories.

**Why this priority**: The Memory trait is the foundational abstraction — without it, no memory backend can exist. This validates that the trait contract works for the simplest use case: write once, read back.

**Independent Test**: Create an in-memory memory backend. Write a user-type memory entry ("the user is a data scientist"), then search with a related query ("what is the user's role?") and verify the correct entry is returned. Also verify that entries of different types (user, feedback, project, reference) can be stored and retrieved independently.

**Acceptance Scenarios**:

1. **Given** a `Memory` implementation with no stored entries, **When** the developer writes a memory with `type=user` and content "the user prefers short answers", **Then** the memory is persisted and immediately searchable by a relevant query.
2. **Given** a `Memory` implementation with 10 stored entries of mixed types, **When** the developer searches with a type filter `type=feedback`, **Then** only feedback-type memories are returned.
3. **Given** a `Memory` implementation with existing entries, **When** the developer writes a new entry with the same unique name as an existing one, **Then** the existing entry is updated (upsert semantics) rather than duplicated.

---

### User Story 2 - Memory Index Management (Priority: P2)

A developer wants the agent to maintain a concise index (`MEMORY.md`) that lists all stored memories with their descriptions, so the agent can decide which memory files to read without loading all content into the context window.

**Why this priority**: The index is the efficiency mechanism — without it, every memory must be loaded on every interaction, which doesn't scale. P2 because memory storage (P1) works without it, but practical use requires index management.

**Independent Test**: Create a memory store with 5 entries. Generate the memory index and verify: (a) each entry appears as a single line with filename and description, (b) total token count of the index is under configurable limits, (c) removing an entry removes its index line.

**Acceptance Scenarios**:

1. **Given** a memory directory with 5 stored memory files, **When** `get_index_content()` is called, **Then** a markdown index is returned containing one bullet-point line per file, each with the filename and one-line description.
2. **Given** a memory index that exceeds `max_index_tokens`, **When** `get_index_content()` is called, **Then** the index is truncated to fit within the token budget, with a truncation notice appended.
3. **Given** a memory file being deleted via the `Memory` trait, **When** `get_index_content()` is subsequently called, **Then** the deleted file's entry no longer appears in the index.

---

### User Story 3 - Relevance-Based Memory Retrieval (Priority: P3)

A developer wants the agent to automatically identify which stored memories are relevant to the current user input, rather than loading all memories into every conversation. The agent uses a lightweight model call to select relevant files based on their descriptions.

**Why this priority**: Automated relevance retrieval is what makes the memory system "agentic" — it allows the memory store to grow large without degrading performance. P3 because manual search (P1) provides basic functionality, but automatic relevance is the key differentiator for production use.

**Independent Test**: Create a memory store with 20 entries across 4 types. Provide a user query "I need to fix the authentication bug". Verify that the relevance selector returns only memories related to authentication/bugs (up to the configured max), not unrelated memories about the user's lunch preferences or project roadmap.

**Acceptance Scenarios**:

1. **Given** a memory store with 20 entries and a user query about "deploying to production", **When** `retrieve_relevant(query, max_results=5)` is called, **Then** at most 5 memory files are returned, selected by relevance to the deploy topic.
2. **Given** a memory store where no entries are relevant to the user query "what is the weather?", **When** `retrieve_relevant()` is called, **Then** an empty result set is returned rather than random unrelated memories.
3. **Given** the retrieval model configured to use a specific `ChatModel`, **When** `retrieve_relevant()` is called, **Then** the structured output schema forces the model to return only valid filenames, and hallucinated filenames are filtered out.

---

### User Story 4 - Integration with Agent Memory Lifecycle (Priority: P4)

A developer wants the Memory trait to integrate with the agent's middleware system so that memory instructions are injected into the system prompt and relevant memories are surfaced automatically during conversation.

**Why this priority**: This connects the Memory trait to the existing Agent infrastructure. Memory works standalone (P1-P3), but agent integration delivers the end-user value. P4 because it depends on the Memory trait being stable first.

**Independent Test**: Register a `MemoryMiddleware` with a ReActAgent. Send a user message. Verify: (a) memory instructions appear in the system prompt, (b) the MEMORY.md index is injected, (c) relevant memory files are retrieved and surfaced as hint blocks during the reasoning step.

**Acceptance Scenarios**:

1. **Given** a ReActAgent with `MemoryMiddleware` configured, **When** the agent processes a reply, **Then** the system prompt is augmented with memory instructions and the bounded MEMORY.md index.
2. **Given** a `MemoryMiddleware` with `async_retrieval=true`, **When** a reply starts, **Then** a retrieval task is spawned that runs concurrently with the model call, and its results are injected as a `HintBlock` into the agent's context before the model call completes.
3. **Given** a `MemoryMiddleware` with `async_retrieval=false`, **When** a reply starts, **Then** retrieval is performed synchronously before the system prompt is built.

---

### Edge Cases

- What happens when a memory file's frontmatter is malformed (missing `---` delimiters, invalid YAML)? The file should be gracefully skipped in listing/search, not crash the entire retrieval.
- What happens when the memory directory doesn't exist yet? The system should create it idempotently on first access.
- What happens when two concurrent writes target the same memory file? The last write should win (no lock contention), with the index updated consistently.
- What happens when a memory file exceeds `max_tokens_per_file` during retrieval? The file content should be truncated to fit the budget.
- What happens when the `MEMORY.md` index file is manually edited externally? On next read, the system should load whatever is on disk — no in-memory cache staleness.
- What happens when the retrieval model call fails (network error, timeout)? Retrieval should return an empty result set rather than propagating the error to the agent loop.
- What happens when a memory file contains binary content? It should be decoded with error replacement, and if completely unreadable, skipped.

## Requirements *(mandatory)*

### Functional Requirements

**Memory trait (core abstraction)**:

- **FR-001**: System MUST define a `Memory` trait with methods: `write()`, `read()`, `delete()`, `list()`, `search()`, `get_index_content()`, `retrieve_relevant()`.
- **FR-002**: `write()` MUST accept a memory entry with fields: `name` (unique slug), `description` (retrieval trigger), `metadata` (type, tags, timestamp), and `content` (the memory body). It MUST implement upsert semantics (update if `name` exists, create if not).
- **FR-003**: `read()` MUST accept a filename/name and return the full memory content, or `None` if not found.
- **FR-004**: `delete()` MUST accept a filename/name and remove both the memory file and its index entry.
- **FR-005**: `list()` MUST return metadata for all stored memory files (name, description, type, modification time) without loading full content.
- **FR-006**: `search()` MUST accept a query string and optional type filter, and return memory entries whose content or description matches the query.
- **FR-007**: `get_index_content()` MUST return the contents of `MEMORY.md` as a string, truncated to a configurable `max_index_tokens` limit.
- **FR-008**: `retrieve_relevant()` MUST accept a user query and return up to `max_results` memory files selected by relevance using a `ChatModel` for semantic matching.

**Memory entry data model**:

- **FR-009**: System MUST define a `MemoryEntry` struct containing: `name` (String), `description` (String), `metadata` (MemoryMetadata), `content` (String).
- **FR-010**: `MemoryMetadata` MUST include: `mem_type` (enum of `User`, `Feedback`, `Project`, `Reference`), `created_at` (timestamp), `updated_at` (timestamp), `tags` (optional Vec<String>).
- **FR-011**: Each memory entry MUST be serializable to and from Markdown files with YAML frontmatter format: `---\nname: ...\ndescription: ...\ntype: ...\n---\n\ncontent`.
- **FR-012**: The `MemoryMetadata::mem_type` enum MUST use `#[serde(untagged)]` or equivalent to gracefully handle unknown type strings (future extensibility per Constitution §12).

**File-based memory backend**:

- **FR-013**: System MUST provide a `FileMemory` implementation of the `Memory` trait that stores memories as individual Markdown files in a configurable directory, using a `Backend` trait for storage abstraction.
- **FR-014**: `FileMemory` MUST maintain a `MEMORY.md` index file in the memory directory, where each line is a markdown link: `- [Title](file.md) — one-line description`.
- **FR-015**: `FileMemory` MUST support a `Backend` abstraction that allows switching between local filesystem and remote storage, with at minimum a `LocalBackend` implementation.
- **FR-016**: The `Backend` trait MUST define: `read_file()`, `write_file()`, `delete_file()`, `file_exists()`, `list_dir()`, `join_path()`, `stat_mtime()`.

**Memory middleware**:

- **FR-017**: System MUST provide a `MemoryMiddleware` that implements the existing `Middleware` trait and wraps a `Memory` implementation.
- **FR-018**: `MemoryMiddleware::on_system_prompt()` MUST append memory usage instructions and the bounded `MEMORY.md` index content to the agent's system prompt.
- **FR-019**: `MemoryMiddleware::on_reply()` MUST optionally start an asynchronous retrieval task (when `async_retrieval=true`) that runs concurrently with the model call.
- **FR-020**: `MemoryMiddleware::on_reasoning()` MUST check if the retrieval task has completed and, if so, inject retrieved memory content as `HintBlock`(s) into the agent's context.

**Configuration**:

- **FR-021**: System MUST define `MemoryConfig` with: `memory_dir` (String), `max_index_tokens` (usize, default 4000), `retrieval_async` (bool, default true), `retrieval_max_files` (usize, default 200), `retrieval_max_tokens_per_file` (usize, default 2000), `retrieval_max_tokens_per_frontmatter` (usize, default 256).
- **FR-022**: `MemoryConfig` MUST validate: `max_index_tokens > 0`, `retrieval_max_files > 0`, `retrieval_max_tokens_per_file > 0`.
- **FR-023**: `MemoryConfig` MUST include default memory usage instructions text and default retrieval instructions text, both overridable.

**Index management**:

- **FR-024**: When `write()` is called, the system MUST update the `MEMORY.md` index by adding/updating the corresponding line entry.
- **FR-025**: When `delete()` is called, the system MUST remove the corresponding line from `MEMORY.md`.
- **FR-026**: `get_index_content()` MUST truncate the index to `max_index_tokens` and append a truncation notice indicating how many lines were omitted when truncation occurs.

### Key Entities

- **Memory (trait)**: The core abstraction for persistent memory storage. Methods: `write()`, `read()`, `delete()`, `list()`, `search()`, `get_index_content()`, `retrieve_relevant()`. Owns the memory directory and manages the index.
- **MemoryEntry**: A single memory record with name (unique slug), description (retrieval trigger), metadata (type, timestamps, tags), and content (body text).
- **MemoryMetadata**: Type classification (`User`, `Feedback`, `Project`, `Reference`), creation/update timestamps, and optional tags.
- **FileMemory**: File-based implementation of `Memory`. Stores entries as individual `.md` files with YAML frontmatter. Maintains `MEMORY.md` index.
- **Backend (trait)**: Storage abstraction layer. `LocalBackend` for filesystem, with extensibility for remote storage. Methods: `read_file()`, `write_file()`, `delete_file()`, `file_exists()`, `list_dir()`, `join_path()`, `stat_mtime()`.
- **MemoryConfig**: Configuration for memory behavior — directory, token limits, retrieval settings, instruction texts.
- **MemoryMiddleware**: Middleware integration layer that wires `Memory` into the Agent lifecycle hooks (`on_system_prompt`, `on_reply`, `on_reasoning`).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A developer can create a memory store, write 10 entries, and search for relevant ones in under 1 second (excluding model-based retrieval).
- **SC-002**: The memory index for 100 memory files does not exceed 4000 tokens (configurable `max_index_tokens`).
- **SC-003**: Relevance-based retrieval correctly selects memory files within 2 seconds for a store of 100 entries (excluding model API latency).
- **SC-004**: The memory system handles 1000 memory files without degradation in listing and index generation performance (under 500ms).
- **SC-005**: Integration with agent middleware adds no more than 50ms overhead to system prompt generation (excluding model calls for retrieval).
- **SC-006**: All memory operations (write, read, delete, list, search) pass 100% of their unit tests, including edge cases for malformed files and missing directories.

## Assumptions

- Short-term memory (agent context messages, `AgentState::context`) is already implemented in `agent_scope_state` and is NOT part of this feature. Feature 009 focuses on long-term, persistent memory that survives across agent sessions.
- The `Backend` trait is necessary for future remote storage support but only `LocalBackend` needs to be implemented in this feature. Remote backends are deferred.
- Memory middleware is implemented in the `agent_scope_agent` crate (alongside existing middleware infrastructure), not in the memory crate itself.
- The `Memory` trait and `FileMemory` implementation live in a new `agent_scope_memory` crate, following the project's layered architecture pattern (Constitution §11).
- Model-based retrieval (`retrieve_relevant()`) uses the agent's bound model by default, consistent with how context compression already works in Feature 007.
- The frontmatter format follows the Python AgentScope `AgenticMemoryMiddleware` convention: YAML-like `key: value` pairs between `---` delimiters.
- Memory entry uniqueness is determined by the `name` field (slug), which must be unique within a memory directory.
- The `MemoryMiddleware` implements the `Middleware` trait defined in Feature 007 (`agent_scope_agent`), using `on_system_prompt`, `on_reply`, and `on_reasoning` hook points.
- The `memory_instructions` text injected into the system prompt is configurable but defaults match the Python AgentScope reference implementation's `DEFAULT_MEMORY_INSTRUCTIONS`.
- Token counting for index truncation uses the same estimation approach as the existing context compression mechanism in `agent_scope_agent`.
