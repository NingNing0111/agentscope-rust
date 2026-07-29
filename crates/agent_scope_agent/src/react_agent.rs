//! ReActAgent — the primary agent implementation.

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use agent_scope_event::AgentEvent;
use agent_scope_message::Msg;
use agent_scope_state::AgentState;
use futures::Stream;

use crate::agent_error::AgentError;
use crate::agent_trait::Agent;
use crate::config::{AgentConfig, ContextConfig, ReActConfig};
use crate::event_emitter::EventEmitter;
use crate::middleware::Middleware;
use crate::react_loop;

/// Shared inner state that can be cloned for spawned tasks.
struct AgentInner {
    config: AgentConfig,
    react_config: ReActConfig,
    /// Context compression configuration, wired into react_loop.
    /// See Python AgentScope's `Agent.context_config` used in `_compress_memory_if_needed()`.
    context_config: ContextConfig,
    state: RwLock<AgentState>,
    middlewares: Vec<Arc<dyn Middleware>>,
    event_emitter: EventEmitter,
    interrupted: AtomicBool,
}

/// The primary agent type — Reasoning + Acting loop with tool execution.
pub struct ReActAgent {
    inner: Arc<AgentInner>,
}

impl ReActAgent {
    /// Create a new ReActAgent with validated configuration.
    pub fn new(
        config: AgentConfig,
        react_config: ReActConfig,
        context_config: ContextConfig,
        middlewares: Vec<Arc<dyn Middleware>>,
    ) -> Result<Self, AgentError> {
        react_config.validate()?;
        context_config.validate()?;

        let agent_state = AgentState::new();

        Ok(Self {
            inner: Arc::new(AgentInner {
                config,
                react_config,
                context_config,
                state: RwLock::new(agent_state),
                middlewares,
                event_emitter: EventEmitter::new(256),
                interrupted: AtomicBool::new(false),
            }),
        })
    }

    /// Interrupt a running reply. Safe to call from any thread.
    ///
    /// The interrupted flag is automatically reset at the start of each `reply()` call.
    pub fn interrupt(&self) {
        self.inner.interrupted.store(true, Ordering::SeqCst);
    }

    /// Lock-aware state accessor.
    pub fn try_state(&self) -> std::sync::RwLockReadGuard<'_, AgentState> {
        self.inner.state.read().unwrap()
    }
}

#[async_trait::async_trait]
impl Agent for ReActAgent {
    async fn reply(&self, input: Option<Vec<Msg>>) -> Result<Msg, AgentError> {
        do_reply(&self.inner, input).await
    }

    async fn reply_stream(
        &self,
        input: Option<Vec<Msg>>,
    ) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError> {
        let mut rx = self.inner.event_emitter.subscribe();
        let inner_clone = Arc::clone(&self.inner);

        tokio::spawn(async move {
            let _ = do_reply(&inner_clone, input).await;
        });

        let stream = async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let is_reply_end = matches!(event, AgentEvent::ReplyEnd(_));
                        yield event;
                        if is_reply_end {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(lagged = n, "Event stream lagged");
                        continue;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    async fn observe(&self, input: Option<Vec<Msg>>) -> Result<(), AgentError> {
        let mut input = input;

        for mw in self.inner.middlewares.iter() {
            mw.pre_observe(&self.inner.config.name, &mut input).await?;
        }

        if let Some(msgs) = input {
            let mut state = self.inner.state.write().unwrap();
            state.context.extend(msgs);
        }

        Ok(())
    }

    fn name(&self) -> &str {
        &self.inner.config.name
    }

    fn state(&self) -> &AgentState {
        panic!("state() not directly accessible on ReActAgent; use try_state()")
    }
}

/// Shared reply implementation.
async fn do_reply(inner: &AgentInner, input: Option<Vec<Msg>>) -> Result<Msg, AgentError> {
    // Check interruption at start — clear flag after handling
    if inner.interrupted.swap(false, Ordering::SeqCst) {
        return Ok(build_interruption_msg_inline(
            &inner.react_config.interruption_message,
        ));
    }

    let session_id = {
        let state = inner.state.read().unwrap();
        state.session_id.clone()
    };

    let reply_id = uuid::Uuid::new_v4().as_simple().to_string();

    {
        let mut state = inner.state.write().unwrap();
        state.reply_context.reply_id = reply_id.clone();
        state.reply_context.cur_iter = 0;
        state.reply_context.structured_schema = None;
        state.reply_context.structured_output = None;
    }

    let mut input = input;
    for mw in inner.middlewares.iter() {
        mw.pre_reply(&inner.config.name, &mut input).await?;
    }

    if input.is_none() {
        let state = inner.state.read().unwrap();
        if state.context.is_empty() {
            return Err(AgentError::NoContentToReply);
        }
    }

    if let Some(ref msgs) = input {
        let mut state = inner.state.write().unwrap();
        state.context.extend(msgs.clone());
    }

    let result = react_loop::run_react_loop(react_loop::ReactLoopContext {
        agent_name: &inner.config.name,
        session_id: &session_id,
        reply_id: &reply_id,
        react_config: &inner.react_config,
        context_config: &inner.context_config,
        model: &inner.config.model,
        toolkit: &inner.config.toolkit,
        middlewares: &inner.middlewares,
        state: &inner.state,
        event_emitter: &inner.event_emitter,
        interrupted: &inner.interrupted,
    })
    .await;

    for mw in inner.middlewares.iter() {
        let dummy: Result<Msg, AgentError> = match &result {
            Ok(msg) => Ok(msg.clone()),
            Err(e) => Err(AgentError::ValidationError {
                message: e.to_string(),
            }),
        };
        let _ = mw.post_reply(&inner.config.name, &dummy).await;
    }

    result
}

fn build_interruption_msg_inline(message: &str) -> Msg {
    Msg::new(
        "assistant".into(),
        vec![agent_scope_message::ContentBlock::Text(
            agent_scope_message::TextBlock::new(message.into()),
        )],
        agent_scope_message::Role::Assistant,
    )
    .unwrap()
}
