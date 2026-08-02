mod planner_mocks;

use std::pin::Pin;
use std::sync::{Arc, Mutex};

use agent_scope_agent::{
    Agent, AgentError, Planner, PlannerConfig, PlannerOutcome, PlanningEventType,
};
use agent_scope_event::AgentEvent;
use agent_scope_message::Msg;
use agent_scope_message::factory::assistant_msg;
use agent_scope_state::AgentState;
use futures::{Stream, stream};
use planner_mocks::PlannerScriptedModel;

struct FailingThenEchoAgent {
    state: AgentState,
    calls: Mutex<usize>,
}

impl FailingThenEchoAgent {
    fn new() -> Self {
        Self {
            state: AgentState::new(),
            calls: Mutex::new(0),
        }
    }
}

#[async_trait::async_trait]
impl Agent for FailingThenEchoAgent {
    async fn reply(&self, input: Option<Vec<Msg>>) -> Result<Msg, AgentError> {
        let mut calls = self.calls.lock().unwrap();
        let idx = *calls;
        *calls += 1;
        if idx == 0 {
            return Err(AgentError::ValidationError {
                message: "recoverable tool failure".into(),
            });
        }
        let text = input
            .unwrap_or_default()
            .iter()
            .filter_map(|msg| msg.get_text_content(" "))
            .collect::<Vec<_>>()
            .join(" | ");
        Ok(assistant_msg("echo", &format!("recovered: {text}")))
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
        "failing-then-echo"
    }

    fn state(&self) -> &AgentState {
        &self.state
    }
}

#[tokio::test]
async fn planner_replans_after_recoverable_failure() {
    let agent = Arc::new(FailingThenEchoAgent::new());
    let planner_model = Arc::new(PlannerScriptedModel::new(
        "planner",
        vec![
            r#"{"objective":"Initial","steps":["failing step"]}"#.into(),
            r#"{"objective":"Revised","steps":["alternate step"]}"#.into(),
        ],
    ));
    let planner = Planner::new(agent, planner_model, PlannerConfig::default()).unwrap();

    let result = planner.run("recover from failure").await.unwrap();

    assert!(matches!(result.outcome, PlannerOutcome::Completed { .. }));
    assert_eq!(result.task.revisions.len(), 1);
    assert!(
        result
            .trace
            .events
            .iter()
            .any(|event| event.event_type == PlanningEventType::StepFailed)
    );
    assert!(
        result
            .trace
            .events
            .iter()
            .any(|event| event.event_type == PlanningEventType::ReplanningStarted)
    );
    assert!(
        result
            .trace
            .events
            .iter()
            .any(|event| event.event_type == PlanningEventType::ReplanningCompleted)
    );
}

#[tokio::test]
async fn planner_stops_when_replanning_limit_exceeded() {
    let agent = Arc::new(FailingThenEchoAgent::new());
    let planner_model = Arc::new(PlannerScriptedModel::new(
        "planner",
        vec![r#"{"objective":"Initial","steps":["failing step"]}"#.into()],
    ));
    let planner = Planner::new(
        agent,
        planner_model,
        PlannerConfig {
            max_replans: 0,
            ..Default::default()
        },
    )
    .unwrap();

    let result = planner.run("cannot recover").await.unwrap();
    assert!(matches!(
        result.outcome,
        PlannerOutcome::Failed { .. } | PlannerOutcome::PartiallyCompleted { .. }
    ));
    assert!(
        result
            .trace
            .events
            .iter()
            .any(|event| event.event_type == PlanningEventType::TaskFailed)
    );
}

#[tokio::test]
async fn planner_preserves_obsolete_step_skipped_reason_after_replan() {
    let agent = Arc::new(FailingThenEchoAgent::new());
    let planner_model = Arc::new(PlannerScriptedModel::new(
        "planner",
        vec![
            r#"{"objective":"Initial","steps":["failing obsolete step","still pending obsolete step"]}"#.into(),
            r#"{"objective":"Revised","steps":["replacement step"]}"#.into(),
        ],
    ));
    let planner = Planner::new(agent, planner_model, PlannerConfig::default()).unwrap();

    let result = planner
        .run("skip obsolete work after replanning")
        .await
        .unwrap();

    assert!(matches!(result.outcome, PlannerOutcome::Completed { .. }));
    assert_eq!(result.task.revisions.len(), 1);
    let revision = &result.task.revisions[0];
    assert_eq!(
        revision.trigger,
        agent_scope_agent::PlanRevisionTrigger::RecoverableFailure
    );
    assert!(revision.rationale.contains("recoverable tool failure"));

    // Feature 021 records obsolete work by superseding the old plan. The replacement
    // plan should contain only the still-actionable step; the pending obsolete step
    // is skipped with rationale by omission from the revised plan and revision record.
    let final_plan = result.task.plan.as_ref().unwrap();
    assert_eq!(final_plan.version, 2);
    assert_eq!(final_plan.steps.len(), 1);
    assert_eq!(final_plan.steps[0].objective, "replacement step");
    assert!(
        !final_plan
            .steps
            .iter()
            .any(|step| step.objective == "still pending obsolete step")
    );
    assert!(result.trace.events.iter().any(|event| {
        event.event_type == PlanningEventType::ReplanningCompleted
            && event
                .summary
                .as_deref()
                .is_some_and(|summary| summary.contains("Revised"))
    }));
}
