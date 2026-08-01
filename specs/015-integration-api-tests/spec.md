# Feature Specification: Integration API Tests (Examples)

**Feature Branch**: `015-integration-api-tests`

**Created**: 2026-07-31

**Status**: Draft

**Input**: User description: "调用真实api 测试已实现的模块功能。model provider用dashscope。写到examples下"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Memory Integration E2E (Priority: P1)

A developer wants to verify that the Memory system works correctly with a real LLM (DashScope). They run a conversation where the agent stores a user's preference in memory, then in a subsequent conversation the agent recalls that preference using memory retrieval.

**Why this priority**: Memory is core to agent statefulness. Without verified memory integration, users cannot trust multi-conversation agent behavior. This is the foundation for persistent agent applications.

**Independent Test**: Run the example binary and observe that (a) first conversation returns memory save confirmation, (b) second conversation retrieves the stored preference successfully.

**Acceptance Scenarios**:

1. **Given** a fresh agent with file-based memory backend and DashScope model, **When** the user says "Remember my name is Alice and I like Python", **Then** the agent confirms the information is saved, and a second conversation retrieval yields "Alice" and "Python".
2. **Given** an agent with memory backend, **When** the user asks "What do you know about me?" without any prior conversation, **Then** the agent responds indicating no relevant memories found.
3. **Given** an agent with memory backend and stored entries, **When** the user asks a question related to stored information, **Then** the agent's response references the stored memory entries.

---

### User Story 2 - Session Persistence E2E (Priority: P1)

A developer wants to verify that Session save/load works across program restarts with real LLM interactions. They run a conversation, save the session to disk, then in a subsequent run load the session and confirm the conversation history is preserved.

**Why this priority**: Session persistence is the backbone of long-running agent applications. Users must be able to pause and resume conversations without losing context.

**Independent Test**: Run the example, conduct a conversation, observe session-save confirmation, then re-run loading that session and verify the agent remembers the prior conversation context.

**Acceptance Scenarios**:

1. **Given** a new session with DashScope agent, **When** the user says "My favorite number is 42" and the session is saved to disk, **Then** loading that session and asking "What's my favorite number?" yields a response containing "42".
2. **Given** a saved session file, **When** the session is loaded and a new message is sent, **Then** the agent's response is consistent with the full conversation history (not just the latest message).
3. **Given** a session with many messages exceeding the trim threshold, **When** the session is trimmed, **Then** loading the trimmed session shows fewer messages but the agent can still carry on a coherent conversation.

---

### User Story 3 - RAG Pipeline E2E (Priority: P2)

A developer wants to verify that the RAG (Retrieval-Augmented Generation) pipeline works with real LLM and embedding APIs. They index a document, then ask questions about its content and verify the agent's answer is grounded in the indexed document.

**Why this priority**: RAG is a high-value feature but depends on both embedding and chat model availability. It's positioned as P2 because it requires both API endpoints to be functional.

**Independent Test**: Run the example with a text document, observe embedding + indexing, then run queries and verify answers contain information from the indexed document.

**Acceptance Scenarios**:

1. **Given** a text document containing factual information indexed into the knowledge base, **When** the user asks a question about that document's content, **Then** the agent's answer includes facts from the document (not hallucinated).
2. **Given** an empty knowledge base, **When** the user asks a question, **Then** the agent responds without retrieved context and does not error out from RAG middleware.
3. **Given** a knowledge base with multiple chunks, **When** chunk retrieval is configured with a top-k limit, **Then** the agent's context includes approximately top-k chunks.

---

### User Story 4 - Streaming Tool-Call Round-Trip (Priority: P2)

A developer wants to verify the complete streaming lifecycle for tool calls with a real API. They ask a math question, observe tool call events and tool result events in the stream, and confirm the final answer is correct.

**Why this priority**: Streaming tool calls are the most complex event lifecycle and are critical for interactive applications. This is covered partially by existing examples but lacks dedicated verification.

**Independent Test**: Run the example and verify all event types in the streaming tool-call lifecycle are emitted in correct order and the final answer is mathematically correct.

**Acceptance Scenarios**:

1. **Given** an agent with calculator tool and streaming mode, **When** the user asks "Calculate 3.14 * 2.718", **Then** the stream emits ReplyStart → TextBlockStart(thinking) → ToolCallStart → ToolCallDelta* → ToolCallEnd → ToolResultStart → ToolResultTextDelta* → ToolResultEnd → TextBlockStart(answer) → TextBlockDelta* → TextBlockEnd → ReplyEnd, and the final answer is approximately 8.53452.
2. **Given** an agent with multiple tools registered, **When** the user asks a question requiring two separate tool calls, **Then** the stream contains two complete ToolCall-Start/Delta/End → ToolResult-Start/Delta/End cycles before the final TextBlock answer.

---

### Edge Cases

- What happens when the DashScope API returns an error (invalid key, quota exceeded, network timeout)?
- How does the memory backend handle concurrent writes during streaming?
- What happens when session save is called mid-streaming (agent still processing)?
- How does RAG middleware behave when the embedding API and chat API use different models?
- What happens when a tool call times out during streaming?
- How does the system handle an empty knowledge base (no documents indexed)?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The integration test example MUST use DashScope as the model provider for all chat completions.
- **FR-002**: Each integration test scenario MUST be independently runnable (distinct binary or CLI subcommand).
- **FR-003**: Memory integration tests MUST demonstrate create, read, search, and delete of memory entries through agent conversation.
- **FR-004**: Session integration tests MUST demonstrate save, load, and trim operations with real conversation history.
- **FR-005**: RAG integration tests MUST demonstrate document indexing, chunk retrieval, and grounded question answering.
- **FR-006**: Streaming tool-call tests MUST verify the complete event lifecycle (start → delta(s) → end) for both tool calls and tool results.
- **FR-007**: All examples MUST accept API key via command-line argument (`--api-key`) or environment variable (`API_KEY`).
- **FR-008**: All examples MUST accept an optional model name parameter (`--model`), defaulting to `qwen-plus`.
- **FR-009**: Each test scenario MUST produce a clear pass/fail indication with human-readable output.
- **FR-010**: Error handling MUST gracefully handle API failures with descriptive error messages (not panic/unwrap).
- **FR-011**: Examples MUST NOT require any pre-existing local state to run (fresh runs must work).
- **FR-012**: The RAG example MUST use DashScope's embedding API for document vectorization.

### Key Entities

- **IntegrationTest**: A self-contained scenario that exercises a specific module's API integration, producing a pass/fail result with timing information.
- **TestResult**: Pass/fail status, scenario name, diagnostic detail, and execution duration.
- **MemoryBackend**: File-based memory storage used in memory integration tests.
- **SessionStore**: Persistent session storage used in session integration tests.
- **KnowledgeBase**: Document storage and retrieval used in RAG integration tests.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A developer can run any integration test example with a single `cargo run --example` command and a valid API key.
- **SC-002**: Each integration test completes within 60 seconds under normal network conditions (excluding first-run cold start).
- **SC-003**: All integration tests produce unambiguous pass/fail output, with failures including actionable diagnostic information.
- **SC-004**: Error scenarios (invalid API key, network failure) produce descriptive error output and exit with non-zero code rather than panicking.
- **SC-005**: The examples cover at least 3 distinct module integrations (Memory, Session, RAG) beyond the existing Agent+Tool examples.

## Assumptions

- DashScope API (Alibaba Cloud Model Studio) is accessible from the test environment.
- The DashScope API key has sufficient quota for chat completions and embedding API calls.
- File-based memory and session backends are sufficient for integration testing (no external database required).
- The `qwen-plus` model supports tool calling, which is required for calculator and multi-tool scenarios.
- Network latency to DashScope API is within typical cloud API ranges (<5s per request).
- The embedding model for RAG (`text-embedding-v3` or equivalent) is available on the DashScope account.
- Examples are not intended as benchmarks — timing is for timeout/deadline purposes only.
- The existing `common.rs` helper module may be reused or extended for shared setup logic.
