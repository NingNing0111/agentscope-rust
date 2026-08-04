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
use agent_scope_model::response::{ChatResponse, FinishedReason};
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
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(600))
                .connect_timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build reqwest client"),
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
            // Apply the ToolChoice tool-subset filter (round-4 M18): the subset
            // restriction was implemented only in `format_tools`, which was
            // never called from the request path, so a caller that restricted
            // the model to a subset of tools silently received every tool.
            let (effective, _tc_json) = self.format_tools(Some(tools), tool_choice)?;
            let formatted_tools: Vec<JsonValue> = effective
                .unwrap_or_default()
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

            // Mirror the streaming path: a `length`/`content_filter`/`max_tokens`
            // termination means the output was cut short and must not be treated
            // as a complete answer (audit D7).
            if let Some(fr) = choice.get("finish_reason").and_then(|v| v.as_str())
                && matches!(fr, "length" | "content_filter" | "max_tokens")
            {
                resp.finished_reason = FinishedReason::Interrupted;
            }

            // Text content. Some multimodal models return `content` as an
            // array of `{type, text}` parts; previously only the string form
            // was accepted, so a non-streaming multimodal reply silently came
            // back empty (round-4 M24).
            if let Some(content) = message.and_then(|m| m.get("content")) {
                if let Some(text) = content.as_str()
                    && !text.is_empty()
                {
                    resp.append_text(text, None);
                } else if let Some(parts) = content.as_array() {
                    for part in parts {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str())
                            && !text.is_empty()
                        {
                            resp.append_text(text, None);
                        }
                    }
                }
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
    ///
    /// Bytes are accumulated across chunk boundaries so a `data:` event split
    /// by the transport (or a multi-byte UTF-8 character cut mid-sequence) is
    /// reconstructed before parsing. Each complete line is decoded strictly;
    /// partial trailing bytes stay buffered for the next chunk.
    pub fn parse_stream_response(
        stream: reqwest::Response,
    ) -> Pin<Box<dyn Stream<Item = Result<ChatResponse, ModelError>> + Send>> {
        use futures::StreamExt;
        use futures::stream;

        let byte_stream = stream.bytes_stream();

        // Accumulate across chunk boundaries (via `ingest_sse_bytes`) and, when
        // the source stream ends, flush any trailing bytes that were not
        // terminated by a newline. A transport that closes cleanly right after
        // the last `data:` line must not lose that final event (audit D6). An
        // `unfold` gives us the terminator hook that `scan` lacks.
        let byte_stream = byte_stream.map(|result| {
            result.map_err(|e| ModelError::ApiError {
                status: 0,
                message: e.to_string(),
                provider: "dashscope".to_string(),
            })
        });

        let s = futures::stream::unfold(
            (byte_stream, Vec::<u8>::new()),
            |(mut byte_stream, mut buf)| async move {
                loop {
                    match byte_stream.next().await {
                        Some(Ok(bytes)) => {
                            let out = Self::ingest_sse_bytes(&mut buf, &bytes);
                            if !out.is_empty() {
                                return Some((stream::iter(out), (byte_stream, buf)));
                            }
                            // Continue draining; a chunk may contain only a
                            // partial line that completes in the next chunk.
                        }
                        Some(Err(e)) => {
                            return Some((stream::iter(vec![Err(e)]), (byte_stream, buf)));
                        }
                        None => {
                            // Source ended. Flush the buffered tail.
                            let tail = std::mem::take(&mut buf);
                            if tail.is_empty() {
                                return None;
                            }
                            let line = String::from_utf8_lossy(&tail).to_string();
                            let out = if line.trim().is_empty() {
                                Vec::new()
                            } else {
                                match Self::parse_sse_line(&line) {
                                    Some(item) => vec![item],
                                    None => Vec::new(),
                                }
                            };
                            return Some((stream::iter(out), (byte_stream, buf)));
                        }
                    }
                }
            },
        )
        .flatten();

        Box::pin(s)
    }

    /// Append raw SSE bytes to the cross-chunk buffer and emit one result per
    /// complete line. Partial trailing bytes (a line split across chunks, or a
    /// multi-byte UTF-8 character cut mid-sequence) stay in `buf` for the next
    /// call. Kept separate so the chunk-boundary behaviour is unit-testable.
    fn ingest_sse_bytes(
        buf: &mut Vec<u8>,
        bytes: &[u8],
    ) -> Vec<Result<ChatResponse, ModelError>> {
        let mut out: Vec<Result<ChatResponse, ModelError>> = Vec::new();
        buf.extend_from_slice(bytes);
        // Consume every complete '\n'-terminated line; the tail stays buffered.
        let mut consumed = 0usize;
        for (i, &b) in buf.iter().enumerate() {
            if b != b'\n' {
                continue;
            }
            let line = &buf[consumed..i];
            consumed = i + 1;
            // Strip a single trailing '\r' for CRLF endings.
            let line = if line.last() == Some(&b'\r') {
                &line[..line.len() - 1]
            } else {
                line
            };
            match std::str::from_utf8(line) {
                Ok(text) => {
                    if let Some(item) = Self::parse_sse_line(text) {
                        out.push(item);
                    }
                }
                Err(err) => {
                    out.push(Err(ModelError::SerializationError {
                        context: "dashscope SSE line is not valid UTF-8".to_string(),
                        source: serde_json::Error::io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            err,
                        )),
                    }));
                }
            }
        }
        if consumed > 0 {
            buf.drain(..consumed);
        }
        out
    }

    /// Parse a single SSE `data:` line into a `ChatResponse`.
    ///
    /// Returns `None` for non-`data` lines and the terminal `[DONE]` sentinel;
    /// `Some(Ok(..))` for a parsed event; `Some(Err(..))` for a malformed
    /// payload. Carrier chunks that carry no content, usage or non-default
    /// finish reason (heartbeats, `{"choices":[]}` without usage) are dropped
    /// rather than emitted with a fabricated id.
    fn parse_sse_line(line: &str) -> Option<Result<ChatResponse, ModelError>> {
        // Accept both `data: {...}` and `data:{...}` (SSE spec allows no space).
        let data = line.strip_prefix("data: ").or_else(|| line.strip_prefix("data:"))?;
        if data.trim() == "[DONE]" {
            return None; // End of stream signal
        }
        if data.trim().is_empty() {
            // Empty `data:` line — a valid SSE heartbeat. Skip it rather than
            // failing the whole stream on a JSON parse of "" (audit D3).
            return None;
        }
        match serde_json::from_str::<JsonValue>(data) {
            Ok(json) => {
                // A stream-level error event (OpenAI-compatible: `data:
                // {"error":{"message":...}}`) must propagate, not be silently
                // dropped as a carrier (audit D4).
                if let Some(err) = json.get("error") {
                    let message = err
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("SSE stream error");
                    return Some(Err(ModelError::ApiError {
                        status: 0,
                        message: message.to_string(),
                        provider: "dashscope".to_string(),
                    }));
                }

                let mut resp = ChatResponse::default();

                if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                    resp.id = id.to_string();
                } else {
                    // `ChatResponse::default()` fabricates a random uuid id. If
                    // this chunk carries no id (e.g. a usage-only tail chunk),
                    // that fabricated id would overwrite the real response id in
                    // the StreamAccumulator (audit D5). Clear it so the
                    // accumulator keeps the id from the content chunks.
                    resp.id.clear();
                }

                let choices = json.get("choices").and_then(|v| v.as_array());

                if let Some(choices_arr) = choices {
                    if choices_arr.is_empty() {
                        // Empty choices: usage-only chunk. Only a real object
                        // counts — `"usage": null` must not fabricate a zero
                        // usage record or defeat the empty-carrier drop below
                        // (round-4 M17).
                        if let Some(usage) = json.get("usage").and_then(|v| v.as_object()) {
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
                        // Map the provider finish reason. A `length` /
                        // `content_filter` termination means the model output
                        // was cut short and must not be treated as a complete
                        // answer (the StreamAccumulator propagates this).
                        if let Some(fr) = choice.get("finish_reason").and_then(|v| v.as_str())
                            && matches!(fr, "length" | "content_filter" | "max_tokens")
                        {
                            resp.finished_reason = FinishedReason::Interrupted;
                        }

                        let delta = choice.get("delta");

                        // Text delta — skip empty text to avoid triggering
                        // empty TextBlock creation which causes
                        // premature ToolCallEnd in streaming (P1-12 fix)
                        if let Some(content) = delta
                            .and_then(|d| d.get("content"))
                            .and_then(|v| v.as_str())
                            && !content.is_empty()
                        {
                            resp.append_text(content, Some("text_0"));
                        }

                        // Reasoning/thinking delta — also guard against an empty
                        // string which would otherwise create an empty block.
                        if let Some(reasoning) = delta
                            .and_then(|d| d.get("reasoning_content"))
                            .and_then(|v| v.as_str())
                            && !reasoning.is_empty()
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
                                // Use index-only block_id for stable streaming
                                // accumulation. The tool_call `id` field may
                                // arrive in a later SSE chunk, which would
                                // cause StreamAccumulator to treat them as
                                // separate tool calls if we included it here.
                                let block_id = format!("tc_{idx}");

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

                // Top-level usage in streaming chunk. `"usage": null` (the
                // common OpenAI-compatible carrier form) must not be treated as
                // a real usage record — it would fabricate 0/0 tokens and
                // prevent the empty-carrier drop (round-4 M17).
                if let Some(usage) = json.get("usage").and_then(|v| v.as_object()) {
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

                // Drop empty carrier chunks. The previous `resp.id.is_empty()`
                // check was dead code because `ChatResponse::default()` always
                // assigns a random id, so every carrier was emitted — and the
                // fabricated id could overwrite the real one in the accumulator.
                if resp.content.is_empty()
                    && resp.usage.is_none()
                    && resp.finished_reason == FinishedReason::Completed
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

    // -----------------------------------------------------------------------
    // SSE cross-chunk buffering (audit D1/D2): a `data:` event split across TCP
    // chunks — including inside a multi-byte UTF-8 character — must be
    // reassembled, not dropped or corrupted.
    // -----------------------------------------------------------------------

    #[test]
    fn sse_split_inside_multibyte_char_is_reassembled() {
        // "你" is 3 bytes in UTF-8. Split the raw line mid-character so the
        // first chunk ends with a partial byte sequence and the newline only
        // arrives in the second chunk.
        let event = "data: {\"choices\":[{\"delta\":{\"content\":\"你好\"}}]}\n";
        let bytes = event.as_bytes();
        let cut = event.find("你").unwrap() + 1; // byte index inside "你"
        let (first, second) = bytes.split_at(cut);

        let mut buf = Vec::new();
        let out1 = DashScopeChatModel::ingest_sse_bytes(&mut buf, first);
        assert!(out1.is_empty(), "partial line must stay buffered, not be dropped");
        assert_eq!(buf.len(), cut, "partial bytes must be retained for next chunk");

        let out2 = DashScopeChatModel::ingest_sse_bytes(&mut buf, second);
        assert_eq!(out2.len(), 1, "completed line must yield exactly one event");
        let resp = out2[0].as_ref().expect("parse must succeed");
        assert_eq!(resp.get_text_content(""), "你好");
        assert!(buf.is_empty(), "buffer must be drained after the newline");
    }

    #[test]
    fn sse_multiple_events_in_one_chunk() {
        let payload = "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\
                       data: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n";
        let mut buf = Vec::new();
        let out = DashScopeChatModel::ingest_sse_bytes(&mut buf, payload.as_bytes());
        assert_eq!(out.len(), 2);
        let mut text = String::new();
        for item in &out {
            let resp = item.as_ref().unwrap();
            text.push_str(&resp.get_text_content(""));
        }
        assert_eq!(text, "ab");
        assert!(buf.is_empty());
    }

    #[test]
    fn sse_crlf_and_done_sentinel() {
        let mut buf = Vec::new();
        let payload = "data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\r\n\
                       data: [DONE]\r\n";
        let out = DashScopeChatModel::ingest_sse_bytes(&mut buf, payload.as_bytes());
        assert_eq!(out.len(), 1, "[DONE] is a sentinel, not an event");
        let resp = out[0].as_ref().unwrap();
        assert_eq!(resp.get_text_content(""), "x");
    }

    #[test]
    fn sse_empty_carrier_chunk_is_dropped() {
        // `{"choices":[]}` without usage carries nothing; it must not be
        // emitted with a fabricated random id (the old `id.is_empty()` check was
        // dead code because ChatResponse::default() always assigns a uuid).
        let mut buf = Vec::new();
        let out = DashScopeChatModel::ingest_sse_bytes(
            &mut buf,
            b"data: {\"choices\":[]}\ndata: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n",
        );
        assert_eq!(out.len(), 1);
        let resp = out[0].as_ref().unwrap();
        assert_eq!(resp.get_text_content(""), "hi");
    }

    #[test]
    fn sse_finish_reason_length_maps_to_interrupted() {
        let mut buf = Vec::new();
        let out = DashScopeChatModel::ingest_sse_bytes(
            &mut buf,
            b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"},\"finish_reason\":\"length\"}]}\n",
        );
        assert_eq!(out.len(), 1);
        let resp = out[0].as_ref().unwrap();
        assert_eq!(
            resp.finished_reason,
            FinishedReason::Interrupted,
            "a length-truncated stream must not be treated as completed"
        );
    }

    #[test]
    fn sse_usage_null_is_not_treated_as_real_usage() {
        // `"usage": null` (the common OpenAI-compatible carrier form) must not
        // fabricate a 0/0 usage record: that would defeat the empty-carrier
        // drop below and report 0 tokens for a real generation (round-4 M17).
        let mut buf = Vec::new();
        let out = DashScopeChatModel::ingest_sse_bytes(
            &mut buf,
            b"data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":null}\n",
        );
        assert_eq!(out.len(), 0, "usage-null carrier must be dropped");
    }

    #[test]
    fn sse_real_usage_object_is_preserved() {
        // A real usage object still lands on the response.
        let mut buf = Vec::new();
        let out = DashScopeChatModel::ingest_sse_bytes(
            &mut buf,
            b"data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5}}\n",
        );
        assert_eq!(out.len(), 1, "real usage-only chunk must be emitted");
        let resp = out[0].as_ref().unwrap();
        let usage = resp.usage.as_ref().expect("usage present");
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 5);
    }
}
