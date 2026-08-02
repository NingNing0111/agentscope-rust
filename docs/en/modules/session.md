# Session Management / Session

> One-liner: `agent_scope_state` provides Agent runtime state management — `Session` wraps AgentState lifecycle with structured concurrency isolation, `AgentState` maintains context messages, summaries, and ReplyContext, while `SessionStore` enables session persistence and cross-request recovery.

## 1. Module Overview (Overview)

This module lives in the `agent_scope_state` crate and provides:

| Component | Responsibility |
|-----------|---------------|
| `Session` / `SessionImpl` | Session lifecycle (create, activate, close), CancellationToken isolation, AgentState access |
| `AgentState` | Context message list, summaries (`SummaryContent`), `ReplyContext`, permission context |
| `SessionStore` / `InMemorySessionStore` | Session persistence with CRUD and cross-request recovery |
| `TokenCounter` / `TrimStrategy` | Token-based context trimming to stay within the model's context window |
| `PermissionContext` / `PermissionRule` | Runtime context and rules for tool call permissions |

**When to use**: managing multi-turn conversation context; recovering session state across requests; isolating AgentState per user/tenant; automatic context trimming when token budget is exceeded.

**Prerequisites**: read [Agent System](./agent.md) and [Message & Basic Types](./message-types.md) first.

## 2. Core Concepts & Main Public Types (Core Concepts)

### 2.1 `Session` trait and `SessionImpl`

`Session` is the abstract session interface:

| Method | Description |
|--------|-------------|
| `session_id()` | Returns the unique session ID |
| `meta()` | Returns `SessionMeta` (creation time, status, user_id) |
| `state()` | Returns a read-only reference to `&AgentState` |
| `state_mut()` | Returns a mutable reference to `&mut AgentState` |
| `cancel_token()` | Returns `CancellationToken` for cancel propagation |
| `close()` | Closes the session, cancels all child tasks |

`SessionImpl` is the built-in implementation, constructed via `SessionImpl::new(meta)`.

### 2.2 `AgentState`

Each Session holds an `AgentState` containing:

| Field | Type | Description |
|-------|------|-------------|
| `context` | `Vec<Msg>` | Dialog context message list |
| `summary` | `SummaryContent` | Summary of compressed messages (`Text` or `BulletPoints`) |
| `reply_context` | `ReplyContext` | Current reply context (reply_id, tool call state, etc.) |
| `permission_context` | `PermissionContext` | Tool permission context |

**Key methods**:
- `append_context(msg)` — appends a message to context
- `get_messages_for_model()` — retrieves messages suitable for sending to the model (with summary injection)

### 2.3 `SessionStore`

| Method | Description |
|--------|-------------|
| `create(meta)` | Creates and stores a new session |
| `get(session_id)` | Loads a session by ID |
| `list()` | Lists all session metas |
| `update_meta(session_id, meta)` | Updates session metadata |
| `delete(session_id)` | Deletes session and its state |
| `save_state(session_id, state)` | Persists AgentState |
| `load_state(session_id)` | Loads persisted AgentState |

### 2.4 Context Trimming

| Type | Description |
|------|-------------|
| `TokenCounter` | Token counting trait; `SimpleTokenCounter` estimates based on character count |
| `TrimStrategy` | Trimming strategy: `KeepLast(n)`, `TailPercent(p)` |
| `trim_context(state, strategy, counter)` | Executes trimming, records `SummaryContent` |

Trimming records `SummaryContent::Text` noting how many messages were removed.

## 3. Quick Example (Quick Example)

```rust
use agent_scope_state::{
    SessionImpl, SessionMeta, Session, SessionStore, InMemorySessionStore,
    AgentState,
};

// Create a session
let meta = SessionMeta::new("user-001");
let mut session = SessionImpl::new(meta);

// Append messages to context
use agent_scope_message::factory::user_msg;
session.state_mut().append_context(
    user_msg("user-001", "Hello!").unwrap()
).unwrap();

// Persist to store
let store = InMemorySessionStore::new();
store.create(session.meta().clone())?;
store.save_state(session.session_id(), session.state())?;

// Recover from store
let loaded = store.get("session-id")?;
let state = store.load_state("session-id")?;
```

## 4. Key Usage Patterns (Usage Patterns)

### 4.1 Context Trimming

When context exceeds the model window, apply a trimming strategy:

```rust
use agent_scope_state::{trim_context, TrimStrategy, SimpleTokenCounter};

let counter = SimpleTokenCounter::default();
let strategy = TrimStrategy::TailPercent(0.7); // keep last 70%
trim_context(&mut state, &strategy, &counter);
// state.summary will contain a trimming note
```

### 4.2 Permission Checking

```rust
use agent_scope_state::{PermissionContext, PermissionRule};

let ctx = PermissionContext::builder()
    .rule(PermissionRule::AllowTool { tool_name: "read".into() })
    .rule(PermissionRule::DenyTool { tool_name: "delete".into() })
    .build();
// Inject into AgentConfig; the Agent checks permissions before each tool call
```

### 4.3 Structured Concurrency Isolation

Each `SessionImpl` internally holds a `CancellationToken`:

```rust
let token = session.cancel_token().clone();
tokio::spawn(async move {
    tokio::select! {
        _ = token.cancelled() => {
            // Clean up resources
        }
        result = long_running_task() => {
            // Completed normally
        }
    }
});
session.close(); // triggers cancel on all child tasks
```

## 5. Errors & Unsupported Capabilities (Errors & Unsupported)

| Error | Cause | Recommendation |
|-------|-------|----------------|
| `SessionError::Closed` | Operating on a closed session | Create a new session |
| `SessionError::AlreadyExists` | Duplicate session ID | Use a different ID |
| `SessionError::NotFound` | Not found in store | Verify the ID |
| `SessionError::SerializationError` | State serialization failure | Check state data structures |
| `SessionError::StorageError` | Storage backend error | Check storage availability |
| `SessionError::InvalidTrimConfig` | Invalid trim config (e.g., percentage > 1.0) | Fix the configuration |

**Unsupported**:
- Only `InMemorySessionStore` is built-in; persistent stores (SQLite, Redis) require custom implementation.
- Distributed session sharing is out of scope.
- No automatic session expiry/TTL mechanism.

## 6. Compatibility (Compatibility)

- **Compatibility Level**: **L1** (`SessionMeta`, `SessionStatus`, `AgentState` data protocol); **L2** (session CRUD, context trimming, permission model)
- **Authority**: `specs/010-session-management/spec.md`
- **Known Deviations**:
  - Rust side uses `SessionImpl` rather than Python's inheritance hierarchy
  - Context trimming currently uses character-count estimation rather than a precise tokenizer
  - `SessionStore` is a Rust-side abstraction layer not present in Python

## 7. See Also (Related Modules)

- [Agent System](./agent.md) — How Session is used within Agents
- [Memory](./memory.md) — Long-term memory (Session manages short-term context)
- [Message & Basic Types](./message-types.md) — `Msg` and `ContentBlock`
