mod planner_mocks;

use std::pin::Pin;
use std::sync::{Arc, RwLock};

use agent_scope_agent::{
    Agent, AgentError, Planner, PlannerConfig, PlannerOutcome, PlanningEventType,
};
use agent_scope_event::AgentEvent;
use agent_scope_message::Msg;
use agent_scope_message::factory::assistant_msg;
use agent_scope_state::AgentState;
use futures::{Stream, stream};
use planner_mocks::PlannerScriptedModel;

struct EchoAgent {
    state: AgentState,
    seen: RwLock<Vec<String>>,
}

impl EchoAgent {
    fn new() -> Self {
        Self {
            state: AgentState::new(),
            seen: RwLock::new(Vec::new()),
        }
    }

    fn seen_count(&self) -> usize {
        self.seen.read().unwrap().len()
    }
}

#[async_trait::async_trait]
impl Agent for EchoAgent {
    async fn reply(&self, input: Option<Vec<Msg>>) -> Result<Msg, AgentError> {
        let text = input
            .unwrap_or_default()
            .iter()
            .filter_map(|msg| msg.get_text_content(" "))
            .collect::<Vec<_>>()
            .join(" | ");
        self.seen.write().unwrap().push(text.clone());
        Ok(assistant_msg("echo", &format!("done: {text}")))
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
        "echo"
    }

    fn state(&self) -> &AgentState {
        &self.state
    }
}

#[tokio::test]
async fn successful_planned_task_executes_steps_in_order() {
    let agent = Arc::new(EchoAgent::new());
    let planner_model = Arc::new(PlannerScriptedModel::new(
        "planner",
        vec![r#"{"objective":"Compare docs","steps":["Read A","Read B","Summarize"]}"#.into()],
    ));
    let planner = Planner::new(agent.clone(), planner_model, PlannerConfig::default()).unwrap();

    let result = planner.run("compare two documents").await.unwrap();

    assert!(matches!(result.outcome, PlannerOutcome::Completed { .. }));
    assert_eq!(agent.seen_count(), 3);
    assert_eq!(result.task.plan.as_ref().unwrap().steps.len(), 3);
    assert!(
        result
            .trace
            .events
            .iter()
            .any(|event| event.event_type == PlanningEventType::PlanningStarted)
    );
    assert!(
        result
            .trace
            .events
            .iter()
            .any(|event| event.event_type == PlanningEventType::TaskCompleted)
    );
    assert!(
        result
            .final_message
            .get_text_content(" ")
            .unwrap()
            .contains("Summarize")
    );
}

#[tokio::test]
async fn non_streaming_planner_rejects_empty_goal() {
    let agent = Arc::new(EchoAgent::new());
    let planner_model = Arc::new(PlannerScriptedModel::new("planner", vec![]));
    let planner = Planner::new(agent, planner_model, PlannerConfig::default()).unwrap();

    let err = planner.run("   ").await.unwrap_err();
    assert!(err.to_string().contains("goal"));
}

#[tokio::test]
async fn max_step_limit_is_enforced() {
    let agent = Arc::new(EchoAgent::new());
    let planner_model = Arc::new(PlannerScriptedModel::new(
        "planner",
        vec![r#"{"objective":"Too much","steps":["one","two"]}"#.into()],
    ));
    let planner = Planner::new(
        agent,
        planner_model,
        PlannerConfig {
            max_steps: 1,
            ..Default::default()
        },
    )
    .unwrap();

    let err = planner.run("do too much").await.unwrap_err();
    assert!(err.to_string().contains("max"));
}

#[tokio::test]
async fn unsupported_capability_returns_explicit_outcome() {
    let agent = Arc::new(EchoAgent::new());
    let planner_model = Arc::new(PlannerScriptedModel::new("planner", vec![]));
    let planner = Planner::new(agent, planner_model, PlannerConfig::default()).unwrap();

    let result = planner.unsupported_capability("parallel DAG execution");
    assert!(matches!(result.outcome, PlannerOutcome::Unsupported { .. }));
    assert!(
        result
            .trace
            .events
            .iter()
            .any(|event| event.event_type == PlanningEventType::TaskUnsupported)
    );
}
