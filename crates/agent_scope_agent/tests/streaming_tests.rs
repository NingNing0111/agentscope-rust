//! Streaming and compression integration tests.

use std::sync::Arc;

use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_message::Role;
use agent_scope_message::factory::user_msg;
use agent_scope_model::ChatModel;

mod mocks;
use mocks::MockModel;

// ---------------------------------------------------------------------------
// Streaming tests
// ---------------------------------------------------------------------------

/// MockModel with stream_mode returns a stream that accumulates correctly.
#[tokio::test]
async fn test_streaming_mock_model_produces_correct_text() {
    let model = Arc::new(MockModel::new("mock", "Hello, streaming world!").with_stream(3));

    // Call through the public ChatModel::call() entry point
    let result = ChatModel::call(model.as_ref(), &[], None, None)
        .await
        .unwrap();

    // Should be a stream
    if let agent_scope_model::ModelCallResult::Stream(mut stream) = result {
        use agent_scope_model::StreamAccumulator;
        use futures::StreamExt;

        let mut acc = StreamAccumulator::new();
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.unwrap();
            acc.append_chat_response(&chunk);
        }
        let built = acc.build();
        assert_eq!(built.get_text_content(""), "Hello, streaming world!");
    } else {
        panic!("Expected Stream variant");
    }
}

/// ReActAgent works with a streaming model (DashScope compatibility).
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

    let input = user_msg("user", "hello").unwrap();
    let reply = agent.reply(Some(vec![input])).await.unwrap();

    assert_eq!(reply.role, Role::Assistant);
    assert_eq!(reply.get_text_content("").unwrap(), "streaming response");
}

// ---------------------------------------------------------------------------
// Compression tests
// ---------------------------------------------------------------------------

/// Compression enabled but context is small — should not trigger.
#[tokio::test]
async fn test_compression_not_triggered_for_small_context() {
    // Use a model with a generous context size and only one small message
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

    let input = user_msg("user", "hi").unwrap();
    let reply = agent.reply(Some(vec![input])).await.unwrap();

    // Should complete normally — compression is a no-op for small contexts
    assert_eq!(reply.role, Role::Assistant);
    assert_eq!(reply.get_text_content("").unwrap(), "ok");
}

/// Compression disabled — no compression even with large context.
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
            enable: false, // explicitly disabled
            ..Default::default()
        },
        vec![],
    )
    .unwrap();

    let input = user_msg("user", "hi").unwrap();
    let reply = agent.reply(Some(vec![input])).await.unwrap();

    assert_eq!(reply.role, Role::Assistant);
    assert_eq!(reply.get_text_content("").unwrap(), "ok");
}

/// Streaming + compression both enabled — should work together.
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
            trigger_ratio: 0.9, // high threshold so compression won't actually trigger
            ..Default::default()
        },
        vec![],
    )
    .unwrap();

    let input = user_msg("user", "hello streaming world").unwrap();
    let reply = agent.reply(Some(vec![input])).await.unwrap();

    assert_eq!(reply.role, Role::Assistant);
    assert_eq!(
        reply.get_text_content("").unwrap(),
        "stream + compress works"
    );
}

/// Streaming mock model with a single chunk behaves like Complete.
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

    let input = user_msg("user", "hi").unwrap();
    let reply = agent.reply(Some(vec![input])).await.unwrap();

    assert_eq!(reply.get_text_content("").unwrap(), "single chunk");
}
