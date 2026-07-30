# Quickstart: Session Management (Feature 010)

**Feature**: 010-session-management  
**Date**: 2026-07-30

## Prerequisites

- Rust toolchain (stable 1.75+)
- Project workspace with all crates built: `cargo build --workspace`
- Required crates compiled: `agent_scope_state`, `agent_scope_event`, `agent_scope_message`, `agent_scope_model`

## Scenario 1: Create, Use, and Close a Session

### Setup
```bash
cargo test -p agent_scope_state -- session_create_close
```

### Expected behavior
1. Create a session with a custom ID
2. Verify session ID matches
3. Verify session starts as Active
4. Append a message to session state
5. Close the session
6. Verify session is Closed
7. Verify close is idempotent (calling again is no-op)

### Expected output
```
test session::tests::test_session_create_close ... ok
```

## Scenario 2: Session Isolation

### Setup
```bash
cargo test -p agent_scope_state -- session_isolation
```

### Expected behavior
1. Create two sessions A and B
2. Append message "msg-for-A" to session A
3. Append message "msg-for-B" to session B
4. Verify session A's context contains only "msg-for-A"
5. Verify session B's context contains only "msg-for-B"
6. Close session A — session B remains Active

### Expected output
```
test session::tests::test_session_isolation ... ok
```

## Scenario 3: Save, Load, Delete (Persistence Round-Trip)

### Setup
```bash
cargo test -p agent_scope_state -- session_save_load_delete
```

### Expected behavior
1. Create a session and add 5 messages
2. Save session to InMemorySessionStore
3. Load session from store using same ID
4. Verify restored session has 5 messages
5. Verify message content is identical
6. Verify session_id is preserved
7. Delete the persisted session
8. Verify load after delete returns NotFound

### Expected output
```
test session_store::tests::test_save_load_delete_roundtrip ... ok
```

## Scenario 4: Context Trimming

### Setup
```bash
cargo test -p agent_scope_state -- context_trimming
```

### Expected behavior
1. Create a session with 50 messages
2. Apply TrimStrategy { max_messages: 30, keep_recent: 20 }
3. Verify context is trimmed to ≤ 30 messages
4. Verify the 20 most recent messages are preserved
5. Verify tool call/tool result pairs are intact
6. Verify summary field contains trimmed content

### Expected output
```
test trim::tests::test_trim_context_count_based ... ok
test trim::tests::test_trim_preserves_tool_chains ... ok
test trim::tests::test_trim_preserves_system_messages ... ok
```

## Scenario 5: Session Events

### Setup
```bash
cargo test -p agent_scope_event -- session_events
```

### Expected behavior
1. Create a session → emit SessionCreatedEvent
2. Save the session → emit SessionSavedEvent
3. Load the session → emit SessionLoadedEvent
4. Trim context → emit SessionTrimmedEvent
5. Close session → emit SessionClosedEvent
6. All events have correct session_id
7. All events serialize/deserialize correctly (round-trip)

### Expected output
```
test session_events::tests::test_session_events_serialization ... ok
test session_events::tests::test_session_events_roundtrip ... ok
```

## Scenario 6: Full Lifecycle (Integration)

### Setup
```bash
cargo test -p agent_scope_state -- session_full_lifecycle
```

### Expected behavior
1. Create session → verify Active
2. Add messages → verify context grows
3. Save → verify persisted
4. Load into new session object → verify state identical
5. Trim → verify messages reduced, tool chains intact
6. Continue appending new messages after trim
7. Save updated state → load → verify
8. Close → verify Closed, further ops rejected

### Expected output
```
test session::tests::test_full_lifecycle ... ok
```

## Running All Session Tests

```bash
# All session-related tests across affected crates
cargo test -p agent_scope_state -- session
cargo test -p agent_scope_state -- trim
cargo test -p agent_scope_state -- session_store
cargo test -p agent_scope_event -- session_events

# Or all at once
cargo test -p agent_scope_state -p agent_scope_event
```

## Workspace Validation

```bash
# Full workspace — ensure no regressions
cargo test --workspace
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```
