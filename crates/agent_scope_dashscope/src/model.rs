//! DashScopeChatModel — ChatModel implementation for Alibaba Cloud DashScope (百炼).
//!
//! Uses the OpenAI-compatible endpoint at `/compatible-mode/v1/chat/completions`.

use std::collections::HashMap;
use std::pin::Pin;
use std::time::Instant;

use agent_scope_message::Msg;
use base64::Engine;
use futures::Stream;
use serde_json::Value as JsonValue;

use agent_scope_model::formatter::Formatter;
use agent_scope_model::model_error::{ModelError, ModelErrorKind};
use agent_scope_model::model_trait::{ChatModel, ModelCallResult};
use agent_scope_model::response::ChatResponse;
use agent_scope_model::schema_flat::flatten_json_schema_with_defs;
use agent_scope_model::tool_choice::ToolChoice;
use agent_scope_model::usage::ChatUsage;

use crate::formatter::DashScopeFormatter;
use crate::parameters::DashScopeParameters;

/// DashScope (Alibaba Cloud Model Studio) Chat Model provider.
///
/// Communicates with the dashscope.aliyuncs.com OpenAI-compatible endpoint.
pub struct DashScopeChatModel {
    /// Alibaba Cloud DashScope API key.
    pub api_key: String,
    /// Base URL for the OpenAI-compatible endpoint.
    pub base_url: String,
    /// Model name (e.g., "qwen-plus", "qwen-max").
    pub model_name: String,
    /// Generation parameters.
    pub parameters: DashScopeParameters,
    /// Whether streaming mode is enabled by default.
    pub stream: bool,
    /// Maximum number of retries for retryable errors.
    pub max_retries: u32,
    /// Delay between retries in seconds.
    pub retry_delay: f64,
    /// Context window size in tokens.
    pub context_size: i64,
    /// Message formatter.
    pub formatter: Box<dyn Formatter>,
    /// HTTP client.
    pub client: reqwest::Client,
    /// Extra body fields to merge into every request.
    pub extra_body: HashMap<String, JsonValue>,
}

impl std::fmt::Debug for DashScopeChatModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DashScopeChatModel")
            .field("model_name", &self.model_name)
            .field("base_url", &self.base_url)
            .field("stream", &self.stream)
            .finish_non_exhaustive()
    }
}

impl DashScopeChatModel {
    /// Create a new DashScopeChatModel with the given API key and model name.
    pub fn new(api_key: impl Into<String>, model_name: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            model_name: model_name.into(),
            parameters: DashScopeParameters::default(),
            stream: true,
            max_retries: 3,
            retry_delay: 1.0,
            context_size: 131072,
            formatter: Box::new(DashScopeFormatter::default()),
            client: reqwest::Client::new(),
            extra_body: HashMap::new(),
        }
    }

    /// Set a custom base URL (e.g., for regional endpoints or mock servers).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Set streaming mode.
    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    // ── Request body builder ──────────────────────────────────────────

    /// Build the JSON request body for the DashScope API.
    pub fn build_request_body(
        &self,
        messages: &[Msg],
        tools: Option<&[JsonValue]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<JsonValue, ModelError> {
        let formatted = self
            .formatter
            .format(messages)
            .map_err(|e| ModelError::FormatError {
                context: "dashscope".to_string(),
                source: e,
            })?;

        let mut body = serde_json::Map::new();
        body.insert(
            "model".to_string(),
            JsonValue::String(self.model_name.clone()),
        );
        body.insert("messages".to_string(), JsonValue::Array(formatted));
        body.insert("stream".to_string(), JsonValue::Bool(self.stream));

        // Standard generation parameters
        if let Some(max_tokens) = self.parameters.max_tokens {
            body.insert(
                "max_tokens".to_string(),
                JsonValue::Number(max_tokens.into()),
            );
        }
        if let Some(temp) = self.parameters.temperature {
            body.insert("temperature".to_string(), serde_json::json!(temp));
        }
        if let Some(top_p) = self.parameters.top_p {
            body.insert("top_p".to_string(), serde_json::json!(top_p));
        }
        if let Some(top_k) = self.parameters.top_k {
            body.insert("top_k".to_string(), serde_json::json!(top_k));
        }
        if let Some(seed) = self.parameters.seed {
            body.insert("seed".to_string(), serde_json::json!(seed));
        }
        if let Some(ref stop) = self.parameters.stop {
            body.insert("stop".to_string(), serde_json::json!(stop));
        }

        // DashScope-specific extensions
        body.insert(
            "enable_search".to_string(),
            JsonValue::Bool(self.parameters.enable_search),
        );

        if let Some(rp) = self.parameters.repetition_penalty {
            body.insert("repetition_penalty".to_string(), serde_json::json!(rp));
        }

        // Thinking / reasoning mode
        if self.parameters.enable_thinking {
            body.insert("enable_thinking".to_string(), JsonValue::Bool(true));
            if let Some(budget) = self.parameters.thinking_budget {
                body.insert("thinking_budget".to_string(), serde_json::json!(budget));
            }
        }

        // Tools
        if let Some(tools) = tools {
            let formatted_tools: Vec<JsonValue> = tools
                .iter()
                .map(|t| {
                    let mut tool = t.clone();
                    if let Some(func) = tool.get_mut("function")
                        && let Some(params) = func.get("parameters")
                    {
                        let flattened = flatten_json_schema_with_defs(params);
                        func["parameters"] = flattened;
                    }
                    tool
                })
                .collect();
            body.insert("tools".to_string(), JsonValue::Array(formatted_tools));
        }

        // Tool choice
        if let Some(tc) = tool_choice {
            match tc.mode.as_str() {
                "auto" | "none" => {
                    body.insert(
                        "tool_choice".to_string(),
                        JsonValue::String(tc.mode.clone()),
                    );
                }
                "required" => {
                    // DashScope thinking mode rejects tool_choice="required"
                    if self.parameters.enable_thinking {
                        return Err(ModelError::UnsupportedFeature {
                            feature: "tool_choice=\"required\" with enable_thinking=true"
                                .to_string(),
                            provider: "dashscope".to_string(),
                        });
                    }
                    body.insert(
                        "tool_choice".to_string(),
                        JsonValue::String("required".to_string()),
                    );
                }
                tool_name => {
                    body.insert(
                        "tool_choice".to_string(),
                        serde_json::json!({
                            "type": "function",
                            "function": { "name": tool_name }
                        }),
                    );
                }
            }
        }

        // Stream options with usage
        if self.stream {
            body.insert(
                "stream_options".to_string(),
                serde_json::json!({
                    "include_usage": true
                }),
            );
        }

        // Merge extra_body
        for (k, v) in &self.extra_body {
            body.insert(k.clone(), v.clone());
        }

        Ok(JsonValue::Object(body))
    }

    // ── Response parsing ──────────────────────────────────────────────

    /// Parse a non-streaming ChatCompletion JSON response.
    pub fn parse_completion_response(&self, json: &JsonValue) -> Result<ChatResponse, ModelError> {
        let mut resp = ChatResponse::default();

        if let Some(resp_id) = json.get("id").and_then(|v| v.as_str()) {
            resp.id = resp_id.to_string();
        }

        let choices = json.get("choices").and_then(|v| v.as_array());

        if let Some(choice) = choices.and_then(|c| c.first()) {
            let message = choice.get("message");

            // Text content
            if let Some(content) = message
                .and_then(|m| m.get("content"))
                .and_then(|v| v.as_str())
                && !content.is_empty()
            {
                resp.append_text(content, None);
            }

            // Reasoning/thinking content
            if let Some(reasoning) = message
                .and_then(|m| m.get("reasoning_content"))
                .and_then(|v| v.as_str())
            {
                resp.append_thinking(reasoning, None, HashMap::new());
            }

            // Tool calls
            if let Some(tool_calls) = message
                .and_then(|m| m.get("tool_calls"))
                .and_then(|v| v.as_array())
            {
                for tc in tool_calls {
                    let id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let func = tc.get("function");
                    let name = func
                        .and_then(|f| f.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let args = func
                        .and_then(|f| f.get("arguments"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("{}");
                    resp.append_tool_call(id, name, args, HashMap::new());
                }
            }

            // Audio
            if let Some(audio) = message.and_then(|m| m.get("audio")) {
                if let Some(data) = audio.get("data").and_then(|v| v.as_str()) {
                    let raw = base64::engine::general_purpose::STANDARD
                        .decode(data)
                        .unwrap_or_default();
                    resp.append_data_block("audio_1", &raw, "audio/pcm16", None);
                }
                if let Some(transcript) = audio.get("transcript").and_then(|v| v.as_str()) {
                    resp.append_text(transcript, None);
                }
            }
        }

        // Usage — DashScope uses `prompt_tokens`/`completion_tokens`/`total_tokens`
        if let Some(usage) = json.get("usage") {
            resp.usage = Some(ChatUsage {
                input_tokens: usage
                    .get("prompt_tokens")
                    .or_else(|| usage.get("input_tokens"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                output_tokens: usage
                    .get("completion_tokens")
                    .or_else(|| usage.get("output_tokens"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                time: 0.0,
                ..Default::default()
            });
        }

        resp.is_last = true;
        Ok(resp)
    }

    /// Parse streaming SSE bytes into a Stream of ChatResponse chunks.
    pub fn parse_stream_response(
        stream: reqwest::Response,
    ) -> Pin<Box<dyn Stream<Item = Result<ChatResponse, ModelError>> + Send>> {
        use futures::StreamExt;
        use futures::stream;

        let byte_stream = stream.bytes_stream();

        let s = byte_stream
            .map(|result| {
                result.map_err(|e| ModelError::ApiError {
                    status: 0,
                    message: e.to_string(),
                    provider: "dashscope".to_string(),
                })
            })
            .flat_map(move |result| {
                let bytes = match result {
                    Ok(b) => b,
                    Err(e) => return stream::iter(vec![Err(e)]),
                };
                let text = String::from_utf8_lossy(&bytes).to_string();

                // Unify line endings and trim
                let text = text.replace("\r\n", "\n");

                let chunks: Vec<Result<ChatResponse, ModelError>> = text
                    .lines()
                    .filter(|line| line.starts_with("data: "))
                    .filter_map(|line| {
                        let data = line.strip_prefix("data: ").unwrap();
                        if data.trim() == "[DONE]" {
                            return None; // End of stream signal
                        }
                        match serde_json::from_str::<JsonValue>(data) {
                            Ok(json) => {
                                let mut resp = ChatResponse::default();

                                if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                                    resp.id = id.to_string();
                                }

                                let choices = json.get("choices").and_then(|v| v.as_array());

                                if let Some(choices_arr) = choices {
                                    if choices_arr.is_empty() {
                                        // Empty choices: usage-only chunk
                                        if let Some(usage) = json.get("usage") {
                                            resp.usage = Some(ChatUsage {
                                                input_tokens: usage
                                                    .get("prompt_tokens")
                                                    .or_else(|| usage.get("input_tokens"))
                                                    .and_then(|v| v.as_i64())
                                                    .unwrap_or(0),
                                                output_tokens: usage
                                                    .get("completion_tokens")
                                                    .or_else(|| usage.get("output_tokens"))
                                                    .and_then(|v| v.as_i64())
                                                    .unwrap_or(0),
                                                time: 0.0,
                                                ..Default::default()
                                            });
                                        }
                                    } else if let Some(choice) = choices_arr.first() {
                                        let delta = choice.get("delta");

                                        // Text delta
                                        if let Some(content) = delta
                                            .and_then(|d| d.get("content"))
                                            .and_then(|v| v.as_str())
                                        {
                                            resp.append_text(content, Some("text_0"));
                                        }

                                        // Reasoning/thinking delta
                                        if let Some(reasoning) = delta
                                            .and_then(|d| d.get("reasoning_content"))
                                            .and_then(|v| v.as_str())
                                        {
                                            resp.append_thinking(
                                                reasoning,
                                                Some("thinking_0"),
                                                HashMap::new(),
                                            );
                                        }

                                        // Tool call delta
                                        if let Some(tool_calls) = delta
                                            .and_then(|d| d.get("tool_calls"))
                                            .and_then(|v| v.as_array())
                                        {
                                            for tc in tool_calls {
                                                let idx = tc
                                                    .get("index")
                                                    .and_then(|v| v.as_u64())
                                                    .unwrap_or(0);
                                                let tc_id = tc
                                                    .get("id")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let block_id = format!("tc_{idx}_{tc_id}");

                                                let func = tc.get("function");
                                                let name = func
                                                    .and_then(|f| f.get("name"))
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("");
                                                let args = func
                                                    .and_then(|f| f.get("arguments"))
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("");

                                                resp.append_tool_call(
                                                    &block_id,
                                                    name,
                                                    args,
                                                    HashMap::new(),
                                                );
                                            }
                                        }

                                        // Audio delta
                                        if let Some(audio) = delta.and_then(|d| d.get("audio")) {
                                            if let Some(data) =
                                                audio.get("data").and_then(|v| v.as_str())
                                            {
                                                let raw = base64::engine::general_purpose::STANDARD
                                                    .decode(data)
                                                    .unwrap_or_default();
                                                resp.append_data_block(
                                                    "audio_0",
                                                    &raw,
                                                    "audio/pcm16",
                                                    None,
                                                );
                                            }
                                            if let Some(transcript) =
                                                audio.get("transcript").and_then(|v| v.as_str())
                                            {
                                                resp.append_text(transcript, Some("transcript_0"));
                                            }
                                        }
                                    }
                                }

                                // Top-level usage in streaming chunk
                                if let Some(usage) = json.get("usage") {
                                    resp.usage = Some(ChatUsage {
                                        input_tokens: usage
                                            .get("prompt_tokens")
                                            .or_else(|| usage.get("input_tokens"))
                                            .and_then(|v| v.as_i64())
                                            .unwrap_or(0),
                                        output_tokens: usage
                                            .get("completion_tokens")
                                            .or_else(|| usage.get("output_tokens"))
                                            .and_then(|v| v.as_i64())
                                            .unwrap_or(0),
                                        time: 0.0,
                                        ..Default::default()
                                    });
                                }

                                // Skip empty carrier chunks
                                if resp.content.is_empty()
                                    && resp.usage.is_none()
                                    && resp.id.is_empty()
                                {
                                    return None;
                                }

                                Some(Ok(resp))
                            }
                            Err(e) => Some(Err(ModelError::SerializationError {
                                context: "dashscope SSE parse".to_string(),
                                source: e,
                            })),
                        }
                    })
                    .collect();
                stream::iter(chunks)
            });

        Box::pin(s)
    }

    /// Parse an error response body. Handles two formats:
    /// 1. OpenAI-compatible: `{"error": {"message": "...", "code": "...", "type": "..."}}`
    /// 2. DashScope flat: `{"code": "...", "message": "...", "request_id": "..."}`
    pub fn parse_error_response(body: &str) -> String {
        // Try OpenAI-compatible nested format first
        if let Ok(json) = serde_json::from_str::<JsonValue>(body) {
            if let Some(error) = json.get("error") {
                let msg = error
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error");
                let code = error.get("code").and_then(|v| v.as_str()).unwrap_or("");
                return if code.is_empty() {
                    msg.to_string()
                } else {
                    format!("[{code}] {msg}")
                };
            }
            // Try flat format
            let msg = json
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            let code = json.get("code").and_then(|v| v.as_str()).unwrap_or("");
            if code.is_empty() {
                msg.to_string()
            } else {
                format!("[{code}] {msg}")
            }
        } else {
            body.to_string()
        }
    }

    /// Format tools for the API request.
    pub fn format_tools(
        &self,
        tools: Option<&[JsonValue]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<(Option<Vec<JsonValue>>, Option<JsonValue>), ModelError> {
        let filtered: Option<Vec<JsonValue>> = if let Some(tools) = tools {
            if let Some(tc) = tool_choice {
                if let Some(ref tc_tools) = tc.tools {
                    let filtered: Vec<JsonValue> = tools
                        .iter()
                        .filter(|t| {
                            let name = t
                                .get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|n| n.as_str());
                            name.is_some_and(|n| tc_tools.contains(&n.to_string()))
                        })
                        .cloned()
                        .collect();
                    Some(filtered)
                } else {
                    Some(tools.to_vec())
                }
            } else {
                Some(tools.to_vec())
            }
        } else {
            None
        };

        let tc_json = tool_choice.map(|tc| match tc.mode.as_str() {
            "auto" | "none" | "required" => JsonValue::String(tc.mode.clone()),
            tool_name => serde_json::json!({
                "type": "function",
                "function": { "name": tool_name }
            }),
        });

        Ok((filtered, tc_json))
    }
}

#[async_trait::async_trait]
impl ChatModel for DashScopeChatModel {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn stream_enabled(&self) -> bool {
        self.stream
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }

    fn retry_delay(&self) -> f64 {
        self.retry_delay
    }

    fn context_size(&self) -> i64 {
        self.context_size
    }

    fn retryable_errors(&self) -> &[ModelErrorKind] {
        &[
            ModelErrorKind::ApiConnection,
            ModelErrorKind::ApiTimeout,
            ModelErrorKind::RateLimit,
            ModelErrorKind::InternalServer,
        ]
    }

    async fn call_api(
        &self,
        _model_name: &str,
        messages: &[Msg],
        tools: Option<&[JsonValue]>,
        tool_choice: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError> {
        let start = Instant::now();

        let body = self.build_request_body(messages, tools, tool_choice)?;

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let req = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body);

        let response = req.send().await.map_err(|e| ModelError::ApiError {
            status: 0,
            message: e.to_string(),
            provider: "dashscope".to_string(),
        })?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let err_text = response.text().await.unwrap_or_default();
            let parsed = Self::parse_error_response(&err_text);
            return Err(ModelError::ApiError {
                status,
                message: parsed,
                provider: "dashscope".to_string(),
            });
        }

        if self.stream {
            let stream = Self::parse_stream_response(response);
            Ok(ModelCallResult::Stream(stream))
        } else {
            let json: JsonValue = response.json().await.map_err(|e| ModelError::ApiError {
                status: 0,
                message: format!("JSON parse error: {e}"),
                provider: "dashscope".to_string(),
            })?;
            let mut resp = self.parse_completion_response(&json)?;
            if let Some(ref mut usage) = resp.usage {
                usage.time = start.elapsed().as_secs_f64();
            }
            Ok(ModelCallResult::Complete(resp))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_scope_message::factory::user_msg;

    fn make_model() -> DashScopeChatModel {
        DashScopeChatModel::new("sk-test-key", "qwen-plus")
    }

    #[test]
    fn test_build_request_body_basic() {
        let model = make_model();
        let msg = user_msg("user", "Hello").unwrap();
        let body = model.build_request_body(&[msg], None, None).unwrap();
        assert_eq!(body["model"], "qwen-plus");
        assert_eq!(body["stream"], true);
        assert_eq!(body["enable_search"], false);
    }

    #[test]
    fn test_build_request_body_with_tools() {
        let model = make_model();
        let msg = user_msg("user", "Hi").unwrap();
        let tools = vec![serde_json::json!({
            "type": "function",
            "function": {
                "name": "search",
                "description": "Search the web",
                "parameters": { "type": "object", "properties": {} }
            }
        })];
        let tool_choice = ToolChoice::auto();
        let body = model
            .build_request_body(&[msg], Some(&tools), Some(&tool_choice))
            .unwrap();
        assert!(body.get("tools").is_some());
        assert_eq!(body["tool_choice"], "auto");
    }

    #[test]
    fn test_tool_choice_required_with_thinking_rejected() {
        let mut model = make_model();
        model.parameters.enable_thinking = true;
        let msg = user_msg("user", "Hi").unwrap();
        let tools = vec![serde_json::json!({
            "type": "function",
            "function": { "name": "search", "parameters": {} }
        })];
        let tool_choice = ToolChoice::required();
        let result = model.build_request_body(&[msg], Some(&tools), Some(&tool_choice));
        assert!(matches!(result, Err(ModelError::UnsupportedFeature { .. })));
    }

    #[test]
    fn test_parse_completion_response() {
        let model = make_model();
        let json = serde_json::json!({
            "id": "chatcmpl-123",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help?"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        });
        let resp = model.parse_completion_response(&json).unwrap();
        assert!(resp.is_last);
        assert_eq!(resp.get_text_content(""), "Hello! How can I help?");
        assert_eq!(resp.id, "chatcmpl-123");
        assert!(resp.usage.is_some());
    }

    #[test]
    fn test_parse_error_nested_format() {
        let body = r#"{"error": {"message": "Invalid API-key provided.", "code": "InvalidApiKey", "type": "invalid_request_error"}}"#;
        let parsed = DashScopeChatModel::parse_error_response(body);
        assert!(parsed.contains("InvalidApiKey"));
        assert!(parsed.contains("Invalid API-key"));
    }

    #[test]
    fn test_parse_error_flat_format() {
        let body = r#"{"code": "InvalidApiKey", "message": "Invalid API-key provided.", "request_id": "xxx"}"#;
        let parsed = DashScopeChatModel::parse_error_response(body);
        assert!(parsed.contains("InvalidApiKey"));
        assert!(parsed.contains("Invalid API-key"));
    }

    #[test]
    fn test_format_tools_with_filter() {
        let model = make_model();
        let tools = vec![
            serde_json::json!({"type": "function", "function": {"name": "search"}}),
            serde_json::json!({"type": "function", "function": {"name": "calc"}}),
        ];
        let tc = ToolChoice::with_tools("auto", vec!["search".to_string()]);
        let (filtered, _) = model.format_tools(Some(&tools), Some(&tc)).unwrap();
        assert_eq!(filtered.unwrap().len(), 1);
    }
}
