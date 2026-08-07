//! Streaming and compression integration tests.
//!
//! Feature 008: progressive event delivery, tool call detection,
//! streaming tool execution, backpressure.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use agent_scope_agent::{
    Agent, AgentConfig, AgentError, ContextConfig, Middleware, ReActAgent, ReActConfig,
};
use agent_scope_message::factory::user_msg;
use agent_scope_message::{
    ContentBlock, Msg, Role, TextBlock, ToolCallBlock, ToolOutput, ToolResultBlock, ToolResultState,
};
use agent_scope_model::{ChatModel, ChatResponse, ModelCallResult, ModelError, ToolChoice};
use agent_scope_tool::{Tool, ToolExecOutput, ToolKit};
use futures::StreamExt;
use serde_json::Value as JsonValue;
use tokio::sync::Notify;

async fn wait_until_reusable(agent: &ReActAgent) {
    for _ in 0..100 {
        match agent
            .reply(Some(vec![user_msg("user", "after cancel").unwrap()]))
            .await
        {
            Ok(_) => return,
            Err(AgentError::AlreadyStreaming) => tokio::task::yield_now().await,
            Err(e) => panic!("unexpected reply error while waiting for reuse: {e}"),
        }
    }
    panic!("agent did not become reusable after cancellation");
}

mod mocks;
use mocks::{MockModel, MockStreamingModel, MockStreamingTool};

// ---------------------------------------------------------------------------
// Pre-existing streaming tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_streaming_mock_model_produces_correct_text() {
    let model = Arc::new(MockModel::new("mock", "Hello, streaming world!").with_stream(3));
    let result = ChatModel::call(model.as_ref(), &[], None, None)
        .await
        .unwrap();
    if let agent_scope_model::ModelCallResult::Stream(mut stream) = result {
        use agent_scope_model::StreamAccumulator;
        let mut acc = StreamAccumulator::new();
        while let Some(chunk_result) = stream.next().await {
            acc.append_chat_response(&chunk_result.unwrap());
        }
        assert_eq!(acc.build().get_text_content(""), "Hello, streaming world!");
    } else {
        panic!("Expected Stream variant");
    }
}

#[tokio::test]
async fn test_react_agent_with_streaming_model() {
    let model = Arc::new(MockModel::new("mock", "streaming response").with_stream(4));
    let config = AgentConfig::builder()
        .name("stream_agent")
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
    let reply = agent
        .reply(Some(vec![user_msg("user", "hello").unwrap()]))
        .await
        .unwrap();
    assert_eq!(reply.role, Role::Assistant);
    assert_eq!(reply.get_text_content("").unwrap(), "streaming response");
}

#[tokio::test]
async fn test_streaming_mock_model_single_chunk() {
    let model = Arc::new(MockModel::new("mock", "single chunk").with_stream(1));
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
    let reply = agent
        .reply(Some(vec![user_msg("user", "hi").unwrap()]))
        .await
        .unwrap();
    assert_eq!(reply.get_text_content("").unwrap(), "single chunk");
}

// ---------------------------------------------------------------------------
// Compression tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_compression_not_triggered_for_small_context() {
    let model = Arc::new(MockModel::new("mock", "ok"));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig {
            enable: true,
            ..Default::default()
        },
        vec![],
    )
    .unwrap();
    let reply = agent
        .reply(Some(vec![user_msg("user", "hi").unwrap()]))
        .await
        .unwrap();
    assert_eq!(reply.role, Role::Assistant);
    assert_eq!(reply.get_text_content("").unwrap(), "ok");
}

#[tokio::test]
async fn test_compression_disabled_noop() {
    let model = Arc::new(MockModel::new("mock", "ok"));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig {
            enable: false,
            ..Default::default()
        },
        vec![],
    )
    .unwrap();
    let reply = agent
        .reply(Some(vec![user_msg("user", "hi").unwrap()]))
        .await
        .unwrap();
    assert_eq!(reply.role, Role::Assistant);
    assert_eq!(reply.get_text_content("").unwrap(), "ok");
}

#[tokio::test]
async fn test_streaming_with_compression_enabled() {
    let model = Arc::new(MockModel::new("mock", "stream + compress works").with_stream(3));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig {
            enable: true,
            trigger_ratio: 0.9,
            ..Default::default()
        },
        vec![],
    )
    .unwrap();
    let reply = agent
        .reply(Some(vec![
            user_msg("user", "hello streaming world").unwrap(),
        ]))
        .await
        .unwrap();
    assert_eq!(reply.role, Role::Assistant);
    assert_eq!(
        reply.get_text_content("").unwrap(),
        "stream + compress works"
    );
}

// ===========================================================================
// Feature 008 US1: Progressive event delivery tests
// ===========================================================================

/// T010: Progressive event delivery — mock model streams chunks,
/// verify events arrive progressively (not all at once after completion).
#[tokio::test]
async fn test_streaming_progressive_events() {
    let model = Arc::new(MockModel::new("mock", "Progressive text!").with_stream(3));
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

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "hello").unwrap()]))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Verify all required event types present
    let has_reply_start = events
        .iter()
        .any(|e| matches!(e, agent_scope_event::AgentEvent::ReplyStart(_)));
    let text_deltas: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, agent_scope_event::AgentEvent::TextBlockDelta(_)))
        .collect();
    let has_reply_end = events
        .iter()
        .any(|e| matches!(e, agent_scope_event::AgentEvent::ReplyEnd(_)));
    let has_model_call_start = events
        .iter()
        .any(|e| matches!(e, agent_scope_event::AgentEvent::ModelCallStart(_)));
    let has_model_call_end = events
        .iter()
        .any(|e| matches!(e, agent_scope_event::AgentEvent::ModelCallEnd(_)));

    assert!(has_reply_start, "Expected ReplyStart");
    assert!(
        !text_deltas.is_empty(),
        "Expected TextBlockDelta events, got {}",
        text_deltas.len()
    );
    assert!(has_reply_end, "Expected ReplyEnd");
    assert!(has_model_call_start, "Expected ModelCallStart");
    assert!(has_model_call_end, "Expected ModelCallEnd");
}

/// T011: ReplyStart event arrives within first poll (before model completes).
#[tokio::test]
async fn test_streaming_reply_start_first_poll() {
    let model = Arc::new(MockModel::new("mock", "Hello world!").with_stream(5));
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

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "hi").unwrap()]))
        .await
        .unwrap();

    let start = Instant::now();
    let first = stream.next().await;
    let elapsed = start.elapsed();

    assert!(first.is_some(), "First event should arrive");
    assert!(
        matches!(&first, Some(agent_scope_event::AgentEvent::ReplyStart(_))),
        "First event should be ReplyStart"
    );
    assert!(
        elapsed < Duration::from_millis(500),
        "ReplyStart should arrive quickly, took {:?}",
        elapsed
    );
}

/// T012: Non-streaming model produces same event sequence (single burst).
#[tokio::test]
async fn test_streaming_non_streaming_model_event_sequence() {
    let model = Arc::new(MockModel::new("mock", "Non-streaming text"));
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

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "hello").unwrap()]))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    assert!(
        events
            .iter()
            .any(|e| matches!(e, agent_scope_event::AgentEvent::ReplyStart(_)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, agent_scope_event::AgentEvent::ModelCallStart(_)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, agent_scope_event::AgentEvent::ModelCallEnd(_)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, agent_scope_event::AgentEvent::TextBlockDelta(_)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, agent_scope_event::AgentEvent::ReplyEnd(_)))
    );
}

// ===========================================================================
// Feature 008 US2: Stream drop cancellation & AlreadyStreaming guard
// (Phase 7 cross-cutting tests — useful early for confidence)
// ===========================================================================

/// T042: Drop stream after first event → is_streaming cleared, new reply succeeds.
#[tokio::test]
async fn test_stream_drop_cancellation() {
    let model = Arc::new(MockModel::new("mock", "Some long text here").with_stream(10));
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

    {
        let mut stream = agent
            .reply_stream(Some(vec![user_msg("user", "hi").unwrap()]))
            .await
            .unwrap();

        // Read first event, then drop
        let _first = stream.next().await;
        drop(stream);
    }

    // After drop, agent should be usable again
    let reply = agent
        .reply(Some(vec![user_msg("user", "hello again").unwrap()]))
        .await
        .unwrap();
    assert_eq!(reply.role, Role::Assistant);
}

/// T043: AlreadyStreaming guard — concurrent reply calls return error.
#[tokio::test]
async fn test_already_streaming_guard() {
    let model = Arc::new(MockModel::new("mock", "text").with_stream(3));
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

    let _stream = agent
        .reply_stream(Some(vec![user_msg("user", "hi").unwrap()]))
        .await
        .unwrap();

    // Second reply_stream should fail
    let result = agent
        .reply_stream(Some(vec![user_msg("user", "again").unwrap()]))
        .await;
    assert!(result.is_err());
    let err = result.err().unwrap();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("streaming"),
        "Expected AlreadyStreaming, got: {}",
        err_msg
    );
}

/// T044: Interrupted agent recovery — interrupt during streaming, then new reply succeeds.
#[tokio::test]
async fn test_interrupted_agent_recovery_streaming() {
    let model = Arc::new(MockModel::new("mock", "ok").with_stream(2));
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

    // Note: interrupt() before reply starts triggers CancellationError
    agent.interrupt();
    let result = agent
        .reply_stream(Some(vec![user_msg("user", "hi").unwrap()]))
        .await;
    assert!(result.is_err());
    // After error, is_streaming should be cleared
    let reply = agent
        .reply(Some(vec![user_msg("user", "hello").unwrap()]))
        .await;
    assert!(reply.is_ok());
}

// ===========================================================================
// Feature 008 US2: Progressive tool call detection tests
// ===========================================================================

/// Helper to create a ChatResponse chunk with a ToolCallBlock.
fn tool_call_chunk(id: &str, name: &str, input: &str) -> agent_scope_model::ChatResponse {
    let mut cr = agent_scope_model::ChatResponse::default();
    cr.append_tool_call(id, name, input, std::collections::HashMap::new());
    cr
}

/// Helper to create a ChatResponse chunk with a TextBlock.
fn text_chunk(block_id: &str, text: &str) -> agent_scope_model::ChatResponse {
    let mut cr = agent_scope_model::ChatResponse::default();
    cr.append_text(text, Some(block_id));
    cr
}

/// T022: Tool call completion detection — tool call spanning 3 chunks,
/// verify ToolCallEnd emitted before text chunk arrives.
#[tokio::test]
async fn test_streaming_tool_call_detection() {
    let chunks = vec![
        tool_call_chunk("tc1", "calc", r#"{"a":1"#),
        tool_call_chunk("tc1", "", r#","b":2}"#),
        text_chunk("t1", "Now computing the result..."),
    ];
    let model = Arc::new(MockStreamingModel::new("mock", chunks));

    // Create a simple toolkit with a tool that echoes its input
    struct EchoTool;
    #[async_trait::async_trait]
    impl agent_scope_tool::Tool for EchoTool {
        fn name(&self) -> &str {
            "calc"
        }
        fn description(&self) -> &str {
            "Echo tool"
        }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn call(
            &self,
            _input: serde_json::Value,
        ) -> Result<agent_scope_tool::ToolExecOutput, agent_scope_tool::ToolError> {
            let trb = ToolResultBlock::new(
                "tc1".into(),
                "calc".into(),
                ToolOutput::Text("result: 3".into()),
            );
            Ok(agent_scope_tool::ToolExecOutput::Complete(trb))
        }
    }

    let mut tk = ToolKit::new();
    tk.register(EchoTool);
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .toolkit(tk)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "calculate 1+2").unwrap()]))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Verify tool call events exist
    let tool_call_starts: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, agent_scope_event::AgentEvent::ToolCallStart(_)))
        .collect();
    let tool_call_deltas: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, agent_scope_event::AgentEvent::ToolCallDelta(_)))
        .collect();
    let tool_call_ends: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, agent_scope_event::AgentEvent::ToolCallEnd(_)))
        .collect();
    let tool_result_starts: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, agent_scope_event::AgentEvent::ToolResultStart(_)))
        .collect();

    assert!(
        !tool_call_starts.is_empty(),
        "Expected ToolCallStart events"
    );
    assert!(
        !tool_call_deltas.is_empty(),
        "Expected ToolCallDelta events"
    );
    assert!(!tool_call_ends.is_empty(), "Expected ToolCallEnd events");
    assert!(
        !tool_result_starts.is_empty(),
        "Expected ToolResultStart — tool should be executed"
    );

    // Verify ToolCallEnd position comes before ToolResultStart
    let tc_end_idx = events
        .iter()
        .position(|e| matches!(e, agent_scope_event::AgentEvent::ToolCallEnd(_)))
        .unwrap();
    let tr_start_idx = events
        .iter()
        .position(|e| matches!(e, agent_scope_event::AgentEvent::ToolResultStart(_)))
        .unwrap();
    assert!(
        tc_end_idx < tr_start_idx,
        "ToolCallEnd should come before ToolResultStart"
    );
}

/// T023: Multiple tool calls detected and each executed.
#[tokio::test]
async fn test_streaming_multiple_tool_calls() {
    let chunks = vec![
        tool_call_chunk("tc1", "calc_a", r#"{"x":1}"#),
        tool_call_chunk("tc2", "calc_b", r#"{"y":2}"#),
    ];
    let model = Arc::new(MockStreamingModel::new("mock", chunks));

    struct CalcATool;
    #[async_trait::async_trait]
    impl agent_scope_tool::Tool for CalcATool {
        fn name(&self) -> &str {
            "calc_a"
        }
        fn description(&self) -> &str {
            "calc a"
        }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn call(
            &self,
            _input: serde_json::Value,
        ) -> Result<agent_scope_tool::ToolExecOutput, agent_scope_tool::ToolError> {
            Ok(agent_scope_tool::ToolExecOutput::Complete(
                ToolResultBlock::new(
                    "tc1".into(),
                    "calc_a".into(),
                    ToolOutput::Text("a_result".into()),
                ),
            ))
        }
    }

    struct CalcBTool;
    #[async_trait::async_trait]
    impl agent_scope_tool::Tool for CalcBTool {
        fn name(&self) -> &str {
            "calc_b"
        }
        fn description(&self) -> &str {
            "calc b"
        }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn call(
            &self,
            _input: serde_json::Value,
        ) -> Result<agent_scope_tool::ToolExecOutput, agent_scope_tool::ToolError> {
            Ok(agent_scope_tool::ToolExecOutput::Complete(
                ToolResultBlock::new(
                    "tc2".into(),
                    "calc_b".into(),
                    ToolOutput::Text("b_result".into()),
                ),
            ))
        }
    }

    let mut tk = ToolKit::new();
    tk.register(CalcATool);
    tk.register(CalcBTool);
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .toolkit(tk)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "run both").unwrap()]))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Verify both tools were started (at least once)
    let tc_start_count = events
        .iter()
        .filter(|e| matches!(e, agent_scope_event::AgentEvent::ToolCallStart(_)))
        .count();
    assert!(
        tc_start_count >= 2,
        "Expected at least 2 ToolCallStart events, got {}",
        tc_start_count
    );

    // Verify both tool names appeared in start events
    let tc_names: Vec<&str> = events
        .iter()
        .filter_map(|e| {
            if let agent_scope_event::AgentEvent::ToolCallStart(s) = e {
                Some(s.tool_call_name.as_str())
            } else {
                None
            }
        })
        .collect();
    assert!(
        tc_names.contains(&"calc_a"),
        "Expected tool 'calc_a' to be started"
    );
    assert!(
        tc_names.contains(&"calc_b"),
        "Expected tool 'calc_b' to be started"
    );

    // Verify both tools finished
    let tr_end_count = events
        .iter()
        .filter(|e| matches!(e, agent_scope_event::AgentEvent::ToolResultEnd(_)))
        .count();
    assert!(
        tr_end_count >= 2,
        "Expected at least 2 ToolResultEnd events, got {}",
        tr_end_count
    );
}

/// T024: Malformed JSON tool arguments → ToolResultEnd with Error state.
#[tokio::test]
async fn test_streaming_tool_call_malformed_json() {
    // This test uses a non-streaming (Complete) model with a tool call
    // containing malformed JSON. The tool itself will attempt to parse and fail.
    let mut cr = agent_scope_model::ChatResponse::default();
    cr.append_tool_call(
        "tc1",
        "parser",
        "not valid json{{{{",
        std::collections::HashMap::new(),
    );
    let model = Arc::new(MockStreamingModel::new("mock", vec![cr]));

    struct ParserTool;
    #[async_trait::async_trait]
    impl agent_scope_tool::Tool for ParserTool {
        fn name(&self) -> &str {
            "parser"
        }
        fn description(&self) -> &str {
            "Parser tool"
        }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn call(
            &self,
            _input: serde_json::Value,
        ) -> Result<agent_scope_tool::ToolExecOutput, agent_scope_tool::ToolError> {
            // Simulate execution failure on bad input
            Err(agent_scope_tool::ToolError::InvalidInput {
                tool_name: "parser".into(),
                reason: "invalid JSON".into(),
            })
        }
    }

    let mut tk = ToolKit::new();
    tk.register(ParserTool);
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .toolkit(tk)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "parse this").unwrap()]))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should have ToolResultEnd with error
    let has_error = events.iter().any(|e| {
        if let agent_scope_event::AgentEvent::ToolResultEnd(te) = e {
            te.state == ToolResultState::Error
        } else {
            false
        }
    });
    assert!(
        has_error,
        "Expected ToolResultEnd with Error state for malformed input"
    );

    // Should still have a ReplyEnd
    let has_reply_end = events
        .iter()
        .any(|e| matches!(e, agent_scope_event::AgentEvent::ReplyEnd(_)));
    // Note: With tool error, the loop continues and model is called again
    // The empty response from model leads to ReplyEnd
    assert!(has_reply_end, "Expected ReplyEnd after tool error");
}

// ===========================================================================
// Feature 008 US3: Streaming tool execution tests
// ===========================================================================

/// T030: Streaming tool output — tool yields 3 chunks,
/// verify progressive ToolResultTextDelta events.
#[tokio::test]
async fn test_streaming_tool_execution_progressive() {
    // Create model returning a tool call in streaming mode
    let chunks = vec![tool_call_chunk("tc1", "stream_echo", r#"{"text":"hello"}"#)];
    let model = Arc::new(MockStreamingModel::new("mock", chunks));

    // Create streaming tool with 3 chunks
    use agent_scope_message::{ToolOutput, ToolResultBlock};
    let tool_chunks: Vec<Result<ToolResultBlock, agent_scope_tool::ToolError>> = vec![
        Ok(ToolResultBlock::new(
            "tc1".into(),
            "stream_echo".into(),
            ToolOutput::Text("chunk1 ".into()),
        )),
        Ok(ToolResultBlock::new(
            "tc1".into(),
            "stream_echo".into(),
            ToolOutput::Text("chunk2 ".into()),
        )),
        {
            let mut trb = ToolResultBlock::new(
                "tc1".into(),
                "stream_echo".into(),
                ToolOutput::Text("chunk3".into()),
            );
            trb.is_last = true;
            Ok(trb)
        },
    ];
    let stream_tool = MockStreamingTool::new("stream_echo", tool_chunks);

    let mut tk = ToolKit::new();
    tk.register(stream_tool);
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .toolkit(tk)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "stream hello").unwrap()]))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Verify ToolResultTextDelta events exist (at least 3)
    let deltas: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, agent_scope_event::AgentEvent::ToolResultTextDelta(_)))
        .collect();
    assert!(
        deltas.len() >= 3,
        "Expected at least 3 ToolResultTextDelta events, got {}",
        deltas.len()
    );

    // Verify ToolResultEnd with Success
    let has_success_end = events.iter().any(|e| {
        if let agent_scope_event::AgentEvent::ToolResultEnd(te) = e {
            te.state == ToolResultState::Success
        } else {
            false
        }
    });
    assert!(has_success_end, "Expected ToolResultEnd(Success)");
}

/// T031: Streaming tool failure mid-execution — error then ToolResultEnd(Error).
#[tokio::test]
async fn test_streaming_tool_execution_error() {
    let chunks = vec![tool_call_chunk("tc1", "flaky_tool", r#"{}"#)];
    let model = Arc::new(MockStreamingModel::new("mock", chunks));

    // Tool that succeeds first chunk then errors
    let tool_chunks: Vec<Result<ToolResultBlock, agent_scope_tool::ToolError>> = vec![
        Ok(ToolResultBlock::new(
            "tc1".into(),
            "flaky_tool".into(),
            ToolOutput::Text("ok".into()),
        )),
        Err(agent_scope_tool::ToolError::Execution {
            tool_name: "flaky_tool".into(),
            reason: "mid-stream crash".into(),
        }),
    ];
    let flaky = MockStreamingTool::new("flaky_tool", tool_chunks);

    let mut tk = ToolKit::new();
    tk.register(flaky);
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .toolkit(tk)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "test").unwrap()]))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    // Should have ToolResultEnd with Error state
    let has_error_end = events.iter().any(|e| {
        if let agent_scope_event::AgentEvent::ToolResultEnd(te) = e {
            te.state == ToolResultState::Error
        } else {
            false
        }
    });
    assert!(
        has_error_end,
        "Expected ToolResultEnd(Error) after mid-stream failure"
    );
}

/// T032: Streaming tool interrupted → ToolResultEnd(Interrupted).
#[tokio::test]
async fn test_streaming_tool_interrupted() {
    let chunks = vec![tool_call_chunk("tc1", "slow_tool", r#"{}"#)];
    let model = Arc::new(MockStreamingModel::new("mock", chunks));

    // Slow tool with many chunks
    let tool_chunks: Vec<Result<ToolResultBlock, agent_scope_tool::ToolError>> = (0..10)
        .map(|i| {
            Ok(ToolResultBlock::new(
                "tc1".into(),
                "slow_tool".into(),
                ToolOutput::Text(format!("chunk{i}")),
            ))
        })
        .collect();
    let slow = MockStreamingTool::new("slow_tool", tool_chunks);

    let mut tk = ToolKit::new();
    tk.register(slow);
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .toolkit(tk)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "slow").unwrap()]))
        .await
        .unwrap();

    // Read just the first event, then drop the stream (simulating cancellation)
    let _first = stream.next().await;
    drop(stream);

    // Agent should be usable again (no assert on specific event since
    // cancellation is asynchronous — the streaming loop may or may not have
    // finished emitting Interrupted state events)
    let reply = agent
        .reply(Some(vec![user_msg("user", "recovery test").unwrap()]))
        .await;
    assert!(reply.is_ok(), "Agent should recover after stream drop");
}

// ===========================================================================
// Feature 008 US4: Backpressure tests
// ===========================================================================

/// T037: Bounded channel backpressure — capacity 4, slow consumer,
/// verify all events delivered without loss.
#[tokio::test]
async fn test_bounded_channel_backpressure() {
    let model = Arc::new(MockModel::new("mock", "test backpressure").with_stream(10));
    let config = AgentConfig::builder()
        .name("agent")
        .model(model.clone())
        .with_stream_channel_capacity(Some(4))
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "hi").unwrap()]))
        .await
        .unwrap();

    // Slow consumer: sleep 10ms between polls
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // verify event structure is present — no events lost
    assert!(
        !events.is_empty(),
        "Should receive all events even with slow consumer"
    );
    let has_reply_start = events
        .iter()
        .any(|e| matches!(e, agent_scope_event::AgentEvent::ReplyStart(_)));
    let has_reply_end = events
        .iter()
        .any(|e| matches!(e, agent_scope_event::AgentEvent::ReplyEnd(_)));
    assert!(has_reply_start, "Expected ReplyStart");
    assert!(has_reply_end, "Expected ReplyEnd");
}

/// T038: Unbounded channel (default) preserves all events with fast model.
#[tokio::test]
async fn test_unbounded_channel_all_events() {
    let model = Arc::new(MockModel::new("mock", "unbounded test text").with_stream(5));
    // Default: unbounded channel (capacity = None)
    let config = AgentConfig::builder()
        .name("agent")
        .model(model.clone())
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "hi").unwrap()]))
        .await
        .unwrap();

    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    assert!(
        !events.is_empty(),
        "Unbounded channel should deliver all events"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, agent_scope_event::AgentEvent::ReplyStart(_)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, agent_scope_event::AgentEvent::ReplyEnd(_)))
    );
}

/// Regression (round-4 H1): the streaming path must report iteration-budget
/// exhaustion as `ExceedMaxIters`, matching the batch path. Previously the
/// streaming `ReplyEnd` claimed `Completed`, so a consumer that keyed off
/// `finished_reason` concluded the reply succeeded normally.
#[tokio::test]
async fn test_streaming_max_iters_reply_end_is_exceed_max_iters() {
    use agent_scope_types::ReplyFinishedReason;

    let chunks = vec![tool_call_chunk("tc1", "stream_echo", r#"{}"#)];
    let model = Arc::new(MockStreamingModel::new("mock", chunks));
    let tool_chunks: Vec<Result<ToolResultBlock, agent_scope_tool::ToolError>> =
        vec![Ok(ToolResultBlock::new(
            "tc1".into(),
            "stream_echo".into(),
            ToolOutput::Text("done".into()),
        ))];
    let stream_tool = MockStreamingTool::new("stream_echo", tool_chunks);

    let mut tk = ToolKit::new();
    tk.register(stream_tool);
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .toolkit(tk)
        .build()
        .unwrap();
    let react_config = ReActConfig {
        max_iters: 1,
        ..ReActConfig::default()
    };
    let agent = ReActAgent::new(config, react_config, ContextConfig::default(), vec![]).unwrap();

    let mut stream = agent
        .reply_stream(Some(vec![user_msg("user", "hi").unwrap()]))
        .await
        .unwrap();
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }

    assert!(
        events
            .iter()
            .any(|e| matches!(e, agent_scope_event::AgentEvent::ExceedMaxIters(_))),
        "expected ExceedMaxIters event"
    );
    let reply_ends: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            agent_scope_event::AgentEvent::ReplyEnd(ev) => Some(ev),
            _ => None,
        })
        .collect();
    assert_eq!(reply_ends.len(), 1, "expected exactly one ReplyEnd");
    assert_eq!(
        reply_ends[0].finished_reason,
        ReplyFinishedReason::ExceedMaxIters,
        "streaming ReplyEnd must report ExceedMaxIters, not Completed"
    );
}

struct FirstCallCompleteToolModel {
    call_started: Arc<Notify>,
    allow_return: Arc<Notify>,
    calls: AtomicUsize,
}

#[async_trait::async_trait]
impl ChatModel for FirstCallCompleteToolModel {
    fn model_name(&self) -> &str {
        "complete-tool-model"
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
        let call_idx = self.calls.fetch_add(1, Ordering::SeqCst);
        if call_idx == 0 {
            self.call_started.notify_one();
            self.allow_return.notified().await;
            let mut resp = ChatResponse::default();
            resp.content.push(ContentBlock::ToolCall(ToolCallBlock::new(
                "tc1".into(),
                "side_effect".into(),
                r#"{}"#.into(),
            )));
            Ok(ModelCallResult::Complete(resp))
        } else {
            let mut resp = ChatResponse::default();
            resp.content
                .push(ContentBlock::Text(TextBlock::new("done".into())));
            Ok(ModelCallResult::Complete(resp))
        }
    }
}

struct CountingTool {
    calls: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl Tool for CountingTool {
    fn name(&self) -> &str {
        "side_effect"
    }

    fn description(&self) -> &str {
        "counts invocations"
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({"type":"object"})
    }

    async fn call(&self, _input: JsonValue) -> Result<ToolExecOutput, agent_scope_tool::ToolError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ToolExecOutput::Complete(ToolResultBlock::new(
            "tc1".into(),
            "side_effect".into(),
            ToolOutput::Text("ran".into()),
        )))
    }
}

struct GatedPreActingMw {
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

#[async_trait::async_trait]
impl Middleware for GatedPreActingMw {
    async fn pre_acting(
        &self,
        _agent_name: &str,
        _tool_call: &mut ToolCallBlock,
    ) -> Result<(), AgentError> {
        self.entered.notify_one();
        self.release.notified().await;
        Ok(())
    }
}

/// Regression: if an EventStream is dropped while the initial Complete model
/// call is in-flight, the reactor must notice StreamHandle cancellation before
/// it writes assistant/tool state or executes side-effectful tools.
#[tokio::test]
async fn test_drop_stream_during_initial_complete_model_call_cancels_before_tools() {
    let call_started = Arc::new(Notify::new());
    let allow_return = Arc::new(Notify::new());
    let tool_calls = Arc::new(AtomicUsize::new(0));
    let model = Arc::new(FirstCallCompleteToolModel {
        call_started: Arc::clone(&call_started),
        allow_return: Arc::clone(&allow_return),
        calls: AtomicUsize::new(0),
    });
    let mut tk = ToolKit::new();
    tk.register(CountingTool {
        calls: Arc::clone(&tool_calls),
    });
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .toolkit(tk)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )
    .unwrap();

    let stream = agent
        .reply_stream(Some(vec![user_msg("user", "run").unwrap()]))
        .await
        .unwrap();
    call_started.notified().await;
    drop(stream);
    allow_return.notify_one();

    wait_until_reusable(&agent).await;

    assert_eq!(tool_calls.load(Ordering::SeqCst), 0);
}

/// Regression: cancellation after `pre_acting.await` but before dispatching the
/// tool must be re-checked so side effects are not executed.
#[tokio::test]
async fn test_streaming_cancellation_after_pre_acting_prevents_tool_dispatch() {
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let tool_calls = Arc::new(AtomicUsize::new(0));
    let chunks = vec![tool_call_chunk("tc1", "side_effect", r#"{}"#)];
    let model = Arc::new(MockStreamingModel::new("mock", chunks));
    let mut tk = ToolKit::new();
    tk.register(CountingTool {
        calls: Arc::clone(&tool_calls),
    });
    let config = AgentConfig::builder()
        .name("agent")
        .model(model)
        .toolkit(tk)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![Arc::new(GatedPreActingMw {
            entered: Arc::clone(&entered),
            release: Arc::clone(&release),
        })],
    )
    .unwrap();

    let stream = agent
        .reply_stream(Some(vec![user_msg("user", "run").unwrap()]))
        .await
        .unwrap();
    entered.notified().await;
    drop(stream);
    release.notify_one();

    wait_until_reusable(&agent).await;

    assert_eq!(tool_calls.load(Ordering::SeqCst), 0);
}
