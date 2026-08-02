mod planner_mocks;

use std::fs;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use agent_scope_agent::{Agent, AgentError, Planner, PlannerConfig, PlannerOutcome};
use agent_scope_event::AgentEvent;
use agent_scope_message::Msg;
use agent_scope_message::factory::assistant_msg;
use agent_scope_state::AgentState;
use futures::{Stream, stream};
use planner_mocks::PlannerScriptedModel;
use serde_json::Value;

struct CompatEchoAgent {
    state: AgentState,
    fail_first: bool,
    calls: Mutex<usize>,
}

impl CompatEchoAgent {
    fn new(fail_first: bool) -> Self {
        Self {
            state: AgentState::new(),
            fail_first,
            calls: Mutex::new(0),
        }
    }
}

#[async_trait::async_trait]
impl Agent for CompatEchoAgent {
    async fn reply(&self, input: Option<Vec<Msg>>) -> Result<Msg, AgentError> {
        let mut calls = self.calls.lock().unwrap();
        let idx = *calls;
        *calls += 1;
        if self.fail_first && idx == 0 {
            return Err(AgentError::ValidationError {
                message: "recoverable compatibility failure".into(),
            });
        }
        let text = input
            .unwrap_or_default()
            .iter()
            .filter_map(|msg| msg.get_text_content(" "))
            .collect::<Vec<_>>()
            .join(" | ");
        Ok(assistant_msg("compat", &format!("done: {text}")))
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
        "compat"
    }

    fn state(&self) -> &AgentState {
        &self.state
    }
}

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/compatibility/fixtures")
        .join(name);
    let raw = fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {path:?}: {err}"));
    serde_json::from_str(&raw).unwrap()
}

fn event_names(value: &Value) -> Vec<String> {
    value["data"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event_type"].as_str().unwrap().to_string())
        .collect()
}

fn rust_event_names(events: &[agent_scope_agent::PlanningEvent]) -> Vec<String> {
    events
        .iter()
        .map(|event| {
            serde_json::to_value(event.event_type)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect()
}

#[tokio::test]
async fn normalized_success_trace_matches_python_fixture_shape() {
    let fixture = fixture("planner_success_trace.json");
    let planner_model = Arc::new(PlannerScriptedModel::new(
        "planner",
        vec![r#"{"objective":"Compat","steps":["answer directly"]}"#.into()],
    ));
    let planner = Planner::new(
        Arc::new(CompatEchoAgent::new(false)),
        planner_model,
        PlannerConfig::default(),
    )
    .unwrap();

    let result = planner.run("compat success").await.unwrap();

    assert!(matches!(result.outcome, PlannerOutcome::Completed { .. }));
    assert_eq!(
        rust_event_names(&result.trace.events),
        event_names(&fixture)
    );
}

#[tokio::test]
async fn normalized_replanning_trace_matches_python_fixture_order() {
    let fixture = fixture("planner_replanning_trace.json");
    let planner_model = Arc::new(PlannerScriptedModel::new(
        "planner",
        vec![
            r#"{"objective":"Initial","steps":["fail first"]}"#.into(),
            r#"{"objective":"Revised","steps":["recover"]}"#.into(),
        ],
    ));
    let planner = Planner::new(
        Arc::new(CompatEchoAgent::new(true)),
        planner_model,
        PlannerConfig::default(),
    )
    .unwrap();

    let result = planner.run("compat replan").await.unwrap();

    assert!(matches!(result.outcome, PlannerOutcome::Completed { .. }));
    assert_eq!(result.task.revisions.len(), 1);
    assert_eq!(
        rust_event_names(&result.trace.events),
        event_names(&fixture)
    );
}

#[tokio::test]
async fn compatibility_fixtures_cover_tool_cancellation_and_unsupported_scenarios() {
    let tool = fixture("planner_tool_step_trace.json");
    assert!(event_names(&tool).contains(&"tool_call_start".to_string()));
    assert_eq!(tool["data"]["tool_activity"][0]["tool_name"], "calculator");

    let cancellation = fixture("planner_cancellation_trace.json");
    assert!(event_names(&cancellation).contains(&"task_cancelled".to_string()));

    let unsupported = fixture("planner_unsupported_trace.json");
    assert!(event_names(&unsupported).contains(&"task_unsupported".to_string()));
    assert_eq!(
        unsupported["data"]["final_outcome"]["unsupported"]["capability"],
        "parallel DAG scheduling"
    );
}
