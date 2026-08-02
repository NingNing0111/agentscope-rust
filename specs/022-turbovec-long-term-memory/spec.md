# Feature Specification: TurboVec Long-Term Memory

**Feature Branch**: `022-turbovec-long-term-memory`

**Created**: 2026-08-02

**Status**: Draft

**Input**: User description: "新增一个基于 turbovec 实现的长期记忆memory 实现。"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Persistent Semantic Memory Store (Priority: P1)

A developer wants an agent to remember durable facts, preferences, project notes, and reference links across sessions, while retrieving memories by semantic relevance rather than only exact text matching. They create a long-term memory store that accepts memory entries and later searches them with natural language queries.

**Why this priority**: Durable memory is the core user value. Without a working store that can write, persist, and retrieve relevant memories, no agent integration or migration scenario can deliver value.

**Independent Test**: Create an empty long-term memory store, write several memory entries about a user and project, restart/reload the store, then search using a related query and verify that the most relevant memories are returned in ranked order.

**Acceptance Scenarios**:

1. **Given** an empty long-term memory store, **When** the developer writes a memory entry describing a user preference, **Then** the entry is durably stored and can be read back after the store is reopened.
2. **Given** a store containing at least 20 memories across multiple categories, **When** the developer searches for "deployment preferences", **Then** results prioritize memories about deployment and exclude clearly unrelated memories.
3. **Given** an existing memory with the same stable name, **When** the developer writes an updated version, **Then** the existing memory is updated rather than duplicated.

---

### User Story 2 - Agent Uses Relevant Long-Term Memories (Priority: P2)

A developer wants an agent to automatically consult long-term memories during conversation so responses can reflect known user preferences, feedback, project constraints, and references without requiring the developer to manually inject every relevant memory.

**Why this priority**: Agent integration converts storage into end-user benefit. It depends on the store being reliable first, but it is required for the feature to be useful in real agent workflows.

**Independent Test**: Configure an agent with the long-term memory backend, add memories about the user's coding style and project constraints, send a related prompt, and verify that only relevant memories are surfaced to the agent context within the configured bounds.

**Acceptance Scenarios**:

1. **Given** an agent configured with long-term memory, **When** the agent receives a prompt related to a stored project constraint, **Then** the relevant memory is made available to the agent before it produces its response.
2. **Given** a memory store with many unrelated entries, **When** the agent receives a prompt about a specific topic, **Then** memory retrieval returns at most the configured number of relevant entries and does not flood the context with unrelated content.
3. **Given** memory retrieval fails due to a temporary store or indexing issue, **When** the agent processes a request, **Then** the agent continues with a clear non-fatal retrieval outcome rather than crashing or silently corrupting memory state.

---

### User Story 3 - Maintain and Inspect Long-Term Memories (Priority: P3)

A developer wants to list, inspect, update, delete, and rebuild searchable memory data so the long-term store can be maintained safely over time as memories grow, become stale, or are edited externally.

**Why this priority**: Maintenance prevents stale or incorrect memories from accumulating. It is lower priority than storage and agent use, but essential for production-quality long-term memory.

**Independent Test**: Create memories, list their metadata, delete one, update another, rebuild the searchable index, and verify that subsequent searches reflect the latest visible memory set.

**Acceptance Scenarios**:

1. **Given** a store with memories in user, feedback, project, and reference categories, **When** the developer lists memories with a category filter, **Then** only matching memory metadata is returned without loading every full memory body.
2. **Given** a deleted memory, **When** the developer searches for text that previously matched it, **Then** the deleted memory is not returned.
3. **Given** memory files or records were modified outside the normal write path, **When** the developer rebuilds the searchable data, **Then** subsequent retrieval reflects the current durable memory content.

---

### Edge Cases

- Empty store search returns an empty result set and a successful status.
- Duplicate memory names use upsert semantics and never create two active entries with the same stable name.
- Malformed memory content or metadata is reported as a validation error for that entry without preventing healthy entries from being listed or searched.
- Extremely long memory bodies are bounded during retrieval so they do not consume the entire agent context.
- Search requests with empty or whitespace-only queries return either recent/indexed summaries or an empty result according to documented configuration, not arbitrary unrelated results.
- Deleting a memory removes it from both durable storage and semantic retrieval results.
- Rebuilding the searchable data after interruption can be safely retried without creating duplicate results.
- Concurrent readers can search while writes are happening, and callers observe either the previous complete state or the next complete state, not partial/corrupt entries.
- If the vector index is missing or out of date relative to durable memory content, the system detects the mismatch and offers a safe rebuild path.
- Sensitive data included in memory content is not emitted into logs or traces by default.

## Requirements *(mandatory)*

### Functional Requirements

**Long-term memory behavior**:

- **FR-001**: System MUST provide a long-term memory implementation that persists memory entries across agent sessions and process restarts.
- **FR-002**: System MUST store each memory with a stable unique name, one-line description, category/type, optional tags, timestamps, and body content.
- **FR-003**: System MUST support creating, reading, updating, deleting, listing, and searching long-term memories.
- **FR-004**: System MUST implement upsert behavior when writing a memory with an existing stable name.
- **FR-005**: System MUST preserve the existing memory categories used by the project: user, feedback, project, and reference.
- **FR-006**: System MUST support semantic retrieval using the turbovec-backed searchable representation so natural-language queries can find conceptually related memories.
- **FR-007**: System MUST return search results with stable memory identity, relevance score or rank, memory metadata, and bounded content suitable for agent context injection.
- **FR-008**: System MUST support configurable limits for maximum returned memories, maximum content per memory, and maximum total memory context.

**Durability and index consistency**:

- **FR-009**: System MUST durably persist memory content independently from the semantic search index so memory bodies remain recoverable if the index must be rebuilt.
- **FR-010**: System MUST keep semantic search data consistent with memory writes, updates, and deletes.
- **FR-011**: System MUST provide a rebuild operation that regenerates the searchable data from durable memory content.
- **FR-012**: System MUST detect missing, incompatible, or corrupted searchable data and return a clear recoverable error or rebuild recommendation.
- **FR-013**: System MUST ensure interrupted writes do not leave a memory visible as partially updated content.
- **FR-014**: System MUST preserve deterministic ordering for equally relevant search results using stable tie-breakers.

**Agent integration**:

- **FR-015**: System MUST allow the long-term memory implementation to be used wherever the existing memory abstraction is accepted.
- **FR-016**: System MUST allow agent workflows to retrieve relevant long-term memories for a user prompt before model response generation.
- **FR-017**: System MUST bound retrieved memory content before adding it to the agent context.
- **FR-018**: System MUST expose retrieval failures as typed, non-sensitive errors and MUST NOT silently pretend retrieval succeeded.
- **FR-019**: System MUST keep retrieval optional for agent progress: a retrieval failure must not crash the agent loop unless the caller explicitly configures fail-closed behavior.

**Maintenance and observability**:

- **FR-020**: System MUST support listing memory metadata without loading all full memory bodies.
- **FR-021**: System MUST support filtering memories by category/type and tags during listing and retrieval where applicable.
- **FR-022**: System MUST record non-sensitive structured trace events for memory write, delete, search, rebuild, and retrieval failure outcomes.
- **FR-023**: System MUST avoid logging raw memory bodies by default.
- **FR-024**: System MUST provide validation for malformed memory metadata and unsupported configuration values.
- **FR-025**: System MUST document compatibility level, known deviations, and migration expectations for this memory backend.

### Key Entities

- **LongTermMemoryStore**: A durable memory capability that owns memory entries, searchable data, configuration, and maintenance operations.
- **MemoryEntry**: A single durable memory record with stable name, description, category/type, optional tags, timestamps, and body content.
- **MemoryMetadata**: The inspectable metadata for a memory entry, used for listing, filtering, indexing, and context selection.
- **SemanticMemoryIndex**: The searchable representation that maps natural-language queries to relevant memory entries using turbovec-backed vector retrieval.
- **MemorySearchResult**: A ranked retrieval result containing memory identity, metadata, score/rank, and bounded content.
- **MemoryRebuildReport**: A summary of rebuild outcomes, including processed entries, skipped invalid entries, recovered entries, and errors.
- **MemoryConfig**: User-configurable limits and behavior for storage location, retrieval bounds, rebuild policy, and fail-open/fail-closed agent behavior.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A developer can write 1,000 long-term memories and reopen the store with all entries recoverable without manual repair.
- **SC-002**: Searching 1,000 memories returns the top 10 relevant results in under 100ms on a typical development machine, excluding time spent generating query embeddings.
- **SC-003**: Rebuilding searchable data for 1,000 memories completes in under 10 seconds, excluding time spent generating embeddings for memory content.
- **SC-004**: Retrieval context injection respects configured limits 100% of the time and never exceeds the configured maximum number of memories or total memory content budget.
- **SC-005**: Delete and update operations are reflected in subsequent search results with no stale deleted entries returned after the operation reports success.
- **SC-006**: Malformed entries do not prevent healthy entries from being listed or searched; validation reports identify 100% of skipped malformed entries.
- **SC-007**: Agent workflows using long-term memory continue to produce responses when retrieval fails under the default fail-open configuration, while exposing a typed retrieval error for observability.
- **SC-008**: Compatibility and regression tests cover create, read, update, delete, list, search, rebuild, agent retrieval, malformed metadata, empty store, and delete/update consistency scenarios.

## Assumptions

- The feature adds an additional long-term memory backend rather than replacing the existing file-based memory behavior.
- turbovec is a required product constraint for the semantic searchable representation of this backend.
- Durable memory content remains the source of truth; the semantic index is rebuildable derived data.
- Embedding generation is provided by existing model or embedding abstractions and is not introduced as a new provider capability by this feature.
- The initial scope is single-process local long-term memory; distributed synchronization and multi-node replication are out of scope.
- Existing memory categories and frontmatter-style metadata remain compatible with previously created memories where feasible.
- Agent integration should reuse existing memory abstraction and middleware patterns instead of introducing a parallel agent lifecycle.
- Security posture follows the project constitution: no sensitive memory bodies in default logs, typed errors, and no silent pseudo-success.
