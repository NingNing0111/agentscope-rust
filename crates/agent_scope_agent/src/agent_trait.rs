//! Agent trait — the common interface for all agent types.
//!
//! Every agent type (ReActAgent, future multi-agent orchestrators, etc.)
//! implements this trait to participate in the agent ecosystem.

use std::pin::Pin;

use agent_scope_event::AgentEvent;
use agent_scope_message::Msg;
use agent_scope_state::AgentState;
use futures::Stream;

use crate::agent_error::AgentError;

/// Common interface for all agent types.
///
/// # Object safety
/// Uses `#[async_trait]` to box future return types, enabling `Arc<dyn Agent>`.
///
/// # Contract
/// - `reply(None)` with empty context returns `Err(NoContentToReply)`.
/// - `observe(Some(msgs))` appends to context without triggering a reply.
/// - `name()` and `state()` never panic.
#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    /// Process input and return the assistant's final response.
    async fn reply(&self, input: Option<Vec<Msg>>) -> Result<Msg, AgentError>;

    /// Stream reply events in real-time.
    ///
    /// The stream yields intermediate events (ModelCallStart, TextBlockDelta, etc.)
    /// and terminates after the final `ReplyEnd` event.
    async fn reply_stream(
        &self,
        input: Option<Vec<Msg>>,
    ) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError>;

    /// Observe messages without triggering a reply.
    ///
    /// Messages are appended to `state().context` for future processing.
    /// `observe(None)` is a no-op.
    async fn observe(&self, input: Option<Vec<Msg>>) -> Result<(), AgentError>;

    /// The agent's configured name.
    fn name(&self) -> &str;

    /// Read lock over the agent's runtime state.
    ///
    /// Returns a guard instead of a `&AgentState` so the contract
    /// "`state()` never panics" can be honored by agents whose state lives
    /// behind a lock (e.g. `ReActAgent`). Callers must not hold the guard
    /// across an `.await`.
    fn state(&self) -> std::sync::RwLockReadGuard<'_, AgentState>;
}
