//! Test utilities: MockModel and ScriptedModel.
//!
//! These provide deterministic model responses for testing the agent system
//! without requiring live LLM API calls (per Constitution Article 6).

use std::sync::Mutex;

use agent_scope_message::{ContentBlock, Msg, TextBlock, ToolCallBlock};
use agent_scope_model::{
    ChatModel, ChatResponse, ChatUsage, ModelCallResult, ModelError, ToolChoice,
};
use serde_json::Value as JsonValue;

// ---------------------------------------------------------------------------
// MockModel
// ---------------------------------------------------------------------------

/// A mock ChatModel that always returns a preset text response.
///
/// Useful for US1 tests where the agent only needs a text reply.
pub struct MockModel {
    name: String,
    response_text: String,
    #[allow(dead_code)]
    block_id: String,
}

impl MockModel {
    /// Create a new MockModel that returns `response_text` in a TextBlock.
    pub fn new(name: impl Into<String>, response_text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            response_text: response_text.into(),
            block_id: uuid::Uuid::new_v4().as_simple().to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ChatModel for MockModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn stream_enabled(&self) -> bool {
        false
    }

    async fn call_api(
        &self,
        _model: &str,
        _messages: &[Msg],
        _tools: Option<&[JsonValue]>,
        _tool_choice: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError> {
        let mut resp = ChatResponse::default();
        let tb = TextBlock::new(self.response_text.clone());
        resp.content.push(ContentBlock::Text(tb));
        resp.usage = Some(ChatUsage::default());
        Ok(ModelCallResult::Complete(resp))
    }
}

// ---------------------------------------------------------------------------
// ScriptedModel
// ---------------------------------------------------------------------------

/// Response script entry — either a text response or a tool call.
#[derive(Debug, Clone)]
pub enum ScriptedResponse {
    Text(String),
    ToolCall {
        id: String,
        name: String,
        input: String,
    },
}

/// A mock ChatModel that returns a pre-scripted sequence of responses.
///
/// Each call to `call_api()` consumes the next response in the script.
/// After the last script entry, it returns an empty response.
pub struct ScriptedModel {
    name: String,
    script: Mutex<Vec<ScriptedResponse>>,
    call_count: Mutex<usize>,
}

impl ScriptedModel {
    /// Create a new ScriptedModel with the given script.
    pub fn new(name: impl Into<String>, script: Vec<ScriptedResponse>) -> Self {
        Self {
            name: name.into(),
            script: Mutex::new(script),
            call_count: Mutex::new(0),
        }
    }

    /// Number of times `call_api` has been invoked.
    pub fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap()
    }
}

#[async_trait::async_trait]
impl ChatModel for ScriptedModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn stream_enabled(&self) -> bool {
        false
    }

    async fn call_api(
        &self,
        _model: &str,
        _messages: &[Msg],
        _tools: Option<&[JsonValue]>,
        _tool_choice: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError> {
        let mut count = self.call_count.lock().unwrap();
        let idx = *count;
        *count += 1;

        let script = self.script.lock().unwrap();

        let mut resp = ChatResponse::default();

        if idx < script.len() {
            match &script[idx] {
                ScriptedResponse::Text(text) => {
                    let tb = TextBlock::new(text.clone());
                    resp.content.push(ContentBlock::Text(tb));
                }
                ScriptedResponse::ToolCall { id, name, input } => {
                    let tc = ToolCallBlock::new(id.clone(), name.clone(), input.clone());
                    resp.content.push(ContentBlock::ToolCall(tc));
                }
            }
        }
        // If beyond script, return empty response (signals end of conversation)

        resp.usage = Some(ChatUsage::default());
        Ok(ModelCallResult::Complete(resp))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// MockModel returns preset text.
    #[tokio::test]
    async fn test_mock_model_returns_preset_text() {
        let model = MockModel::new("mock", "Hello, World!");
        let result = model.call_api("mock", &[], None, None).await.unwrap();
        let text = if let ModelCallResult::Complete(resp) = result {
            resp.get_text_content("")
        } else {
            panic!("expected Complete");
        };
        assert_eq!(text, "Hello, World!");
    }

    /// ScriptedModel follows script sequence.
    #[tokio::test]
    async fn test_scripted_model_follows_script() {
        let script = vec![
            ScriptedResponse::ToolCall {
                id: "tc1".into(),
                name: "calc".into(),
                input: r#"{"a":1,"b":2}"#.into(),
            },
            ScriptedResponse::Text("The answer is 3".into()),
        ];
        let model = ScriptedModel::new("scripted", script);

        // First call: tool call
        let result = model.call_api("s", &[], None, None).await.unwrap();
        if let ModelCallResult::Complete(resp) = result {
            if let Some(ContentBlock::ToolCall(tc)) = resp.content.first() {
                assert_eq!(tc.name, "calc");
                assert_eq!(tc.input, r#"{"a":1,"b":2}"#);
            } else {
                panic!("expected ToolCall block");
            }
        }

        // Second call: text
        let result = model.call_api("s", &[], None, None).await.unwrap();
        if let ModelCallResult::Complete(resp) = result {
            assert_eq!(resp.get_text_content(""), "The answer is 3");
        }

        // Third call: empty
        let result = model.call_api("s", &[], None, None).await.unwrap();
        if let ModelCallResult::Complete(resp) = result {
            assert!(resp.content.is_empty());
        }

        assert_eq!(model.call_count(), 3);
    }
}
