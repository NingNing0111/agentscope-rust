//! ReActAgent — the primary agent implementation.

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use agent_scope_event::AgentEvent;
use agent_scope_message::Msg;
use agent_scope_state::{
    AgentState, JsonFileSessionStore, Session, SessionError, SessionImpl, SessionStore,
};
use futures::Stream;
use tokio::sync::{Mutex as AsyncMutex, mpsc, oneshot};

use crate::agent_error::AgentError;
use crate::agent_trait::Agent;
use crate::config::{AgentConfig, ContextConfig, ReActConfig};
use crate::event_emitter::EventEmitter;
use crate::middleware::Middleware;
use crate::react_loop;
use crate::stream_handle::StreamHandle;
use crate::streaming_reactor;
use crate::task_tools::{TaskCreateTool, TaskGetTool, TaskListTool, TaskUpdateTool};
use tokio_util::sync::CancellationToken;

/// Shared inner state that can be cloned for spawned tasks.
pub(crate) struct AgentInner {
    pub(crate) config: AgentConfig,
    pub(crate) react_config: ReActConfig,
    pub(crate) context_config: ContextConfig,
    pub(crate) state: Arc<RwLock<AgentState>>,
    pub(crate) middlewares: Vec<Arc<dyn Middleware>>,
    pub(crate) event_emitter: EventEmitter,
    pub(crate) interrupted: Arc<AtomicBool>,
    pub(crate) is_streaming: Arc<AtomicBool>,
    /// Session storage backend for agent-state persistence (Feature 025).
    /// `None` when persistence is fully disabled.
    pub(crate) session_store: Option<Arc<dyn SessionStore>>,
    /// Serializes persistence writes so concurrent saves (e.g. a cancelled
    /// streaming reply followed by a batch reply) cannot race on the same
    /// session file.
    pub(crate) persist_lock: AsyncMutex<()>,
    /// The original session's creation time, retained so auto-saves preserve
    /// it instead of resetting it to "now" on every reply (round-4 F3).
    /// `None` for a freshly constructed session.
    pub(crate) session_created_at: Option<chrono::DateTime<chrono::Utc>>,
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
    ///
    /// Persistence fields in [`AgentConfig`] are honored: a session store is
    /// resolved (the default `JsonFileSessionStore` rooted at `sessions/` when
    /// none is injected) so replies auto-persist, and `auto_persist(false)`
    /// disables all storage writes. This synchronous constructor does **not**
    /// resume an existing session from a `session_id` — use the asynchronous
    /// [`ReActAgent::build`] for session resume.
    pub fn new(
        config: AgentConfig,
        react_config: ReActConfig,
        context_config: ContextConfig,
        middlewares: Vec<Arc<dyn Middleware>>,
    ) -> Result<Self, AgentError> {
        let agent_state = match &config.session_id {
            Some(id) => AgentState::with_session_id(id.clone()),
            None => AgentState::new(),
        };
        Self::construct(
            config,
            react_config,
            context_config,
            middlewares,
            agent_state,
            // A fresh agent has no persisted creation time; auto-saves stamp it now.
            None,
        )
    }

    /// Build a ReActAgent asynchronously, resuming any persisted session state.
    ///
    /// When `config.session_id` is set, the resolved session store is queried:
    /// an existing session is resumed with its full state, a missing one is
    /// created fresh, and any other store error (corruption, I/O) fails
    /// construction with a typed [`AgentError`] rather than silently degrading
    /// (spec FR-005 / contracts/agent-config.md).
    pub async fn build(
        config: AgentConfig,
        react_config: ReActConfig,
        context_config: ContextConfig,
        middlewares: Vec<Arc<dyn Middleware>>,
    ) -> Result<Self, AgentError> {
        let mut config = config;

        // Resolve the store first so it can be queried for resume and then
        // retained on the agent for auto-persist after replies.
        let session_store: Arc<dyn SessionStore> = config
            .session_store
            .clone()
            .unwrap_or_else(|| Arc::new(JsonFileSessionStore::with_default_dir()));

        let mut session_created_at = None;
        let agent_state = match &config.session_id {
            Some(id) => match session_store.load(id).await {
                Ok(session) => {
                    // Retain the original creation time so later auto-saves do
                    // not reset it to "now" (round-4 F3).
                    session_created_at = Some(session.created_at());
                    let mut state = session.state().clone();
                    state.session_id = id.clone();
                    state
                }
                // Missing session id = create a new session, not an error.
                Err(SessionError::NotFound { .. }) => AgentState::with_session_id(id.clone()),
                Err(e) => return Err(e.into()),
            },
            None => AgentState::new(),
        };

        config.session_store = Some(session_store);
        Self::construct(
            config,
            react_config,
            context_config,
            middlewares,
            agent_state,
            session_created_at,
        )
    }

    /// Shared construction core.
    fn construct(
        mut config: AgentConfig,
        react_config: ReActConfig,
        context_config: ContextConfig,
        middlewares: Vec<Arc<dyn Middleware>>,
        agent_state: AgentState,
        session_created_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Self, AgentError> {
        react_config.validate()?;
        context_config.validate()?;

        // Validate the runtime-state injection config against the real context
        // compression trigger ratio (Feature 026, FR-014).
        config
            .injection_config
            .validate_with_trigger(context_config.trigger_ratio)?;

        let session_store = resolve_store(&config);
        let state = Arc::new(RwLock::new(agent_state));
        let stream_channel_capacity = config.stream_channel_capacity;

        // Register the built-in task planning tools at construction time so
        // they share the agent's state handle (default enabled, see
        // `AgentConfig::task_tools_enabled`).
        if config.task_tools_enabled {
            let mut toolkit = config.toolkit.take().unwrap_or_default();
            toolkit.register(TaskCreateTool::new(Arc::clone(&state)));
            toolkit.register(TaskListTool::new(Arc::clone(&state)));
            toolkit.register(TaskGetTool::new(Arc::clone(&state)));
            toolkit.register(TaskUpdateTool::new(Arc::clone(&state)));
            config.toolkit = Some(toolkit);
        }

        Ok(Self {
            inner: Arc::new(AgentInner {
                config,
                react_config,
                context_config,
                state,
                middlewares,
                event_emitter: EventEmitter::new(stream_channel_capacity),
                interrupted: Arc::new(AtomicBool::new(false)),
                is_streaming: Arc::new(AtomicBool::new(false)),
                session_store,
                persist_lock: AsyncMutex::new(()),
                session_created_at,
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

    fn state(&self) -> std::sync::RwLockReadGuard<'_, AgentState> {
        self.inner.state.read().unwrap()
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

/// Resolve the session store for an agent config.
///
/// An explicitly injected store wins; otherwise the built-in
/// `JsonFileSessionStore` rooted at `sessions/` is used (Feature 025).
fn resolve_store(config: &AgentConfig) -> Option<Arc<dyn SessionStore>> {
    Some(config.session_store.clone().unwrap_or_else(|| {
        Arc::new(JsonFileSessionStore::with_default_dir()) as Arc<dyn SessionStore>
    }))
}

/// Persist the agent's latest state after a reply finishes.
///
/// Honors `auto_persist` — no writes occur when it is disabled (spec FR-007 /
/// SC-007). A failed save is reported through tracing but does not break the
/// reply result already produced (spec FR-006). Saves are serialized via the
/// agent's `persist_lock` so concurrent saves never race on the same file.
pub(crate) async fn persist_after_reply(inner: &Arc<AgentInner>) {
    if !inner.config.auto_persist {
        return;
    }
    let store = match &inner.session_store {
        Some(store) => Arc::clone(store),
        None => return,
    };

    // Snapshot AFTER acquiring the lock so an older snapshot can never be
    // written over a newer one by a racing save from a finished streaming
    // reply (audit round-4 M5). The lock serializes saves, so the last save
    // always carries the newest state.
    let _guard = inner.persist_lock.lock().await;
    let state = inner.state.read().unwrap().clone();
    let session = build_session_for_persist(inner, state);

    if let Err(e) = store.save(&session).await {
        tracing::warn!(
            session_id = %session.id(),
            error = %e,
            "automatic session persistence failed after reply"
        );
    }
}

/// Persist the agent's latest state on a short-lived background task.
///
/// Used by the streaming path: the reactor must exit promptly so `is_streaming`
/// clears and the agent is immediately reusable after the event stream ends.
/// The save is serialized via `persist_lock`, so it cannot race a following
/// reply's save on the same session file.
pub(crate) fn spawn_persist_after_reply(inner: &Arc<AgentInner>) {
    if !inner.config.auto_persist {
        return;
    }
    let store = match &inner.session_store {
        Some(store) => Arc::clone(store),
        None => return,
    };
    let inner = Arc::clone(inner);

    tokio::spawn(async move {
        // Snapshot AFTER acquiring the lock so an older snapshot cannot be
        // written over a newer save from a following reply (audit round-4 M5).
        let _guard = inner.persist_lock.lock().await;
        let state = inner.state.read().unwrap().clone();
        let session = build_session_for_persist(&inner, state);
        if let Err(e) = store.save(&session).await {
            tracing::warn!(
                session_id = %session.id(),
                error = %e,
                "background session persistence failed after streaming reply"
            );
        }
    });
}

/// Construct the `SessionImpl` for an auto-save, preserving the original
/// session creation time (round-4 F3) instead of letting `SessionImpl::new`
/// stamp it with the current time on every save.
fn build_session_for_persist(
    inner: &Arc<AgentInner>,
    state: AgentState,
) -> SessionImpl {
    if let Some(created_at) = inner.session_created_at {
        SessionImpl::new(state).with_persisted_timestamps(created_at, chrono::Utc::now())
    } else {
        SessionImpl::new(state)
    }
}

/// Batch reply: uses react_loop with mpsc channel.
async fn do_reply(inner: Arc<AgentInner>, input: Option<Vec<Msg>>) -> Result<Msg, AgentError> {
    let cancel_token = fresh_cancel_token(&inner);

    if inner.interrupted.swap(false, Ordering::SeqCst) {
        // Checkpoint the pre-reply state so an interrupted agent still
        // persists the latest state (spec FR-006).
        persist_after_reply(&inner).await;
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
        permission_context: &inner.config.permission_context,
        middlewares: &inner.middlewares,
        state: &inner.state,
        interrupted: &inner.interrupted,
        cancel_token: &cancel_token,
        task_tools_enabled: inner.config.task_tools_enabled,
        injection_config: &inner.config.injection_config,
    };

    let result = react_loop::run_react_loop(ctx, &event_tx).await;

    drop(event_tx);
    // Drainer exits when the sender is dropped (channel closed).
    let _ = drain_handle.await;

    // Auto-persist the latest state after the reply ends (normal or
    // interrupted). Save failures are reported but never break the result.
    persist_after_reply(&inner).await;

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
        // Checkpoint the pre-reply state so an interrupted agent still
        // persists the latest state (spec FR-006).
        spawn_persist_after_reply(&inner);
        // Streaming has no `Msg` result to return when interrupted before any
        // reply starts, so it surfaces `Err(CancellationError)` — unlike the
        // batch `reply()` which returns the interruption message as `Ok`. This
        // asymmetry is intentional and documented by the recovery test
        // (round-4 M4 re-examined: design difference, not a defect).
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
