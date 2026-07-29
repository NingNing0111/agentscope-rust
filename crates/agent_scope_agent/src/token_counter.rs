//! Token counting helper for context size estimation.
//!
//! Uses `ChatModel::count_tokens()` for estimating token consumption
//! to drive context compression decisions.
//!
//! These are helper utilities for the deferred compression integration
//! (see `context_compression.rs`). They will be used when `compress_context`
//! is wired into `react_loop.rs`.

use agent_scope_message::Msg;
use serde_json::Value as JsonValue;

/// Estimate token count for a set of messages using the model's count_tokens method.
///
/// Returns the estimated total tokens for the given messages plus any tool schemas.
#[allow(dead_code)]
pub(crate) fn estimate_tokens(
    model: &dyn agent_scope_model::ChatModel,
    messages: &[Msg],
    tools: Option<&[JsonValue]>,
) -> usize {
    model.count_tokens(messages, tools)
}

/// Check whether compression should be triggered.
///
/// Returns true when estimated tokens exceed `context_size * trigger_ratio`.
#[allow(dead_code)]
pub(crate) fn should_compress(token_count: usize, context_size: i64, trigger_ratio: f64) -> bool {
    let threshold = (context_size as f64 * trigger_ratio) as usize;
    token_count > threshold
}
