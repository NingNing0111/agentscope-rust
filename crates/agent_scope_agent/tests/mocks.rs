//! Test utilities: MockModel and ScriptedModel.
//!
//! These provide deterministic model responses for testing the agent system
//! without requiring live LLM API calls (per Constitution Article 6).
//!
//! NOTE: This file is compiled both as a standalone integration test binary
//! and via `mod mocks` from streaming_tests.rs. We allow dead_code/unused_imports
//! to avoid warnings in the standalone mode.

#![allow(dead_code)]
#![allow(unused_imports)]

use std::pin::Pin;
use std::sync::{Arc, Mutex};

use agent_scope_message::{
    ContentBlock, Msg, TextBlock, ToolCallBlock, ToolOutput, ToolResultBlock, ToolResultState,
};
use agent_scope_model::{
    ChatModel, ChatResponse, ChatUsage, ModelCallResult, ModelError, ToolChoice,
};
use agent_scope_tool::{Tool, ToolError, ToolExecOutput};
use futures::{Stream, stream};
use serde_json::Value as JsonValue;
use tokio::sync::Notify;

// ---------------------------------------------------------------------------
// CallGate
// ---------------------------------------------------------------------------

/// A gate that lets a test hold a model call in-flight until released.
///
/// When attached to a [`MockModel`], `call_api` notifies `started` on entry and
/// then awaits `release` before returning. This enables deterministic
/// mid-reply interruption tests.
#[derive(Clone, Default)]
pub struct CallGate {
    /// Notified once `call_api` enters the gate.
    pub started: Arc<Notify>,
    /// `call_api` awaits this before producing a response.
    pub release: Arc<Notify>,
}

impl CallGate {
    /// Create a fresh, closed gate.
    pub fn new() -> Self {
        Self {
            started: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
        }
    }

    /// Signal that `call_api` has entered the gate.
    ///
    /// Uses `notify_one` (which stores a permit) rather than `notify_waiters`
    /// so the test's `started.notified().await` never misses the signal even
    /// when the model reaches the gate before the test polls the future.
    pub fn signal_started(&self) {
        self.started.notify_one();
    }

    /// Release the blocked model call.
    ///
    /// Uses `notify_one` so a later call also passes immediately.
    pub fn release(&self) {
        self.release.notify_one();
    }
}

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
    /// When true, `call_api` returns `ModelCallResult::Stream` instead of `Complete`.
    pub stream_mode: bool,
    /// Number of chunks to split the response into when stream_mode is true.
    pub stream_chunks: usize,
    /// Fixed context window size (overrides the trait default 32768).
    pub context_size: i64,
    /// Optional fixed token count returned by `count_tokens` (overrides the
    /// byte/4 heuristic), enabling deterministic context-length injection tests.
    pub fixed_tokens: Option<usize>,
    /// When set, `call_api` blocks until the gate is released.
    pub gate: Option<CallGate>,
}

impl MockModel {
    /// Create a new MockModel that returns `response_text` in a TextBlock.
    pub fn new(name: impl Into<String>, response_text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            response_text: response_text.into(),
            block_id: uuid::Uuid::new_v4().as_simple().to_string(),
            stream_mode: false,
            stream_chunks: 1,
            context_size: 32768,
            fixed_tokens: None,
            gate: None,
        }
    }

    /// Enable streaming mode with the given number of chunks.
    /// All chunks share the same block_id for StreamAccumulator merging.
    #[allow(dead_code)]
    pub fn with_stream(mut self, chunks: usize) -> Self {
        self.stream_mode = true;
        self.stream_chunks = chunks.max(1);
        self
    }

    /// Set the fixed context window size.
    #[allow(dead_code)]
    pub fn with_context_size(mut self, size: i64) -> Self {
        self.context_size = size;
        self
    }

    /// Set a fixed token count returned by `count_tokens`.
    #[allow(dead_code)]
    pub fn with_fixed_tokens(mut self, tokens: usize) -> Self {
        self.fixed_tokens = Some(tokens);
        self
    }

    /// Attach a gate that holds `call_api` in-flight until released.
    #[allow(dead_code)]
    pub fn with_gate(mut self, gate: CallGate) -> Self {
        self.gate = Some(gate);
        self
    }
}

#[async_trait::async_trait]
impl ChatModel for MockModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn stream_enabled(&self) -> bool {
        self.stream_mode
    }

    fn context_size(&self) -> i64 {
        self.context_size
    }

    fn count_tokens(&self, messages: &[Msg], tools: Option<&[JsonValue]>) -> usize {
        if let Some(tokens) = self.fixed_tokens {
            return tokens;
        }
        // Inline the trait default's byte/4 heuristic. Calling
        // `ChatModel::count_tokens(self, ...)` here would dispatch back to
        // `MockModel::count_tokens` and infinitely recurse.
        let mut total_bytes = 0usize;
        for msg in messages {
            for block in &msg.content {
                match block {
                    agent_scope_message::ContentBlock::Text(tb) => total_bytes += tb.text.len(),
                    agent_scope_message::ContentBlock::Thinking(tb) => {
                        total_bytes += tb.thinking.len();
                    }
                    agent_scope_message::ContentBlock::Hint(hb) => match &hb.hint {
                        agent_scope_message::HintContent::Text(t) => total_bytes += t.len(),
                        agent_scope_message::HintContent::Blocks(_) => total_bytes += 500,
                    },
                    agent_scope_message::ContentBlock::ToolCall(tc) => {
                        total_bytes += tc.input.len() + tc.name.len();
                    }
                    agent_scope_message::ContentBlock::ToolResult(tr) => match &tr.output {
                        agent_scope_message::ToolOutput::Text(t) => total_bytes += t.len(),
                        agent_scope_message::ToolOutput::Blocks(_) => total_bytes += 2000,
                    },
                    agent_scope_message::ContentBlock::Data(_) => total_bytes += 2000 * 4,
                    agent_scope_message::ContentBlock::Unknown => {}
                }
            }
        }
        if let Some(tools) = tools
            && let Ok(json_str) = serde_json::to_string(tools)
        {
            total_bytes += json_str.len();
        }
        (total_bytes as f64 / 4.0).ceil() as usize
    }

    async fn call_api(
        &self,
        _model: &str,
        _messages: &[Msg],
        _tools: Option<&[JsonValue]>,
        _tool_choice: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError> {
        if let Some(gate) = &self.gate {
            gate.started.notify_one();
            gate.release.notified().await;
        }
        if self.stream_mode {
            let text = self.response_text.clone();
            let total_chars = text.len();
            let n_chunks = self.stream_chunks;
            let chunk_size = (total_chars as f64 / n_chunks as f64).ceil() as usize;
            // Shared block_id so StreamAccumulator merges all chunks into one TextBlock
            let shared_block_id = uuid::Uuid::new_v4().as_simple().to_string();
            let resp_id = uuid::Uuid::new_v4().as_simple().to_string();

            let chunks: Vec<ChatResponse> = text
                .chars()
                .collect::<Vec<_>>()
                .chunks(chunk_size)
                .enumerate()
                .map(|(i, chars)| {
                    let mut resp = ChatResponse::default();
                    let mut tb = TextBlock::new(chars.iter().collect::<String>());
                    tb.id = shared_block_id.clone();
                    resp.content.push(ContentBlock::Text(tb));
                    resp.id = resp_id.clone();
                    // Usage on last chunk only (mimics DashScope behavior)
                    if i == n_chunks - 1 {
                        resp.usage = Some(ChatUsage::default());
                    }
                    resp
                })
                .collect();

            let stream: Pin<Box<dyn Stream<Item = Result<ChatResponse, ModelError>> + Send>> =
                Box::pin(stream::iter(chunks.into_iter().map(Ok)));
            Ok(ModelCallResult::Stream(stream))
        } else {
            let mut resp = ChatResponse::default();
            let tb = TextBlock::new(self.response_text.clone());
            resp.content.push(ContentBlock::Text(tb));
            resp.usage = Some(ChatUsage::default());
            Ok(ModelCallResult::Complete(resp))
        }
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

/// A mock ChatModel that streams a pre-built sequence of ChatResponse chunks.
///
/// Each chunk can contain mixed content blocks (ToolCall + Text interleaved),
/// enabling US2 tool call detection tests.
///
/// On the first call, streams the configured chunks. On subsequent calls,
/// returns an empty Complete response (allowing ReAct loop termination).
pub struct MockStreamingModel {
    name: String,
    chunks: Vec<ChatResponse>,
    call_count: Mutex<usize>,
}

impl MockStreamingModel {
    /// Create a new MockStreamingModel with the given chunks.
    /// First call streams chunks; subsequent calls return empty Complete.
    pub fn new(name: impl Into<String>, chunks: Vec<ChatResponse>) -> Self {
        Self {
            name: name.into(),
            chunks,
            call_count: Mutex::new(0),
        }
    }
}

#[async_trait::async_trait]
impl ChatModel for MockStreamingModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn stream_enabled(&self) -> bool {
        // Only stream on first call; subsequent calls are Complete
        *self.call_count.lock().unwrap() == 0
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

        if idx == 0 {
            // First call: stream chunks
            let resp_id = uuid::Uuid::new_v4().as_simple().to_string();
            let chunks: Vec<_> = self
                .chunks
                .iter()
                .enumerate()
                .map(|(i, chunk)| {
                    let mut c = chunk.clone();
                    c.id = resp_id.clone();
                    if i == self.chunks.len() - 1 {
                        c.usage = Some(ChatUsage::default());
                    }
                    c
                })
                .collect();
            let stream: Pin<Box<dyn Stream<Item = Result<ChatResponse, ModelError>> + Send>> =
                Box::pin(stream::iter(chunks.into_iter().map(Ok)));
            Ok(ModelCallResult::Stream(stream))
        } else {
            // Subsequent calls: empty Complete response (stops loop)
            let resp = ChatResponse {
                usage: Some(ChatUsage::default()),
                ..Default::default()
            };
            Ok(ModelCallResult::Complete(resp))
        }
    }
}

// ---------------------------------------------------------------------------
// MockStreamingTool
// ---------------------------------------------------------------------------

/// A mock Tool that returns `ToolExecOutput::Stream` with configurable chunks.
///
/// Used in US3 tests for progressive tool output delivery.
#[allow(dead_code)] // Used via mod mocks in streaming_tests.rs
pub struct MockStreamingTool {
    name: String,
    chunks: Vec<Result<ToolResultBlock, ToolError>>,
}

impl MockStreamingTool {
    pub fn new(name: impl Into<String>, chunks: Vec<Result<ToolResultBlock, ToolError>>) -> Self {
        Self {
            name: name.into(),
            chunks,
        }
    }
}

#[async_trait::async_trait]
impl Tool for MockStreamingTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "A mock streaming tool"
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": {"type": "string"}
            },
            "required": ["text"]
        })
    }

    async fn call(&self, _input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        let chunks: Vec<Result<ToolResultBlock, ToolError>> = self
            .chunks
            .iter()
            .map(|r| {
                r.clone().map_err(|e| ToolError::Execution {
                    tool_name: self.name.clone(),
                    reason: e.to_string(),
                })
            })
            .collect();
        let stream: Pin<Box<dyn Stream<Item = Result<ToolResultBlock, ToolError>> + Send>> =
            Box::pin(stream::iter(chunks));
        Ok(ToolExecOutput::Stream(stream))
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
