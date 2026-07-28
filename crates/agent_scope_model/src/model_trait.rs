//! ChatModel trait — unified interface for all LLM providers.

use std::pin::Pin;

use agent_scope_message::Msg;
use futures::Stream;
use serde_json::Value as JsonValue;

use crate::card::ModelCard;
use crate::json_repair::json_repair;
use crate::model_error::{ModelError, ModelErrorKind};
use crate::response::{ChatResponse, StructuredResponse};
use crate::schema_flat::flatten_json_schema_with_defs;
use crate::tool_choice::ToolChoice;

/// Result type for `ChatModel::call()`.
#[allow(clippy::large_enum_variant)]
pub enum ModelCallResult {
    Complete(ChatResponse),
    Stream(Pin<Box<dyn Stream<Item = Result<ChatResponse, ModelError>> + Send>>),
}

/// Unified trait for all chat model providers.
///
/// # Retry logic
/// `call()` implements automatic retry: up to `max_retries()` additional attempts,
/// only for errors matching `retryable_errors()`, with `retry_delay()` seconds between.
///
/// # Streaming
/// When `stream_enabled()` is true, `call()` returns a `Stream<ChatResponse>`.
/// The stream should be consumed through a `StreamAccumulator` for O(n) accumulation.
///
/// # Token counting
/// Default implementation uses `bytes/4` heuristic. Provider implementations
/// can override with precise tokenizers.
///
/// # Structured output
/// Default implementation uses tool-calling bypass: inject a
/// `generate_structured_output` tool, force tool_choice, parse the result.
#[async_trait::async_trait]
pub trait ChatModel: Send + Sync {
    // ── Required methods ───────────────────────────────────────────────

    /// The model identifier string (e.g., "gpt-4.1").
    fn model_name(&self) -> &str;

    /// Whether streaming mode is enabled by default.
    fn stream_enabled(&self) -> bool;

    /// Provider-specific API call implementation.
    async fn call_api(
        &self,
        model_name: &str,
        messages: &[Msg],
        tools: Option<&[JsonValue]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError>;

    // ── Optional overrides ─────────────────────────────────────────────

    /// Maximum number of retry attempts (default: 3).
    fn max_retries(&self) -> u32 {
        3
    }

    /// Delay between retries in seconds (default: 1.0).
    fn retry_delay(&self) -> f64 {
        1.0
    }

    /// Error categories that trigger a retry.
    fn retryable_errors(&self) -> &[ModelErrorKind] {
        &[]
    }

    /// Context window size in tokens.
    fn context_size(&self) -> i64 {
        32768
    }

    // ── Default trait methods ──────────────────────────────────────────

    /// Entry point for model calls with retry and cancel logic.
    async fn call(
        &self,
        messages: &[Msg],
        tools: Option<&[JsonValue]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError> {
        let max_retries = self.max_retries();
        let retry_delay = self.retry_delay();
        let model_name = self.model_name().to_string();

        let mut last_error: Option<ModelError> = None;

        for attempt in 0..=max_retries {
            match self
                .call_api(&model_name, messages, tools, tool_choice)
                .await
            {
                Ok(result) => return Ok(result),
                Err(err) => {
                    let is_retryable = self
                        .retryable_errors()
                        .iter()
                        .any(|kind| err.kind().as_ref() == Some(kind));

                    if !is_retryable {
                        return Err(err);
                    }

                    last_error = Some(err);

                    if attempt < max_retries {
                        tokio::time::sleep(std::time::Duration::from_secs_f64(retry_delay)).await;
                    }
                }
            }
        }

        Err(ModelError::RetryExhausted {
            attempts: max_retries + 1,
            last_error: Box::new(last_error.unwrap()),
            provider: model_name,
        })
    }

    /// Count tokens in messages using byte/4 heuristic.
    ///
    /// Each DataBlock adds 2000 tokens. Provider implementations can override
    /// with precise tokenizers.
    fn count_tokens(&self, messages: &[Msg], tools: Option<&[JsonValue]>) -> usize {
        let mut total_bytes = 0usize;

        for msg in messages {
            for block in &msg.content {
                match block {
                    agent_scope_message::ContentBlock::Text(tb) => total_bytes += tb.text.len(),
                    agent_scope_message::ContentBlock::Thinking(tb) => {
                        total_bytes += tb.thinking.len();
                    }
                    agent_scope_message::ContentBlock::Hint(hb) => {
                        match &hb.hint {
                            agent_scope_message::HintContent::Text(t) => total_bytes += t.len(),
                            agent_scope_message::HintContent::Blocks(_) => {
                                total_bytes += 500; // rough estimate
                            }
                        }
                    }
                    agent_scope_message::ContentBlock::ToolCall(tc) => {
                        total_bytes += tc.input.len() + tc.name.len();
                    }
                    agent_scope_message::ContentBlock::ToolResult(tr) => match &tr.output {
                        agent_scope_message::ToolOutput::Text(t) => total_bytes += t.len(),
                        agent_scope_message::ToolOutput::Blocks(_) => total_bytes += 2000,
                    },
                    agent_scope_message::ContentBlock::Data(_) => {
                        total_bytes += 2000 * 4; // each DataBlock ≈ 2000 tokens
                    }
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

    /// Validate tool_choice against available tools.
    fn validate_tool_choice(
        &self,
        tool_choice: Option<&ToolChoice>,
        tools: Option<&[JsonValue]>,
    ) -> Result<(), ModelError> {
        let tool_names: Option<Vec<String>> = tools.map(|t| {
            t.iter()
                .filter_map(|v| {
                    v.get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                })
                .map(|s| s.to_string())
                .collect()
        });

        if let Some(tc) = tool_choice {
            tc.validate(tool_names.as_deref())
                .map_err(|msg| ModelError::ValidationError {
                    field: "tool_choice".to_string(),
                    message: msg,
                })?;
        }

        Ok(())
    }

    /// List available models from pre-parsed model card values.
    ///
    /// The calling Provider is responsible for reading YAML files, converting
    /// them to `serde_json::Value`, and passing them here.
    fn list_models(
        parsed_cards: &[(String, JsonValue)],
        base_parameter_schema: &JsonValue,
    ) -> Result<Vec<ModelCard>, ModelError> {
        let mut cards = Vec::new();

        for (_filename, card_value) in parsed_cards {
            match ModelCard::from_value(card_value, base_parameter_schema) {
                Ok(card) => cards.push(card),
                Err(e) => {
                    eprintln!("WARNING: Failed to load model card: {e}",);
                }
            }
        }

        Ok(cards)
    }

    /// Generate structured output via tool-calling bypass.
    async fn generate_structured_output(
        &self,
        messages: &[Msg],
        structured_model: &JsonValue,
    ) -> Result<StructuredResponse, ModelError> {
        if messages.is_empty() {
            return Err(ModelError::ValidationError {
                field: "messages".to_string(),
                message: "messages list must not be empty for structured output".to_string(),
            });
        }

        self.call_api_with_structured_output(messages, structured_model)
            .await
    }

    /// Default implementation: inject a `generate_structured_output` tool.
    async fn call_api_with_structured_output(
        &self,
        messages: &[Msg],
        structured_model: &JsonValue,
    ) -> Result<StructuredResponse, ModelError> {
        let json_schema = flatten_json_schema_with_defs(structured_model);

        let tool_schema = serde_json::json!({
            "type": "function",
            "function": {
                "name": "generate_structured_output",
                "description": "Generate structured output matching the required schema.",
                "parameters": json_schema
            }
        });

        let tools = vec![tool_schema];
        let tool_choice = ToolChoice::required();

        let result = self
            .call(messages, Some(&tools), Some(&tool_choice))
            .await?;

        match result {
            ModelCallResult::Complete(resp) => {
                let tool_input = resp
                    .content
                    .iter()
                    .find_map(|b| {
                        if let agent_scope_message::ContentBlock::ToolCall(tc) = b {
                            Some(tc.input.clone())
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| ModelError::StructuredOutputError {
                        reason: "No tool call found in response".to_string(),
                    })?;

                // Try parsing, fall back to JSON repair
                let parsed: JsonValue = serde_json::from_str(&tool_input)
                    .or_else(|_| {
                        let repaired = json_repair(&tool_input);
                        serde_json::from_str(&repaired)
                    })
                    .map_err(|e| ModelError::StructuredOutputError {
                        reason: format!("Failed to parse tool call input as JSON: {e}"),
                    })?;

                Ok(StructuredResponse {
                    content: parsed,
                    usage: resp.usage,
                    ..Default::default()
                })
            }
            ModelCallResult::Stream(_stream) => {
                // Structured output with streaming not supported by default.
                Err(ModelError::StructuredOutputError {
                    reason: "Streaming structured output not supported by default implementation"
                        .to_string(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_scope_message::factory::user_msg;

    struct TestModel {
        name: String,
        stream: bool,
    }

    #[async_trait::async_trait]
    impl ChatModel for TestModel {
        fn model_name(&self) -> &str {
            &self.name
        }
        fn stream_enabled(&self) -> bool {
            self.stream
        }

        async fn call_api(
            &self,
            _model: &str,
            _msgs: &[Msg],
            _tools: Option<&[JsonValue]>,
            _tc: Option<&ToolChoice>,
        ) -> Result<ModelCallResult, ModelError> {
            let mut resp = ChatResponse::default();
            resp.append_text("test response", None);
            Ok(ModelCallResult::Complete(resp))
        }
    }

    #[tokio::test]
    async fn test_count_tokens_byte_heuristic() {
        let model = TestModel {
            name: "test".into(),
            stream: false,
        };
        let msg = user_msg("user", "Hello, world!").unwrap(); // 13 bytes
        let tokens = model.count_tokens(&[msg], None);
        assert_eq!(tokens, 4); // 13/4 = 3.25, ceil = 4
    }

    #[tokio::test]
    async fn test_validate_tool_choice_valid() {
        let model = TestModel {
            name: "test".into(),
            stream: false,
        };
        let tc = ToolChoice::auto();
        let tools = vec![serde_json::json!({
            "type": "function",
            "function": { "name": "search" }
        })];
        assert!(model.validate_tool_choice(Some(&tc), Some(&tools)).is_ok());
    }

    #[tokio::test]
    async fn test_validate_tool_choice_specific_valid() {
        let model = TestModel {
            name: "test".into(),
            stream: false,
        };
        let tc = ToolChoice::specific_tool("search");
        let tools = vec![serde_json::json!({
            "type": "function",
            "function": { "name": "search" }
        })];
        assert!(model.validate_tool_choice(Some(&tc), Some(&tools)).is_ok());
    }

    #[tokio::test]
    async fn test_validate_tool_choice_specific_invalid() {
        let model = TestModel {
            name: "test".into(),
            stream: false,
        };
        let tc = ToolChoice::specific_tool("nonexistent");
        let tools = vec![serde_json::json!({
            "type": "function",
            "function": { "name": "search" }
        })];
        assert!(model.validate_tool_choice(Some(&tc), Some(&tools)).is_err());
    }

    #[tokio::test]
    async fn test_call_retry_exhausted() {
        struct FailingModel;
        #[async_trait::async_trait]
        impl ChatModel for FailingModel {
            fn model_name(&self) -> &str {
                "fail"
            }
            fn stream_enabled(&self) -> bool {
                false
            }
            fn retryable_errors(&self) -> &[ModelErrorKind] {
                &[ModelErrorKind::InternalServer]
            }
            async fn call_api(
                &self,
                _: &str,
                _: &[Msg],
                _: Option<&[JsonValue]>,
                _: Option<&ToolChoice>,
            ) -> Result<ModelCallResult, ModelError> {
                Err(ModelError::ApiError {
                    status: 500,
                    message: "fail".into(),
                    provider: "test".into(),
                })
            }
        }
        let result = FailingModel.call(&[], None, None).await;
        assert!(matches!(result, Err(ModelError::RetryExhausted { .. })));
    }

    #[tokio::test]
    async fn test_call_non_retryable_immediate() {
        struct BadModel;
        #[async_trait::async_trait]
        impl ChatModel for BadModel {
            fn model_name(&self) -> &str {
                "bad"
            }
            fn stream_enabled(&self) -> bool {
                false
            }
            async fn call_api(
                &self,
                _: &str,
                _: &[Msg],
                _: Option<&[JsonValue]>,
                _: Option<&ToolChoice>,
            ) -> Result<ModelCallResult, ModelError> {
                Err(ModelError::ValidationError {
                    field: "msg".into(),
                    message: "bad".into(),
                })
            }
        }
        let result = BadModel.call(&[], None, None).await;
        assert!(matches!(result, Err(ModelError::ValidationError { .. })));
    }

    // ── US3: Structured output tests ──────────────────────────────────

    struct StructuredModel {
        tool_input: String,
        should_fail: bool,
    }

    #[async_trait::async_trait]
    impl ChatModel for StructuredModel {
        fn model_name(&self) -> &str {
            "structured"
        }
        fn stream_enabled(&self) -> bool {
            false
        }

        async fn call_api(
            &self,
            _: &str,
            _: &[Msg],
            _: Option<&[JsonValue]>,
            _: Option<&ToolChoice>,
        ) -> Result<ModelCallResult, ModelError> {
            if self.should_fail {
                return Err(ModelError::StructuredOutputError {
                    reason: "mock schema validation failure".into(),
                });
            }
            let mut resp = ChatResponse::default();
            let tc = agent_scope_message::ToolCallBlock::new(
                "tc1".into(),
                "generate_structured_output".into(),
                self.tool_input.clone(),
            );
            resp.content
                .push(agent_scope_message::ContentBlock::ToolCall(tc));
            Ok(ModelCallResult::Complete(resp))
        }
    }

    /// T072: Mock model returns ToolCallBlock with JSON, verify StructuredResponse.
    #[tokio::test]
    async fn test_structured_output_success() {
        let model = StructuredModel {
            tool_input: r#"{"name": "test", "value": 42}"#.into(),
            should_fail: false,
        };
        let msg = user_msg("user", "Extract data").unwrap();
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "value": {"type": "number"}
            }
        });
        let result = model
            .generate_structured_output(&[msg], &schema)
            .await
            .unwrap();
        assert_eq!(result.response_type, "structured_response");
        assert_eq!(result.content["name"], "test");
        assert_eq!(result.content["value"], 42);
    }

    /// T073: Schema validation failure returns StructuredOutputError.
    #[tokio::test]
    async fn test_structured_output_schema_failure() {
        let model = StructuredModel {
            tool_input: "".into(),
            should_fail: true,
        };
        let msg = user_msg("user", "Extract").unwrap();
        let schema = serde_json::json!({"type": "object"});
        let result = model.generate_structured_output(&[msg], &schema).await;
        assert!(matches!(
            result,
            Err(ModelError::StructuredOutputError { .. })
        ));
    }

    /// T074: JSON repair scenario — missing closing brace → repair succeeds.
    #[tokio::test]
    async fn test_structured_output_json_repair() {
        let model = StructuredModel {
            tool_input: r#"{"name": "test", "value": 42"#.into(),
            should_fail: false,
        };
        let msg = user_msg("user", "Extract").unwrap();
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "value": {"type": "number"}
            }
        });
        let result = model
            .generate_structured_output(&[msg], &schema)
            .await
            .unwrap();
        assert_eq!(result.content["name"], "test");
    }

    /// T075: Empty messages list returns ValidationError.
    #[tokio::test]
    async fn test_structured_output_empty_messages() {
        let model = StructuredModel {
            tool_input: "{}".into(),
            should_fail: false,
        };
        let schema = serde_json::json!({"type": "object"});
        let result = model.generate_structured_output(&[], &schema).await;
        assert!(matches!(result, Err(ModelError::ValidationError { .. })));
    }
}
