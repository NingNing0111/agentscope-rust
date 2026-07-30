//! ReActAgent — the primary agent implementation.

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

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
use crate::stream_handle::StreamHandle;
use crate::streaming_reactor;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

/// Shared inner state that can be cloned for spawned tasks.
pub(crate) struct AgentInner {
    pub(crate) config: AgentConfig,
    pub(crate) react_config: ReActConfig,
    pub(crate) context_config: ContextConfig,
    pub(crate) state: RwLock<AgentState>,
    pub(crate) middlewares: Vec<Arc<dyn Middleware>>,
    pub(crate) event_emitter: EventEmitter,
    pub(crate) interrupted: Arc<AtomicBool>,
    pub(crate) is_streaming: Arc<AtomicBool>,
    /// Cancellation token for interrupting in-progress model calls / stream consumption.
    /// Wrapped in Mutex so it can be replaced with a fresh token after cancellation
    /// (CancellationToken::cancel() is irreversible).
    pub(crate) cancel_token: Mutex<CancellationToken>,
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
        let stream_channel_capacity = config.stream_channel_capacity;

        Ok(Self {
            inner: Arc::new(AgentInner {
                config,
                react_config,
                context_config,
                state: RwLock::new(agent_state),
                middlewares,
                event_emitter: EventEmitter::new(stream_channel_capacity),
                interrupted: Arc::new(AtomicBool::new(false)),
                is_streaming: Arc::new(AtomicBool::new(false)),
                cancel_token: Mutex::new(CancellationToken::new()),
            }),
        })
    }

    /// Interrupt a running reply (streaming or batch).
    ///
    /// Sets the `interrupted` flag and cancels the current token. After the
    /// current reply terminates, the next `reply()` / `reply_stream()` call
    /// will automatically create a fresh token so it can run normally.
    /// Safe to call from any thread.
    pub fn interrupt(&self) {
        self.inner.interrupted.store(true, Ordering::SeqCst);
        self.inner.cancel_token.lock().unwrap().cancel();
    }

    /// Lock-aware state accessor.
    pub fn try_state(&self) -> std::sync::RwLockReadGuard<'_, AgentState> {
        self.inner.state.read().unwrap()
    }
}

/// Consumer-facing event stream returned by `reply_stream()`.
pub struct EventStream {
    rx: mpsc::Receiver<AgentEvent>,
    cancel_tx: Option<oneshot::Sender<()>>,
    /// Keeps a reference to the shared is_streaming flag so it can be
    /// inspected elsewhere, though cleanup is now handled by StreamHandle::Drop.
    #[allow(dead_code)]
    is_streaming: Arc<AtomicBool>,
}

impl Stream for EventStream {
    type Item = AgentEvent;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

impl Drop for EventStream {
    fn drop(&mut self) {
        // Fire the cancellation signal (drop oneshot sender).
        // The reactor checks StreamHandle::is_cancelled() each iteration
        // and will exit shortly. is_streaming is cleared when the reactor
        // task's StreamHandle is dropped — NOT here, to prevent the race
        // where a new reply()/reply_stream() starts before the old reactor
        // has actually exited (P0-2 fix).
        drop(self.cancel_tx.take());
    }
}

#[async_trait::async_trait]
impl Agent for ReActAgent {
    async fn reply(&self, input: Option<Vec<Msg>>) -> Result<Msg, AgentError> {
        if self
            .inner
            .is_streaming
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(AgentError::AlreadyStreaming);
        }

        let _guard = StreamingGuard(Arc::clone(&self.inner.is_streaming));
        do_reply(Arc::clone(&self.inner), input).await
    }

    async fn reply_stream(
        &self,
        input: Option<Vec<Msg>>,
    ) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError> {
        if self
            .inner
            .is_streaming
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(AgentError::AlreadyStreaming);
        }

        match do_reply_stream(Arc::clone(&self.inner), input).await {
            Ok(stream) => Ok(stream),
            Err(e) => {
                self.inner.is_streaming.store(false, Ordering::SeqCst);
                Err(e)
            }
        }
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

struct StreamingGuard(Arc<AtomicBool>);

impl Drop for StreamingGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Get (or reset) the cancel token for a new reply.
///
/// `CancellationToken::cancel()` is one-shot and irreversible, so if the
/// previous token has been cancelled we replace it with a fresh one so the
/// next `reply()` / `reply_stream()` can run normally.
fn fresh_cancel_token(inner: &Arc<AgentInner>) -> CancellationToken {
    let mut guard = inner.cancel_token.lock().unwrap();
    if guard.is_cancelled() {
        *guard = CancellationToken::new();
    }
    guard.clone()
}

/// Batch reply: uses react_loop with mpsc channel.
async fn do_reply(inner: Arc<AgentInner>, input: Option<Vec<Msg>>) -> Result<Msg, AgentError> {
    let cancel_token = fresh_cancel_token(&inner);

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
        mw.pre_reply(&inner.config.name, &mut input, &inner.config.model)
            .await?;
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

    let mut system_prompt = inner.config.system_prompt.clone();
    for mw in inner.middlewares.iter() {
        mw.on_system_prompt(&inner.config.name, &mut system_prompt)
            .await?;
    }

    let (event_tx, event_rx) = inner.event_emitter.create_channel();

    // Spawn a background drainer to prevent deadlock when a bounded channel
    // fills up. The reactor `send().await` blocks until there is capacity;
    // the drainer consumes events concurrently so the reactor never stalls.
    let drain_handle = tokio::spawn(async move {
        let mut rx = event_rx;
        while rx.recv().await.is_some() {}
    });

    let ctx = react_loop::ReactLoopContext {
        agent_name: &inner.config.name,
        session_id: &session_id,
        reply_id: &reply_id,
        system_prompt: &system_prompt,
        react_config: &inner.react_config,
        context_config: &inner.context_config,
        model: &inner.config.model,
        toolkit: &inner.config.toolkit,
        middlewares: &inner.middlewares,
        state: &inner.state,
        interrupted: &inner.interrupted,
        cancel_token: &cancel_token,
    };

    let result = react_loop::run_react_loop(ctx, &event_tx).await;

    drop(event_tx);
    // Drainer exits when the sender is dropped (channel closed).
    let _ = drain_handle.await;

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

/// Streaming reply: spawns reactor in background, returns EventStream.
async fn do_reply_stream(
    inner: Arc<AgentInner>,
    input: Option<Vec<Msg>>,
) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError> {
    let cancel_token = fresh_cancel_token(&inner);

    if inner.interrupted.swap(false, Ordering::SeqCst) {
        return Err(AgentError::CancellationError {
            reply_id: "pre-reply-interrupted".into(),
        });
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
        mw.pre_reply(&inner.config.name, &mut input, &inner.config.model)
            .await?;
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

    let mut system_prompt = inner.config.system_prompt.clone();
    for mw in inner.middlewares.iter() {
        mw.on_system_prompt(&inner.config.name, &mut system_prompt)
            .await?;
    }

    let (event_tx, event_rx) = inner.event_emitter.create_channel();
    let (stream_handle, cancel_tx) = StreamHandle::new(Arc::clone(&inner.is_streaming));
    let is_streaming = Arc::clone(&inner.is_streaming);

    // Clone inner for the spawned task
    let spawned_inner = Arc::clone(&inner);
    let session_id_for_spawn = session_id.clone();
    let reply_id_for_spawn = reply_id.clone();

    let spawned_cancel = cancel_token.clone();
    tokio::spawn(async move {
        streaming_reactor::run_streaming_loop(
            spawned_inner,
            session_id_for_spawn,
            reply_id_for_spawn,
            system_prompt,
            stream_handle,
            event_tx,
            spawned_cancel,
        )
        .await;
    });

    Ok(Box::pin(EventStream {
        rx: event_rx,
        cancel_tx: Some(cancel_tx),
        is_streaming,
    }))
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
