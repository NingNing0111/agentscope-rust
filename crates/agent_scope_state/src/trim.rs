//! Context trimming — prune older messages to fit within configured thresholds.
//!
//! Complements `context_compression` in `agent_scope_agent` by providing
//! count-based and token-based trimming at the session state level.

use serde::{Deserialize, Serialize};

use crate::agent_state::{AgentState, SummaryContent};

/// Token counter function type: takes a message slice and returns estimated token count.
pub type TokenCounter<'a> = dyn Fn(&[agent_scope_message::Msg]) -> usize + 'a;

// ---------------------------------------------------------------------------
// TrimStrategy (T014)
// ---------------------------------------------------------------------------

/// Configuration for context trimming behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrimStrategy {
    /// Trim when context message count exceeds this value. `None` = no limit.
    pub max_messages: Option<usize>,

    /// Trim when estimated token count exceeds this value. `None` = no limit.
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

// ---------------------------------------------------------------------------
// TrimResult
// ---------------------------------------------------------------------------

/// Result of a trim operation.
#[derive(Debug, Clone)]
pub struct TrimResult {
    pub messages_before: usize,
    pub messages_after: usize,
    pub tokens_before: Option<usize>,
    pub tokens_after: Option<usize>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract text content from a ContentBlock for summary generation.
fn extract_text(block: &agent_scope_message::ContentBlock) -> Option<String> {
    use agent_scope_message::ContentBlock;
    match block {
        ContentBlock::Text(tb) => Some(tb.text.clone()),
        ContentBlock::Thinking(tb) => Some(tb.thinking.clone()),
        ContentBlock::ToolCall(tc) => Some(format!("[tool_call: {}]", tc.name)),
        ContentBlock::ToolResult(tr) => Some(format!("[tool_result: {}]", tr.name)),
        _ => None,
    }
}

/// Approximate character length of a content block, used to bound the summary
/// size across both `SummaryContent` variants.
fn text_len(block: &agent_scope_message::ContentBlock) -> usize {
    extract_text(block)
        .map(|t| t.len())
        .unwrap_or(0)
}

/// Check if a message contains a ToolCall block.
fn has_tool_call(msg: &agent_scope_message::Msg) -> bool {
    msg.content
        .iter()
        .any(|b| matches!(b, agent_scope_message::ContentBlock::ToolCall(_)))
}

/// Check if a message contains a ToolResult block.
fn has_tool_result(msg: &agent_scope_message::Msg) -> bool {
    msg.content
        .iter()
        .any(|b| matches!(b, agent_scope_message::ContentBlock::ToolResult(_)))
}

// ---------------------------------------------------------------------------
// trim_context (T015)
// ---------------------------------------------------------------------------

/// Trim context messages based on the configured strategy.
///
/// # Guarantees
///
/// - Adjacent tool-call/tool-result message pairs are kept together (not split)
/// - System messages at the start are preserved when `keep_system_messages` is true
/// - At least `keep_recent` messages are retained
/// - Trimmed content is accumulated into `state.summary`
/// - Returns `None` if no trimming was needed
///
/// # Token counting
///
/// If `token_counter_fn` is provided, token-based trimming is enabled.
/// If `None`, only message-count-based trimming applies.
pub fn trim_context(
    state: &mut AgentState,
    strategy: &TrimStrategy,
    token_counter_fn: Option<&TokenCounter<'_>>,
) -> Option<TrimResult> {
    let messages_before = state.context.len();

    // Determine if trimming is needed
    let need_trim = if let Some(max) = strategy.max_messages
        && messages_before > max
    {
        true
    } else if let Some(max_tokens) = strategy.max_tokens {
        token_counter_fn.is_some_and(|f| f(&state.context) > max_tokens)
    } else {
        false
    };

    if !need_trim {
        return None;
    }

    let tokens_before = token_counter_fn.map(|f| f(&state.context));
    let total = state.context.len();

    // Count leading system messages
    let leading_system_count = if strategy.keep_system_messages {
        let mut count = 0usize;
        for msg in state.context.iter() {
            if msg.role == agent_scope_message::Role::System {
                count += 1;
            } else {
                break;
            }
        }
        count
    } else {
        0
    };

    // Calculate which indices to trim.
    // We build a boolean vec: keep[i] = true means message i is retained.
    let mut keep = vec![false; total];

    // Always keep leading system messages
    for item in keep.iter_mut().take(leading_system_count) {
        *item = true;
    }

    // Keep the most recent `keep_recent` messages
    let keep_count = strategy.keep_recent.min(total);
    let recent_start = total.saturating_sub(keep_count);
    for item in keep.iter_mut().skip(recent_start) {
        *item = true;
    }

    // Ensure tool-call/tool-result adjacency is not split:
    // if a ToolResult is kept and the preceding message has a ToolCall, keep it too.
    let mut changed = true;
    while changed {
        changed = false;
        for i in 1..total {
            if keep[i] && !keep[i - 1] {
                // Message i is kept, message i-1 is not.
                // If the kept message has a ToolResult, also keep any preceding
                // message with a ToolCall (they form a pair).
                if has_tool_result(&state.context[i]) && has_tool_call(&state.context[i - 1]) {
                    keep[i - 1] = true;
                    changed = true;
                }
            }
        }
    }

    // Determine which messages to remove: those NOT in `keep`
    let trim_indices: Vec<usize> = (0..total).filter(|&i| !keep[i]).collect();

    if trim_indices.is_empty() {
        return None; // Nothing to trim — keep_recent already covers everything
    }

    // Accumulate trimmed content into summary
    let trimmed_texts: Vec<String> = trim_indices
        .iter()
        .map(|&i| {
            state.context[i]
                .content
                .iter()
                .filter_map(extract_text)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty())
        .collect();

    if !trimmed_texts.is_empty() {
        let new_text = trimmed_texts.join("\n");
        // Append to any existing summary instead of overwriting it, so the
        // history of prior compressions is preserved (audit M8). Cap the
        // accumulated size so a very long session cannot turn the summary
        // itself into a context-budget problem.
        const MAX_SUMMARY_CHARS: usize = 4096;
        state.summary = match &state.summary {
            SummaryContent::Text(existing) if !existing.is_empty() => {
                // Roll the window rather than replacing wholesale: when the
                // combined text exceeds the cap, keep the *tail* of the old
                // summary (plus the new text) instead of dropping the entire
                // prior history.
                let combined = format!("{existing}\n{new_text}");
                if combined.len() <= MAX_SUMMARY_CHARS {
                    SummaryContent::Text(combined)
                } else {
                    let room = MAX_SUMMARY_CHARS.saturating_sub(new_text.len() + 1);
                    let keep = if room >= existing.len() {
                        existing.clone()
                    } else {
                        // Keep the end of the old summary so the most recent
                        // compression context survives.
                        let start = existing.len() - room;
                        let truncated = &existing[start..];
                        format!("…{truncated}")
                    };
                    SummaryContent::Text(format!("{keep}\n{new_text}"))
                }
            }
            SummaryContent::Blocks(blocks) => {
                // Mirror the char cap for block summaries: drop the oldest
                // blocks while the total stays above the budget.
                let mut blocks = blocks.clone();
                blocks.push(agent_scope_message::ContentBlock::Text(
                    agent_scope_message::TextBlock::new(new_text),
                ));
                let mut total: usize = blocks.iter().map(text_len).sum();
                while total > MAX_SUMMARY_CHARS && blocks.len() > 1 {
                    let removed = text_len(&blocks[0]);
                    blocks.remove(0);
                    total = total.saturating_sub(removed);
                }
                SummaryContent::Blocks(blocks)
            }
            _ => SummaryContent::Text(new_text),
        };
    }

    // Remove trimmed messages (working backwards to keep indices valid)
    for &i in trim_indices.iter().rev() {
        state.context.remove(i);
    }

    let messages_after = state.context.len();
    let tokens_after = token_counter_fn.map(|f| f(&state.context));

    Some(TrimResult {
        messages_before,
        messages_after,
        tokens_before,
        tokens_after,
    })
}

// ---------------------------------------------------------------------------
// Tests (T016, T017)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_scope_message::{ContentBlock, Msg, Role, TextBlock};

    /// Helper: create a test message with text content.
    fn make_msg(name: &str, text: &str, role: Role) -> Msg {
        let blocks = vec![ContentBlock::Text(TextBlock::new(text.into()))];
        Msg::new(name.into(), blocks, role).unwrap()
    }

    /// Helper: fill state with N assistant messages.
    fn fill_state(state: &mut AgentState, count: usize) {
        for i in 0..count {
            let msg = make_msg(
                &format!("agent-{}", i),
                &format!("msg-{}", i),
                Role::Assistant,
            );
            state.context.push(msg);
        }
    }

    // T016: Count-based trimming
    #[test]
    fn test_context_trimming() {
        let mut state = AgentState::new();
        fill_state(&mut state, 50);

        let strategy = TrimStrategy {
            max_messages: Some(30),
            keep_recent: 20,
            ..Default::default()
        };

        let result = trim_context(&mut state, &strategy, None);
        assert!(result.is_some(), "should have trimmed");
        let r = result.unwrap();
        assert_eq!(r.messages_before, 50);
        assert!(
            r.messages_after <= 30,
            "after trim messages={} <= max_messages=30",
            r.messages_after
        );
        // At least keep_recent messages retained
        assert!(r.messages_after >= 20, "at least keep_recent retained");
    }

    // T016: No-trim when under threshold
    #[test]
    fn test_trim_context_no_trim_when_under_threshold() {
        let mut state = AgentState::new();
        fill_state(&mut state, 10);

        let strategy = TrimStrategy {
            max_messages: Some(30),
            keep_recent: 20,
            ..Default::default()
        };

        let result = trim_context(&mut state, &strategy, None);
        assert!(result.is_none(), "no trim when under threshold");
        assert_eq!(state.context.len(), 10);
    }

    // T016: Token-based trimming
    #[test]
    fn test_trim_context_token_based() {
        let mut state = AgentState::new();
        fill_state(&mut state, 50);

        // Token counter: each message = 100 tokens
        let token_fn = |msgs: &[Msg]| msgs.len() * 100;

        let strategy = TrimStrategy {
            max_tokens: Some(2000), // 20 messages worth
            keep_recent: 15,
            ..Default::default()
        };

        let result = trim_context(&mut state, &strategy, Some(&token_fn));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.messages_before, 50);
        // After trimming, tokens should be <= 2000
        let after_tokens = token_fn(&state.context);
        assert!(
            after_tokens <= 2000,
            "tokens after trim {} <= max_tokens=2000",
            after_tokens
        );
    }

    // T016: keep_recent honored
    #[test]
    fn test_trim_context_keep_recent_honored() {
        let mut state = AgentState::new();
        fill_state(&mut state, 100);

        let strategy = TrimStrategy {
            max_messages: Some(50),
            max_tokens: None,
            keep_recent: 10,
            keep_system_messages: false,
        };

        let result = trim_context(&mut state, &strategy, None);
        assert!(result.is_some());
        assert!(
            state.context.len() >= 10,
            "at least keep_recent messages retained"
        );
    }

    // T017: System messages preserved
    #[test]
    fn test_trim_preserves_system_messages() {
        let mut state = AgentState::new();

        // Add system message at start
        let sys_msg = make_msg("system", "You are a helpful assistant.", Role::System);
        state.context.push(sys_msg);

        // Add many assistant messages
        fill_state(&mut state, 50);

        assert_eq!(state.context.len(), 51);
        assert_eq!(state.context[0].role, Role::System);

        let strategy = TrimStrategy {
            max_messages: Some(20),
            max_tokens: None,
            keep_recent: 10,
            keep_system_messages: true,
        };

        trim_context(&mut state, &strategy, None);

        // System message must still be at position 0
        assert_eq!(
            state.context[0].role,
            Role::System,
            "system message should be preserved"
        );

        // Check the text content of the system message
        if let ContentBlock::Text(tb) = &state.context[0].content[0] {
            assert_eq!(tb.text, "You are a helpful assistant.");
        } else {
            panic!("expected Text block in system message");
        }
    }

    // T017: Tool call chains not broken — adjacent assistant(tool_call) + user(tool_result) pairs
    #[test]
    fn test_trim_preserves_tool_chains() {
        use agent_scope_message::{ToolCallBlock, ToolOutput, ToolResultBlock};

        let mut state = AgentState::new();

        // Fill with regular messages
        fill_state(&mut state, 40);

        // Add a tool call + tool result pair near the end
        let tc_block = ContentBlock::ToolCall(ToolCallBlock::new(
            "tc-chain-001".into(),
            "search".into(),
            "{}".into(),
        ));
        let tr_block = ContentBlock::ToolResult(ToolResultBlock::new(
            "tr-001".into(),
            "search".into(),
            ToolOutput::Text("search result".into()),
        ));

        let tc_msg = Msg::new("agent".into(), vec![tc_block], Role::Assistant).unwrap();
        let tr_msg = Msg::new("agent".into(), vec![tr_block], Role::Assistant).unwrap();

        state.context.push(tc_msg);
        state.context.push(tr_msg);
        assert_eq!(state.context.len(), 42);

        let strategy = TrimStrategy {
            max_messages: Some(10),
            max_tokens: None,
            keep_recent: 5,
            keep_system_messages: false,
        };

        trim_context(&mut state, &strategy, None);

        // The last 2 messages should be the tool-call + tool-result pair (still intact)
        let len = state.context.len();
        assert!(len >= 2, "tool chain should not be broken");

        // Find the tool call and tool result in the remaining messages
        let has_tc = state.context.iter().any(|msg| {
            msg.content.iter().any(|block| {
                if let ContentBlock::ToolCall(tc) = block {
                    tc.id == "tc-chain-001"
                } else {
                    false
                }
            })
        });
        let has_tr = state.context.iter().any(|msg| {
            msg.content
                .iter()
                .any(|block| matches!(block, ContentBlock::ToolResult(_)))
        });

        assert!(has_tc, "tool call should be preserved");
        assert!(has_tr, "tool result should be preserved");
    }

    // T017: Summary updated after trim
    #[test]
    fn test_trim_updates_summary() {
        let mut state = AgentState::new();
        fill_state(&mut state, 30);

        let strategy = TrimStrategy {
            max_messages: Some(10),
            max_tokens: None,
            keep_recent: 5,
            keep_system_messages: false,
        };

        trim_context(&mut state, &strategy, None);

        // Summary should not be empty after trimming 30→≤10 messages
        match &state.summary {
            SummaryContent::Text(t) => {
                assert!(!t.is_empty(), "summary should contain trimmed content");
            }
            _ => panic!("expected Text summary after trim"),
        }
    }
}
