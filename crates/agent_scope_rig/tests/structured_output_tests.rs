//! T014 — 结构化输出测试（`generate_structured_output`）。
//!
//! 对照 `specs/034-rig-llm-integration/contracts/rig-mapping.md` §6：
//! 1. 原生 `output_schema` 路径：请求填 `output_schema`，响应 Text→JSON。
//! 2. 原生被拒（400/422 BadRequest）→ 回退工具 bypass（`generate_structured_output`
//!    工具 + `tool_choice=required`），从 ToolCall arguments 提取。
//! 3. 原生成功但无可解析 JSON → 回退工具 bypass。
//! 4. JSON repair 兜底（trailing comma / 不完整 JSON）。
//! 5. 空 messages → `ValidationError`。

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use futures::Stream;
use rig::completion::message::{AssistantContent, Text, ToolCall, ToolFunction};
use rig::completion::{CompletionRequest, Usage};

use agent_scope_message::factory::user_msg;
use agent_scope_model::model_error::ModelError;
use agent_scope_model::model_trait::ChatModel;
use agent_scope_model::response::StructuredResponse;
use agent_scope_rig::RigChatModel;
use agent_scope_rig::backend::{
    NormCompletion, RigChatBackend, RigProviderCapabilities, RigProviderKind, RigStreamDelta,
};

/// 可编程 mock backend：记录收到的请求，按队列弹出预置响应。
struct MockBackend {
    capabilities: RigProviderCapabilities,
    requests: Arc<Mutex<Vec<CompletionRequest>>>,
    responses: Arc<Mutex<VecDeque<Result<NormCompletion, ModelError>>>>,
}

impl MockBackend {
    fn new(
        responses: Vec<Result<NormCompletion, ModelError>>,
    ) -> (Self, Arc<Mutex<Vec<CompletionRequest>>>) {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let backend = Self {
            capabilities: RigProviderCapabilities {
                supports_thinking: false,
                thinking_tool_choice_incompatible: false,
                supports_embedding: false,
            },
            requests: requests.clone(),
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
        };
        (backend, requests)
    }
}

#[async_trait::async_trait]
impl RigChatBackend for MockBackend {
    fn capabilities(&self) -> &RigProviderCapabilities {
        &self.capabilities
    }

    async fn completion(&self, request: CompletionRequest) -> Result<NormCompletion, ModelError> {
        self.requests.lock().unwrap().push(request);
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| {
                Err(ModelError::StructuredOutputError {
                    reason: "no mock response programmed".to_string(),
                })
            })
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<RigStreamDelta, ModelError>> + Send>>, ModelError>
    {
        Err(ModelError::UnsupportedFeature {
            feature: "stream".to_string(),
            provider: "mock".to_string(),
        })
    }
}

// ── 构造工具 ────────────────────────────────────────────────────────────

fn usage() -> Usage {
    Usage {
        input_tokens: 10,
        output_tokens: 20,
        total_tokens: 30,
        ..Default::default()
    }
}

fn ok_text(text: &str, message_id: Option<&str>) -> Result<NormCompletion, ModelError> {
    Ok(NormCompletion {
        choice: vec![AssistantContent::Text(Text::new(text))],
        usage: usage(),
        message_id: message_id.map(str::to_string),
    })
}

fn ok_tool_call(args: serde_json::Value) -> Result<NormCompletion, ModelError> {
    Ok(NormCompletion {
        choice: vec![AssistantContent::ToolCall(ToolCall::new(
            "call_1".to_string(),
            ToolFunction::new("generate_structured_output".to_string(), args),
        ))],
        usage: usage(),
        message_id: None,
    })
}

fn bad_request() -> ModelError {
    ModelError::ApiError {
        status: 400,
        message: "output_schema not supported by this model".to_string(),
        provider: "mock".to_string(),
    }
}

fn run(
    model: &RigChatModel,
    messages: &[agent_scope_message::Msg],
    schema: &serde_json::Value,
) -> Result<StructuredResponse, ModelError> {
    futures::executor::block_on(model.generate_structured_output(messages, schema))
}

/// user_msg 快捷构造（签名 `(name, text) -> Result`）。
fn um(text: &str) -> agent_scope_message::Msg {
    user_msg("user", text).expect("valid user msg")
}

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "age": { "type": "integer" }
        },
        "required": ["name"]
    })
}

// ── 测试 ────────────────────────────────────────────────────────────────

#[test]
fn native_output_schema_path_returns_parsed_json() {
    let (backend, requests) = MockBackend::new(vec![ok_text(
        r#"{"name": "Alice", "age": 30}"#,
        Some("msg_1"),
    )]);
    let model = RigChatModel::from_backend_for_testing(
        RigProviderKind::OpenAi,
        "gpt-4o-mini",
        Arc::new(backend),
    );

    let resp = run(&model, &[um("Extract the fields.")], &schema()).expect("should succeed");

    // 原生路径：content 为解析后的 JSON。
    assert_eq!(
        resp.content,
        serde_json::json!({"name": "Alice", "age": 30})
    );
    // message_id 写入 metadata（extract_structured）。
    assert_eq!(
        resp.metadata.get("message_id"),
        Some(&serde_json::json!("msg_1"))
    );
    // 只发生一次请求，且填了 output_schema（未回退）。
    let recorded = requests.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert!(
        recorded[0].output_schema.is_some(),
        "native path must fill output_schema"
    );
}

#[test]
fn native_rejected_bad_request_falls_back_to_tool_bypass() {
    let (backend, requests) = MockBackend::new(vec![
        Err(bad_request()),
        ok_tool_call(serde_json::json!({"name": "Bob", "age": 25})),
    ]);
    let model = RigChatModel::from_backend_for_testing(
        RigProviderKind::OpenAi,
        "gpt-4o-mini",
        Arc::new(backend),
    );

    let resp = run(&model, &[um("Extract the fields.")], &schema()).expect("should succeed");

    assert_eq!(resp.content, serde_json::json!({"name": "Bob", "age": 25}));

    let recorded = requests.lock().unwrap();
    assert_eq!(
        recorded.len(),
        2,
        "rejected native request must trigger one fallback call"
    );
    // 第一次：原生 output_schema。
    assert!(recorded[0].output_schema.is_some());
    // 第二次：bypass 工具 + tool_choice 强制。
    let second = &recorded[1];
    assert!(
        second.output_schema.is_none(),
        "bypass path must not set output_schema"
    );
    assert!(
        second.tool_choice.is_some(),
        "bypass path must force tool_choice"
    );
    assert_eq!(second.tools.len(), 1);
    assert_eq!(second.tools[0].name, "generate_structured_output");
}

#[test]
fn native_success_without_json_falls_back_to_tool_bypass() {
    let (backend, requests) = MockBackend::new(vec![
        ok_text("Sorry, I cannot produce JSON here.", None),
        ok_tool_call(serde_json::json!({"name": "Carol", "age": 40})),
    ]);
    let model = RigChatModel::from_backend_for_testing(
        RigProviderKind::OpenAi,
        "gpt-4o-mini",
        Arc::new(backend),
    );

    let resp = run(&model, &[um("Extract the fields.")], &schema()).expect("should succeed");

    assert_eq!(
        resp.content,
        serde_json::json!({"name": "Carol", "age": 40})
    );
    let recorded = requests.lock().unwrap();
    assert_eq!(recorded.len(), 2);
    assert!(
        recorded[1].output_schema.is_none(),
        "fallback must be the bypass path"
    );
}

#[test]
fn native_json_repair_fixes_missing_brace() {
    // 原生 Text 缺右花括号（serde 拒绝），extract_structured 用 json_repair 修复。
    let (backend, _requests) =
        MockBackend::new(vec![ok_text(r#"{"name": "Alice", "age": 30"#, None)]);
    let model = RigChatModel::from_backend_for_testing(
        RigProviderKind::OpenAi,
        "gpt-4o-mini",
        Arc::new(backend),
    );

    let resp =
        run(&model, &[um("Extract the fields.")], &schema()).expect("json_repair should fix it");
    assert_eq!(
        resp.content,
        serde_json::json!({"name": "Alice", "age": 30})
    );
}

#[test]
fn bypass_string_arguments_json_repair() {
    // 工具 bypass 的 arguments 以字符串返回（个别 provider），缺右花括号 → 修复。
    let (backend, _requests) = MockBackend::new(vec![
        Err(bad_request()),
        ok_tool_call(serde_json::Value::String(
            r#"{"name": "Dan", "age": 33"#.to_string(),
        )),
    ]);
    let model = RigChatModel::from_backend_for_testing(
        RigProviderKind::OpenAi,
        "gpt-4o-mini",
        Arc::new(backend),
    );

    let resp = run(&model, &[um("Extract the fields.")], &schema()).expect("should succeed");
    assert_eq!(resp.content, serde_json::json!({"name": "Dan", "age": 33}));
}

#[test]
fn empty_messages_is_validation_error() {
    let (backend, _requests) = MockBackend::new(vec![]);
    let model = RigChatModel::from_backend_for_testing(
        RigProviderKind::OpenAi,
        "gpt-4o-mini",
        Arc::new(backend),
    );

    let err = run(&model, &[], &schema()).expect_err("empty messages must fail");
    assert!(
        matches!(err, ModelError::ValidationError { .. }),
        "expected ValidationError, got {err}"
    );
}
