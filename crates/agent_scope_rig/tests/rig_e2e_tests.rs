//! T028 — rig-backed 模型驱动完整 ReAct 循环端到端测试（wiremock HTTP mock）。
//!
//! mock HTTP 回放两轮流式 OpenAI-compatible SSE 响应（第一轮：工具调用；
//! 第二轮：文本），驱动 `RigChatModel` 跑真实 `ReActAgent`，断言（契约
//! `specs/034-rig-llm-integration/contracts/rig-mapping.md` §5 顺序契约）：
//! - 事件顺序：REPLY_START → MODEL_CALL_START → TOOL_CALL_START → TOOL_CALL_DELTA
//!   → TOOL_CALL_END → MODEL_CALL_END → TOOL_RESULT_START → TOOL_RESULT_TEXT_DELTA
//!   → TOOL_RESULT_END → MODEL_CALL_START → TEXT_BLOCK_START → TEXT_BLOCK_DELTA
//!   → TEXT_BLOCK_END → MODEL_CALL_END → REPLY_END；
//! - `tool_call_id_map` 回填：工具调用与工具结果共享同一稳定 block_id（`tc_0`），
//!   且第二轮请求中 assistant tool_call id 与 tool role `tool_call_id` 一致
//!   （provider id `call_weather_01` → 稳定 `tc_0` 的全链路回填证据）；
//! - 每轮 `is_last` 收尾元数据：usage（ModelCallEnd 的 input/output tokens）与
//!   `finished_reason=Completed`（FR-004：provider 替换后可观察行为与迁移前基线一致）。

use std::sync::Arc;

use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_message::factory::user_msg;
use agent_scope_rig::RigChatModel;
use agent_scope_tool::{FunctionTool, ToolKit};
use futures::StreamExt;
// agent_scope_tool 的 FunctionTool::new 要求 schemars **0.8** 的 JsonSchema；
// derive 宏展开用相对路径 `schemars::`，本地 alias 使其解析到 0.8
// （主 dep schemars="1" 仅 lib 用，匹配 rig-core）。
use schemars::JsonSchema;
use schemars_08 as schemars;
use serde::Deserialize;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// 事件 tag（顺序断言用）。
fn event_tag(event: &agent_scope_event::AgentEvent) -> &'static str {
    use agent_scope_event::AgentEvent;
    match event {
        AgentEvent::ReplyStart(_) => "REPLY_START",
        AgentEvent::ReplyEnd(_) => "REPLY_END",
        AgentEvent::ModelCallStart(_) => "MODEL_CALL_START",
        AgentEvent::ModelCallEnd(_) => "MODEL_CALL_END",
        AgentEvent::TextBlockStart(_) => "TEXT_BLOCK_START",
        AgentEvent::TextBlockDelta(_) => "TEXT_BLOCK_DELTA",
        AgentEvent::TextBlockEnd(_) => "TEXT_BLOCK_END",
        AgentEvent::ToolCallStart(_) => "TOOL_CALL_START",
        AgentEvent::ToolCallDelta(_) => "TOOL_CALL_DELTA",
        AgentEvent::ToolCallEnd(_) => "TOOL_CALL_END",
        AgentEvent::ToolResultStart(_) => "TOOL_RESULT_START",
        AgentEvent::ToolResultTextDelta(_) => "TOOL_RESULT_TEXT_DELTA",
        AgentEvent::ToolResultEnd(_) => "TOOL_RESULT_END",
        AgentEvent::ExceedMaxIters(_) => "EXCEED_MAX_ITERS",
        AgentEvent::UserInterrupt(_) => "USER_INTERRUPT",
        _ => "OTHER",
    }
}

/// 第一轮流式 SSE：工具调用（get_weather，name → arguments 分片 →
/// finish_reason=tool_calls → usage → [DONE]）。
fn tool_call_sse() -> String {
    r#"data: {"id":"chatcmpl-t1","object":"chat.completion.chunk","created":0,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_weather_01","type":"function","function":{"name":"get_weather","arguments":""}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-t1","object":"chat.completion.chunk","created":0,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"city\":\"Beijing\"}"}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-t1","object":"chat.completion.chunk","created":0,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}

data: {"id":"chatcmpl-t1","object":"chat.completion.chunk","created":0,"model":"gpt-4o-mini","choices":[],"usage":{"prompt_tokens":10,"completion_tokens":3,"total_tokens":13}}

data: [DONE]

"#
    .to_string()
}

/// 第二轮流式 SSE：文本回复（两 content 分片 → finish_reason=stop → usage → [DONE]）。
fn text_sse() -> String {
    r#"data: {"id":"chatcmpl-t2","object":"chat.completion.chunk","created":0,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{"role":"assistant","content":"Beijing is sunny, 25"},"finish_reason":null}]}

data: {"id":"chatcmpl-t2","object":"chat.completion.chunk","created":0,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{"content":"C today."},"finish_reason":null}]}

data: {"id":"chatcmpl-t2","object":"chat.completion.chunk","created":0,"model":"gpt-4o-mini","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: {"id":"chatcmpl-t2","object":"chat.completion.chunk","created":0,"model":"gpt-4o-mini","choices":[],"usage":{"prompt_tokens":20,"completion_tokens":4,"total_tokens":24}}

data: [DONE]

"#
    .to_string()
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct WeatherInput {
    city: String,
}

/// 工具 handler：返回城市天气文本（作为 ToolResultTextDelta 流出）。
async fn weather_handler(input: WeatherInput) -> String {
    format!("{}: sunny, 25C today", input.city)
}

/// 断言实际事件标签序列包含 `expected` 且保持相对顺序（过滤 OTHER / 额外 delta）。
fn assert_sequence(tags: &[&str], expected: &[&str]) {
    let mut idx = 0usize;
    for exp in expected {
        let found = tags[idx..]
            .iter()
            .position(|t| *t == *exp)
            .unwrap_or_else(|| panic!("event {exp} not found after position {idx} in {tags:?}"));
        idx += found + 1;
    }
}

#[tokio::test]
async fn react_loop_tool_call_then_text_event_order_and_ids() {
    let mock_server = MockServer::start().await;

    // 注册顺序决定匹配顺序（wiremock 稳定排序按 priority，同优先级保持注册序）：
    // 1) tool_call mock 先注册（position 0）→ 第一轮匹配，`up_to_n_times(1)` 用尽后
    //    `matches()` 返回 false，fall-through 到文本 mock；
    // 2) 文本 mock 后注册（position 1）→ 第二轮匹配。
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("Authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(tool_call_sse(), "text/event-stream"))
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("Authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(text_sse(), "text/event-stream"))
        .mount(&mock_server)
        .await;

    // rig-backed OpenAI 聊天模型，指向 wiremock。
    let model = Arc::new(
        RigChatModel::openai("test-key", "gpt-4o-mini")
            .unwrap()
            .with_base_url(mock_server.uri())
            .with_stream(true),
    );

    // 工具 + ReActAgent。
    let mut toolkit = ToolKit::new();
    toolkit.register(FunctionTool::new(
        "get_weather",
        "Get weather for a city",
        weather_handler,
    ));
    let config = AgentConfig::builder()
        .name("weather-agent")
        .system_prompt("You are a weather assistant.")
        .model(model)
        .toolkit(toolkit)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    let input = user_msg("user", "What is the weather in Beijing?").unwrap();
    let stream = agent.reply_stream(Some(vec![input])).await.unwrap();

    tokio::pin!(stream);
    let mut tags: Vec<&str> = Vec::new();
    let mut tc_ids = (None, None, None, None); // start / end / result_start / result_end
    let mut model_call_usage: Vec<(i64, i64, String)> = Vec::new();
    let mut reply_end_reason: Option<String> = None;

    while let Some(event) = stream.next().await {
        tags.push(event_tag(&event));
        use agent_scope_event::AgentEvent;
        match &event {
            AgentEvent::ToolCallStart(e) => tc_ids.0 = Some(e.tool_call_id.clone()),
            AgentEvent::ToolCallEnd(e) => tc_ids.1 = Some(e.tool_call_id.clone()),
            AgentEvent::ToolResultStart(e) => tc_ids.2 = Some(e.tool_call_id.clone()),
            AgentEvent::ToolResultEnd(e) => tc_ids.3 = Some(e.tool_call_id.clone()),
            AgentEvent::ModelCallEnd(e) => {
                model_call_usage.push((
                    e.input_tokens,
                    e.output_tokens,
                    format!("{:?}", e.finished_reason),
                ));
            }
            AgentEvent::ReplyEnd(e) => {
                reply_end_reason = Some(format!("{:?}", e.finished_reason));
            }
            _ => {}
        }
    }

    // ── 1. 事件顺序：完整 ReAct 循环（工具调用 → 工具结果 → 文本回复）───────────
    assert_sequence(
        &tags,
        &[
            "REPLY_START",
            "MODEL_CALL_START",
            "TOOL_CALL_START",
            "TOOL_CALL_DELTA",
            "TOOL_CALL_END",
            "MODEL_CALL_END",
            "TOOL_RESULT_START",
            "TOOL_RESULT_TEXT_DELTA",
            "TOOL_RESULT_END",
            "MODEL_CALL_START",
            "TEXT_BLOCK_START",
            "TEXT_BLOCK_DELTA",
            "TEXT_BLOCK_END",
            "MODEL_CALL_END",
            "REPLY_END",
        ],
    );
    assert!(
        tags.ends_with(&["REPLY_END"]),
        "stream must end with REPLY_END, got: {tags:?}"
    );

    // ── 2. tool_call_id_map 回填：四类事件共享同一稳定 block_id（tc_0）─────────
    assert_eq!(tc_ids.0.as_deref(), Some("tc_0"), "ToolCallStart id");
    assert_eq!(tc_ids.1.as_deref(), Some("tc_0"), "ToolCallEnd id");
    assert_eq!(tc_ids.2.as_deref(), Some("tc_0"), "ToolResultStart id");
    assert_eq!(tc_ids.3.as_deref(), Some("tc_0"), "ToolResultEnd id");

    // ── 3. 每轮 is_last 收尾元数据：usage / finished_reason ────────────────────
    assert_eq!(model_call_usage.len(), 2, "two model calls expected");
    assert_eq!(model_call_usage[0].0, 10, "round 1 input tokens");
    assert_eq!(model_call_usage[0].1, 3, "round 1 output tokens");
    assert_eq!(model_call_usage[0].2, "Completed");
    assert_eq!(model_call_usage[1].0, 20, "round 2 input tokens");
    assert_eq!(model_call_usage[1].1, 4, "round 2 output tokens");
    assert_eq!(model_call_usage[1].2, "Completed");
    assert_eq!(reply_end_reason.as_deref(), Some("Completed"));

    // ── 4. 第二轮请求：assistant tool_call id 与 tool role tool_call_id 一致 ────
    //   （provider id call_weather_01 → 稳定 block_id tc_0 的全链路回填证据：
    //   agent 以 tc_0 关联工具调用与工具结果，rig 序列化 tool result 时
    //   call_id=None fallback 到 id=tc_0，两轮请求自洽。）
    let requests = mock_server
        .received_requests()
        .await
        .expect("requests recorded");
    assert_eq!(requests.len(), 2, "exactly two model calls expected");
    let second: serde_json::Value =
        serde_json::from_slice(&requests[1].body).expect("second request json body");
    let msgs = second["messages"]
        .as_array()
        .expect("messages array in second request");
    let assistant = msgs
        .iter()
        // 运行时注入（runtime_injection，Feature 026）会以独立 assistant 消息携带
        // `<system-reminder>` Hint（Feature 034 起 Hint 以文本发送），因此"第一条
        // assistant 消息"不一定是带 tool_calls 的那条——按 tool_calls 定位。
        .find(|m| {
            m["role"] == "assistant"
                && m["tool_calls"]
                    .as_array()
                    .is_some_and(|tcs| !tcs.is_empty())
        })
        .expect("assistant message with tool_calls in second request");
    assert_eq!(assistant["tool_calls"][0]["id"], "tc_0");
    assert_eq!(
        assistant["tool_calls"][0]["function"]["name"],
        "get_weather"
    );
    let tool = msgs
        .iter()
        .find(|m| m["role"] == "tool")
        .expect("tool result message in second request");
    assert_eq!(tool["tool_call_id"], "tc_0");
    // rig 单文本 ToolResult 的 content 序列化为字符串（ToolResultContentValue::String），
    // 多 part 才用数组；两种格式都应能取回文本。
    let tool_content = tool["content"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| tool["content"][0]["text"].as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    assert!(
        tool_content.contains("Beijing"),
        "tool result text must roundtrip, got: {tool_content}"
    );
}
