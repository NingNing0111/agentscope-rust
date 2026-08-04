//! Feature 026 integration test: the runtime-state hint must reach the *same*
//! model call that triggers it (audit A1).
//!
//! The react loop previously cloned the message list before compression and
//! runtime-state injection, so the injected `<current-time>` hint was appended
//! to `state.context` but the model call used the pre-injection clone — the
//! hint was invisible on the triggering iteration (and on any single-iteration
//! reply). This test asserts the first model call actually sees the hint.

mod mocks;

use std::sync::{Arc, Mutex};

use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_message::{ContentBlock, Msg, TextBlock};
use agent_scope_message::factory::user_msg;
use agent_scope_model::{
    ChatModel, ChatResponse, ChatUsage, ModelCallResult, ModelError, ToolChoice,
};
use serde_json::Value as JsonValue;

/// A `ChatModel` that records the messages of every `call_api` invocation and
/// returns a single plain-text response.
#[derive(Clone)]
struct RecordingModel {
    calls: Arc<Mutex<Vec<Vec<Msg>>>>,
}

impl RecordingModel {
    fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn first_call_messages(&self) -> Option<Vec<Msg>> {
        self.calls.lock().unwrap().first().cloned()
    }
}

#[async_trait::async_trait]
impl ChatModel for RecordingModel {
    fn model_name(&self) -> &str {
        "recording"
    }

    fn stream_enabled(&self) -> bool {
        false
    }

    async fn call_api(
        &self,
        _model: &str,
        messages: &[Msg],
        _tools: Option<&[JsonValue]>,
        _tool_choice: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError> {
        self.calls.lock().unwrap().push(messages.to_vec());
        let mut resp = ChatResponse::default();
        resp.content.push(ContentBlock::Text(TextBlock::new("done".into())));
        resp.usage = Some(ChatUsage::default());
        Ok(ModelCallResult::Complete(resp))
    }
}

/// The first model call of a fresh reply must include the injected runtime
/// hint (it was previously built from a pre-injection clone and missed it).
#[tokio::test]
async fn first_model_call_sees_injected_hint() {
    let model = Arc::new(RecordingModel::new());
    let recording = model.clone();
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .build()
        .unwrap();

    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    let input = user_msg("user", "what time is it?").unwrap();
    let reply = agent.reply(Some(vec![input])).await.unwrap();
    assert_eq!(reply.get_text_content("").as_deref(), Some("done"));

    let first_call = recording
        .first_call_messages()
        .expect("the model must have been called");
    let has_time_hint = first_call.iter().any(|msg| {
        msg.content.iter().any(|block| match block {
            ContentBlock::Hint(h) => match &h.hint {
                agent_scope_message::HintContent::Text(t) => {
                    t.contains("<current-time>") && t.contains("<timezone>")
                }
                agent_scope_message::HintContent::Blocks(_) => false,
            },
            _ => false,
        })
    });
    assert!(
        has_time_hint,
        "the injected runtime hint must be present in the model's first call"
    );
}
