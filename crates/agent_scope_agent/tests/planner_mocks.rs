//! Planner test utilities.

#![allow(dead_code)]

use std::sync::Mutex;

use agent_scope_message::{ContentBlock, Msg, TextBlock};
use agent_scope_model::{ChatModel, ChatResponse, ModelCallResult, ModelError, ToolChoice};
use serde_json::Value as JsonValue;

/// Deterministic model that returns a fixed sequence of text responses.
pub struct PlannerScriptedModel {
    name: String,
    responses: Mutex<Vec<String>>,
    calls: Mutex<usize>,
}

impl PlannerScriptedModel {
    pub fn new(name: impl Into<String>, responses: Vec<String>) -> Self {
        Self {
            name: name.into(),
            responses: Mutex::new(responses),
            calls: Mutex::new(0),
        }
    }

    pub fn call_count(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

#[async_trait::async_trait]
impl ChatModel for PlannerScriptedModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn stream_enabled(&self) -> bool {
        false
    }

    async fn call_api(
        &self,
        _model_name: &str,
        _messages: &[Msg],
        _tools: Option<&[JsonValue]>,
        _tool_choice: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError> {
        let mut calls = self.calls.lock().unwrap();
        let idx = *calls;
        *calls += 1;
        let responses = self.responses.lock().unwrap();
        let text = responses.get(idx).cloned().unwrap_or_default();
        let mut resp = ChatResponse::default();
        resp.content.push(ContentBlock::Text(TextBlock::new(text)));
        Ok(ModelCallResult::Complete(resp))
    }
}
