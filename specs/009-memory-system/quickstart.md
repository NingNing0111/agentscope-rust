# Quickstart: Memory System

**Feature**: 009-memory-system  
**Date**: 2026-07-30  

## Prerequisites

- Rust toolchain with `cargo` installed
- Working AgentScope Rust workspace (`cargo build` succeeds)
- Understanding of the `Memory` trait contract: [memory-trait.md](contracts/memory-trait.md)

## Scenario 1: Basic Memory CRUD (US1)

**Goal**: Write a memory entry, read it back, search for it, delete it.

### Step 1: Create a memory store

```rust
use agent_scope_memory::{FileMemory, Memory, MemoryConfig, MemoryEntry, MemoryMetadata, MemoryType};

let config = MemoryConfig::default();
let memory = FileMemory::new("/tmp/test-memory", config, None); // None = LocalBackend
```

### Step 2: Write a memory entry

```rust
let entry = MemoryEntry {
    name: "user-role".into(),
    description: "The user is a data scientist".into(),
    metadata: MemoryMetadata::new(MemoryType::User),
    content: "The user is a data scientist investigating logging infrastructure.".into(),
};

memory.write(entry).await.expect("write should succeed");
```

### Step 3: Read it back

```rust
let read_entry = memory.read("user-role").await.expect("read should succeed");
assert!(read_entry.is_some());
assert_eq!(read_entry.unwrap().content, "The user is a data scientist investigating logging infrastructure.");
```

### Step 4: Search for memories

```rust
let results = memory.search("logging", None).await.expect("search should succeed");
assert!(results.iter().any(|e| e.name == "user-role"));
```

### Step 5: Delete and verify

```rust
memory.delete("user-role").await.expect("delete should succeed");
assert!(memory.read("user-role").await.unwrap().is_none());
```

**Expected outcome**: All assertions pass. File `<memory_dir>/user-role.md` and its `MEMORY.md` index line are cleaned up.

---

## Scenario 2: Index Management (US2)

**Goal**: Verify that writing entries updates the index, and the index respects token limits.

### Step 1: Write 5 entries

```rust
for i in 0..5 {
    let entry = MemoryEntry {
        name: format!("memory-{}", i),
        description: format!("Memory entry number {}", i),
        metadata: MemoryMetadata::new(MemoryType::Project),
        content: format!("Content for memory {}", i),
    };
    memory.write(entry).await.unwrap();
}
```

### Step 2: Get index content

```rust
let index = memory.get_index_content().await.expect("index should exist");
// Should contain 5 lines like:
// - [memory-0](memory-0.md) — Memory entry number 0
// - [memory-1](memory-1.md) — Memory entry number 1
// ...
assert!(index.lines().count() >= 5);
```

### Step 3: Test token truncation

Configure a small `max_index_tokens`:

```rust
let small_config = MemoryConfig {
    max_index_tokens: 10, // very small
    ..MemoryConfig::default()
};
let small_memory = FileMemory::new("/tmp/test-memory-small", small_config, None);
// Write multiple entries...
let truncated = small_memory.get_index_content().await.unwrap();
assert!(truncated.contains("<<<TRUNCATED>>>"));
```

### Step 4: Delete and verify index update

```rust
memory.delete("memory-0").await.unwrap();
let updated_index = memory.get_index_content().await.unwrap();
assert!(!updated_index.contains("memory-0"));
```

**Expected outcome**: Index reflects all writes and deletes. Truncation kicks in when token budget is exceeded.

---

## Scenario 3: Relevance Retrieval (US3)

**Goal**: Use a mock LLM to select relevant memory files.

### Step 1: Create a mock model (for structured output)

```rust
use agent_scope_model::{ChatModel, ModelCallResult, ChatResponse};
// Refer to agent_scope_model tests for StructuredModel pattern
// The mock should return a _MemorySelection structured output
```

### Step 2: Write diverse memory entries

```rust
// Write auth-related, deploy-related, and unrelated entries
let auth_entry = MemoryEntry {
    name: "auth-bug".into(),
    description: "Authentication bug in login flow".into(),
    metadata: MemoryMetadata::new(MemoryType::Project),
    content: "The OAuth callback is failing for Google provider.".into(),
};
memory.write(auth_entry).await.unwrap();

let deploy_entry = MemoryEntry {
    name: "deploy-guide".into(),
    description: "Deployment procedure for production".into(),
    metadata: MemoryMetadata::new(MemoryType::Reference),
    content: "Deploy via `kubectl apply -f production.yaml`.".into(),
};
memory.write(deploy_entry).await.unwrap();
```

### Step 3: Retrieve relevant memories

```rust
let result = memory.retrieve_relevant(
    "I need to fix the login bug",
    &mock_model,
    5,
).await.expect("retrieval should succeed");

// Should return auth-related memory, not deploy-related
assert!(result.is_some());
assert!(result.unwrap().contains("auth-bug"));
```

### Step 4: Test irrelevance

```rust
let result = memory.retrieve_relevant(
    "what is the weather?",
    &mock_model,
    5,
).await.expect("retrieval should succeed");

assert!(result.is_none()); // nothing relevant
```

**Expected outcome**: Retrieval returns only semantically relevant memories. Irrelevant queries return empty.

---

## Scenario 4: Agent Integration (US4)

**Goal**: MemoryMiddleware injects instructions and relevant memories into agent context.

### Step 1: Set up memory middleware

```rust
use agent_scope_agent::middleware::Middleware;
use agent_scope_agent::memory_middleware::MemoryMiddleware;

let memory: Arc<dyn Memory> = Arc::new(FileMemory::new("/tmp/agent-memory", config, None));
let memory_mw = Arc::new(MemoryMiddleware::new(memory, Default::default()));
```

### Step 2: Create agent with middleware

```rust
let agent = ReActAgent::new(
    agent_config,
    react_config,
    context_config,
    vec![memory_mw], // registered as middleware
)?;
```

### Step 3: Verify system prompt injection

Check that after construction, the agent's system prompt includes memory instructions:

```rust
// Via a test hook or inspecting agent state
let state = agent.state();
// The system prompt should contain memory instructions
// (verifiable via a test-only method or event tracing)
```

### Step 4: Verify async retrieval

Send a user message and verify a HintBlock is injected:

```rust
// With a mock model that writes a memory first, then queries
let response = agent.reply(Some(vec![user_msg("What was the auth bug?")])).await?;
// The response should include context from the retrieved memory
```

**Expected outcome**: System prompt augmented with `MEMORY.md`. Relevant memories surfaced as `HintBlock` during reasoning.

---

## Running the Test Suite

```bash
# Run all memory crate tests
cargo test -p agent_scope_memory

# Run memory middleware tests
cargo test -p agent_scope_agent -- memory_middleware

# Run with output
cargo test -p agent_scope_memory -- --nocapture

# Lint
cargo clippy -p agent_scope_memory
cargo clippy -p agent_scope_agent
```

## Validation Checklist

- [ ] Write 10 entries in < 1 second (excluding model calls)
- [ ] Index for 100 files ≤ 4000 tokens
- [ ] Retrieval completes in < 2 seconds (excluding model latency)
- [ ] Listing 1000 files in < 500ms
- [ ] System prompt injection < 50ms overhead
- [ ] All CRUD operations have passing unit tests
- [ ] Edge cases covered: malformed frontmatter, missing directory, binary files, model failure
