# Contract: Context Trimming

**Feature**: 010-session-management  
**Crate**: `agent_scope_state`  
**File**: `src/trim.rs`

## Interface

```rust
use serde::{Deserialize, Serialize};

use crate::agent_state::AgentState;

/// Configuration for context trimming behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrimStrategy {
    /// Trim when context message count exceeds this value. None = no limit.
    pub max_messages: Option<usize>,

    /// Trim when estimated token count exceeds this value. None = no limit.
    pub max_tokens: Option<usize>,

    /// Number of recent messages to always retain.
    pub keep_recent: usize,

    /// Whether to preserve system-role messages at context start.
    pub keep_system_messages: bool,
}

impl Default for TrimStrategy {
    fn default() -> Self {
        Self {
            max_messages: None,
            max_tokens: None,
            keep_recent: 20,
            keep_system_messages: true,
        }
    }
}

/// Result of a trim operation.
#[derive(Debug, Clone)]
pub struct TrimResult {
    pub messages_before: usize,
    pub messages_after: usize,
    pub tokens_before: Option<usize>,
    pub tokens_after: Option<usize>,
}
```

## Function Contract

```rust
/// Trim context messages based on the configured strategy.
///
/// # Guarantees
/// - Tool call/tool result pairs are kept together (atomic units)
/// - System messages at the start are preserved when keep_system_messages is true
/// - At least keep_recent messages are retained
/// - Trimmed content is accumulated into state.summary
/// - Returns TrimResult with before/after counts
///
/// # Token counting
/// If token_counter_fn is provided, token-based trimming is enabled.
/// If None, only message-count-based trimming applies.
pub fn trim_context(
    state: &mut AgentState,
    strategy: &TrimStrategy,
    token_counter_fn: Option<&dyn Fn(&[agent_scope_message::Msg]) -> usize>,
) -> Option<TrimResult>;
```

## Algorithm Contract

1. Check if trimming is needed:
   - If `max_messages` is set and `context.len() > max_messages`: trim needed
   - If `max_tokens` is set and token_counter is available and count > max_tokens: trim needed
   - Otherwise: return `None` (no trim needed)

2. Identify messages to retain:
   - **System messages**: If `keep_system_messages`, mark all System-role messages at context start
   - **Recent messages**: Walk from the end, counting toward `keep_recent`
   - **Tool call chains**: If a ToolResult is retained, its corresponding ToolCall MUST also be retained

3. Remove trimmed messages:
   - Move trimmed message content text into `state.summary` (as SummaryContent::Text)
   - Remove trimmed messages from `state.context`

4. Return `TrimResult` with before/after statistics

## Usage Contract

```rust
let mut state = AgentState::new();
// ... add 50 messages ...

let strategy = TrimStrategy {
    max_messages: Some(30),
    keep_recent: 20,
    ..Default::default()
};

let result = trim_context(&mut state, &strategy, Some(&|msgs| msgs.len() * 100));
if let Some(r) = result {
    assert!(r.messages_after <= 30);
    assert_eq!(r.messages_before, 50);
}
```

## Guarantees

- **G1**: After trimming with `max_messages`, `context.len() <= max_messages`
- **G2**: After trimming with `max_tokens`, `token_count <= max_tokens`
- **G3**: Tool call/tool result pairs are never split — if one end is kept, both are kept
- **G4**: System messages at context start are preserved when `keep_system_messages = true`
- **G5**: At least `keep_recent` messages are always retained (unless total < keep_recent)
- **G6**: Trim is a pure state mutation — no I/O, no async
- **G7**: `summary` field accumulates trimmed content (text form)
