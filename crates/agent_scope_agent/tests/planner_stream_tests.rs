mod planner_mocks;

use std::pin::Pin;
use std::sync::Arc;

use agent_scope_agent::{Agent, AgentError, Planner, PlannerConfig};
use agent_scope_event::AgentEvent;
use agent_scope_message::Msg;
use agent_scope_message::factory::assistant_msg;
use agent_scope_state::AgentState;
use futures::{Stream, StreamExt, stream};
use planner_mocks::PlannerScriptedModel;

struct StreamEchoAgent {
    state: AgentState,
}

#[async_trait::async_trait]
impl Agent for StreamEchoAgent {
    async fn reply(&self, input: Option<Vec<Msg>>) -> Result<Msg, AgentError> {
        let text = input
            .unwrap_or_default()
            .iter()
            .filter_map(|msg| msg.get_text_content(" "))
            .collect::<Vec<_>>()
            .join(" | ");
        Ok(assistant_msg("echo", &text))
    }

    async fn reply_stream(
        &self,
        _input: Option<Vec<Msg>>,
    ) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError> {
        Ok(Box::pin(stream::empty()))
    }

    async fn observe(&self, _input: Option<Vec<Msg>>) -> Result<(), AgentError> {
        Ok(())
    }

    fn name(&self) -> &str {
        "stream-echo"
    }

    fn state(&self) -> &AgentState {
        &self.state
    }
}

#[tokio::test]
async fn run_stream_yields_planner_lifecycle_custom_events() {
    let agent = Arc::new(StreamEchoAgent {
        state: AgentState::new(),
    });
    let planner_model = Arc::new(PlannerScriptedModel::new(
        "planner",
        vec![r#"{"objective":"Stream","steps":["one"]}"#.into()],
    ));
    let planner = Planner::new(agent, planner_model, PlannerConfig::default()).unwrap();

    let mut stream = planner.run_stream("stream a plan").await.unwrap();
    let mut names = Vec::new();
    while let Some(event) = stream.next().await {
        if let AgentEvent::Custom(custom) = event {
            names.push(custom.name);
        }
    }

    assert!(!names.is_empty());
    assert!(names.iter().all(|name| name == "planner.lifecycle"));
}
