//! Middleware hook dispatch tests — US3.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use agent_scope_agent::{
    Agent, AgentConfig, AgentError, ContextConfig, Middleware, ReActAgent, ReActConfig,
};
use agent_scope_message::factory::user_msg;
use agent_scope_message::Msg;
use serde_json::Value as JsonValue;

mod mocks;
use mocks::MockModel;

// --- Test Middleware Implementations ---

/// T050: Middleware that tracks pre_reply and post_reply invocations.
struct ReplyTrackingMw {
    pre_called: AtomicBool,
    post_called: AtomicBool,
}

impl ReplyTrackingMw {
    fn new() -> Self {
        Self {
            pre_called: AtomicBool::new(false),
            post_called: AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl Middleware for ReplyTrackingMw {
    async fn pre_reply(
        &self,
        _agent_name: &str,
        _input: &mut Option<Vec<Msg>>,
    ) -> Result<(), AgentError> {
        self.pre_called.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn post_reply(
        &self,
        _agent_name: &str,
        _result: &Result<Msg, AgentError>,
    ) -> Result<(), AgentError> {
        self.post_called.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// T050: Verify pre_reply and post_reply fire correctly.
#[tokio::test]
async fn test_pre_post_reply_hooks_fire() {
    let mw = Arc::new(ReplyTrackingMw::new());
    let model = Arc::new(MockModel::new("mock", "response"));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .build()
        .unwrap();

    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![mw.clone()],
    )
    .unwrap();

    let input = user_msg("user", "hello").unwrap();
    let reply = agent.reply(Some(vec![input])).await.unwrap();
    assert!(!reply.get_text_content("").unwrap().is_empty());
    assert!(
        mw.pre_called.load(Ordering::SeqCst),
        "pre_reply should have been called"
    );
    assert!(
        mw.post_called.load(Ordering::SeqCst),
        "post_reply should have been called"
    );
}

/// T051: pre_reasoning can modify messages.
struct ModifyMessagesMw {
    modified: AtomicBool,
}

#[async_trait::async_trait]
impl Middleware for ModifyMessagesMw {
    async fn pre_reasoning(
        &self,
        _agent_name: &str,
        _messages: &mut Vec<Msg>,
        _tools: &mut Option<Vec<JsonValue>>,
    ) -> Result<(), AgentError> {
        self.modified.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn test_pre_reasoning_modifies_messages() {
    let mw = Arc::new(ModifyMessagesMw {
        modified: AtomicBool::new(false),
    });
    let model = Arc::new(MockModel::new("mock", "ok"));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .build()
        .unwrap();

    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![mw.clone()],
    )
    .unwrap();

    let input = user_msg("user", "hi").unwrap();
    let _ = agent.reply(Some(vec![input])).await.unwrap();
    assert!(mw.modified.load(Ordering::SeqCst));
}

/// T053: pre_observe fires when observe() is called.
struct ObserveTrackingMw {
    observe_called: AtomicBool,
}

#[async_trait::async_trait]
impl Middleware for ObserveTrackingMw {
    async fn pre_observe(
        &self,
        _agent_name: &str,
        _input: &mut Option<Vec<Msg>>,
    ) -> Result<(), AgentError> {
        self.observe_called.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn test_pre_observe_fires_on_observe() {
    let mw = Arc::new(ObserveTrackingMw {
        observe_called: AtomicBool::new(false),
    });
    let model = Arc::new(MockModel::new("mock", "ok"));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .build()
        .unwrap();

    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![mw.clone()],
    )
    .unwrap();

    let input = user_msg("user", "observed").unwrap();
    agent.observe(Some(vec![input])).await.unwrap();
    assert!(mw.observe_called.load(Ordering::SeqCst));
}

/// T054: Middleware FIFO order — register [A, B, C], verify each fires in order.
#[tokio::test]
async fn test_middleware_fifo_order() {
    let order = Arc::new(Mutex::new(Vec::new()));

    struct OrderedMw {
        idx: usize,
        order: Arc<Mutex<Vec<usize>>>,
    }

    #[async_trait::async_trait]
    impl Middleware for OrderedMw {
        async fn pre_reply(
            &self,
            _agent_name: &str,
            _input: &mut Option<Vec<Msg>>,
        ) -> Result<(), AgentError> {
            self.order.lock().unwrap().push(self.idx);
            Ok(())
        }
    }

    let mw_a = Arc::new(OrderedMw {
        idx: 0,
        order: order.clone(),
    });
    let mw_b = Arc::new(OrderedMw {
        idx: 1,
        order: order.clone(),
    });
    let mw_c = Arc::new(OrderedMw {
        idx: 2,
        order: order.clone(),
    });

    let model = Arc::new(MockModel::new("mock", "ok"));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .build()
        .unwrap();

    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![mw_a, mw_b, mw_c],
    )
    .unwrap();

    let input = user_msg("user", "hi").unwrap();
    let _ = agent.reply(Some(vec![input])).await.unwrap();

    let called_order = order.lock().unwrap();
    assert_eq!(
        *called_order,
        vec![0, 1, 2],
        "Middleware must fire in FIFO order"
    );
}

/// T055: Middleware implementing only one hook — others are no-ops.
#[tokio::test]
async fn test_single_hook_middleware_noop_others() {
    struct SingleHookMw;
    #[async_trait::async_trait]
    impl Middleware for SingleHookMw {
        async fn pre_observe(
            &self,
            _agent_name: &str,
            _input: &mut Option<Vec<Msg>>,
        ) -> Result<(), AgentError> {
            Ok(())
        }
    }

    let model = Arc::new(MockModel::new("mock", "ok"));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .build()
        .unwrap();

    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![Arc::new(SingleHookMw)],
    )
    .unwrap();

    // reply() should NOT crash — other hooks are no-ops
    let input = user_msg("user", "hi").unwrap();
    let reply = agent.reply(Some(vec![input])).await;
    assert!(
        reply.is_ok(),
        "Single-hook middleware should not break agent"
    );
}

/// T056: pre_reply returning Err aborts reply.
struct FailingPreReplyMw;
#[async_trait::async_trait]
impl Middleware for FailingPreReplyMw {
    async fn pre_reply(
        &self,
        _agent_name: &str,
        _input: &mut Option<Vec<Msg>>,
    ) -> Result<(), AgentError> {
        Err(AgentError::ValidationError {
            message: "blocked by middleware".into(),
        })
    }
}

#[tokio::test]
async fn test_pre_reply_error_aborts_reply() {
    let model = Arc::new(MockModel::new("mock", "ok"));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .build()
        .unwrap();

    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![Arc::new(FailingPreReplyMw)],
    )
    .unwrap();

    let input = user_msg("user", "hi").unwrap();
    let result = agent.reply(Some(vec![input])).await;
    assert!(result.is_err(), "pre_reply error should abort reply");
}
