mod planner_mocks;

use std::pin::Pin;
use std::sync::Arc;

use agent_scope_agent::{Agent, AgentError, Planner, PlannerConfig, PlannerErrorCategory};
use agent_scope_event::AgentEvent;
use agent_scope_message::Msg;
use agent_scope_message::factory::assistant_msg;
use agent_scope_state::AgentState;
use futures::{Stream, stream};
use planner_mocks::PlannerScriptedModel;

struct CancelEchoAgent {
    state: AgentState,
}

#[async_trait::async_trait]
impl Agent for CancelEchoAgent {
    async fn reply(&self, _input: Option<Vec<Msg>>) -> Result<Msg, AgentError> {
        Ok(assistant_msg("cancel-echo", "should not run"))
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
        "cancel-echo"
    }

    fn state(&self) -> &AgentState {
        &self.state
    }
}

fn cancelled_planner() -> Planner {
    let agent = Arc::new(CancelEchoAgent {
        state: AgentState::new(),
    });
    let planner_model = Arc::new(PlannerScriptedModel::new(
        "planner",
        vec![r#"{"objective":"Cancel","steps":["one"]}"#.into()],
    ));
    let planner = Planner::new(agent, planner_model, PlannerConfig::default()).unwrap();
    planner.cancel();
    planner
}

#[tokio::test]
async fn cancellation_before_planning_returns_cancelled_error() {
    let planner = cancelled_planner();

    let err = planner.run("cancel before plan").await.unwrap_err();

    assert_eq!(err.category, PlannerErrorCategory::Cancelled);
    assert!(err.message.contains("cancelled"));
}

#[tokio::test]
async fn cancellation_before_stream_planning_returns_cancelled_error() {
    let planner = cancelled_planner();

    let err = match planner.run_stream("cancel streaming before plan").await {
        Err(err) => err,
        Ok(_) => panic!("expected cancelled planner stream error"),
    };

    assert_eq!(err.category, PlannerErrorCategory::Cancelled);
}

#[tokio::test]
async fn cancellation_state_is_stable_for_replanning_boundary() {
    let planner = cancelled_planner();

    let first = planner
        .run("cancel before step execution")
        .await
        .unwrap_err();
    let second = planner.run("cancel before replanning").await.unwrap_err();

    assert_eq!(first.category, PlannerErrorCategory::Cancelled);
    assert_eq!(second.category, PlannerErrorCategory::Cancelled);
}
