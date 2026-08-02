//! Planner orchestration built on top of the existing Agent/ReAct flow.

#![allow(clippy::result_large_err)]

use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use agent_scope_message::Msg;
use agent_scope_message::factory::{assistant_msg, user_msg};
use agent_scope_model::{ChatModel, ModelCallResult};
use futures::Stream;
use tokio_util::sync::CancellationToken;

use crate::agent_error::AgentError;
use crate::agent_trait::Agent;
use crate::plan::{
    Plan, PlanRevision, PlanRevisionTrigger, PlanStatus, PlanStepStatus, PlannedTask,
    PlannerOutcome, parse_plan_json, validate_goal,
};
use crate::planner_error::{PlannerError, PlannerErrorCategory};
use crate::planner_stream;
use crate::planning_trace::{PlanningEvent, PlanningEventType, PlanningTrace};

/// Configuration for planned task execution.
#[derive(Debug, Clone)]
pub struct PlannerConfig {
    /// Maximum executable steps in a plan.
    pub max_steps: usize,
    /// Maximum replanning attempts for one planned task.
    pub max_replans: u32,
    /// Maximum ReAct iterations allowed per step.
    pub per_step_max_iters: u32,
    /// Optional end-to-end timeout for a planned task.
    pub timeout: Option<Duration>,
    /// Redaction policy name recorded in traces.
    pub redaction_policy: String,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            max_steps: 20,
            max_replans: 3,
            per_step_max_iters: 20,
            timeout: None,
            redaction_policy: "default-redact-secrets".into(),
        }
    }
}

impl PlannerConfig {
    /// Validate planner configuration.
    pub fn validate(&self) -> Result<(), PlannerError> {
        if self.max_steps == 0 {
            return Err(PlannerError::new(
                PlannerErrorCategory::StepLimitExceeded,
                "max_steps must be > 0",
            ));
        }
        if self.per_step_max_iters == 0 {
            return Err(PlannerError::new(
                PlannerErrorCategory::StepLimitExceeded,
                "per_step_max_iters must be > 0",
            ));
        }
        if self.redaction_policy.trim().is_empty() {
            return Err(PlannerError::new(
                PlannerErrorCategory::MalformedPlan,
                "redaction_policy must not be empty",
            ));
        }
        Ok(())
    }
}

/// Result returned by non-streaming planned task execution.
#[derive(Debug, Clone)]
pub struct PlannerRunResult {
    pub task: PlannedTask,
    pub trace: PlanningTrace,
    pub final_message: Msg,
    pub outcome: PlannerOutcome,
}

/// Planner wrapper around a ReAct-capable agent.
pub struct Planner {
    agent: Arc<dyn Agent>,
    planner_model: Arc<dyn ChatModel>,
    config: PlannerConfig,
    cancel_token: CancellationToken,
}

impl Planner {
    /// Create a new planner around an existing agent and planning model.
    pub fn new(
        agent: Arc<dyn Agent>,
        planner_model: Arc<dyn ChatModel>,
        config: PlannerConfig,
    ) -> Result<Self, PlannerError> {
        config.validate()?;
        Ok(Self {
            agent,
            planner_model,
            config,
            cancel_token: CancellationToken::new(),
        })
    }

    /// Access planner configuration.
    pub fn config(&self) -> &PlannerConfig {
        &self.config
    }

    /// Cancel the active planned task.
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    /// Return an explicit unsupported capability result.
    pub fn unsupported_capability(&self, capability: &str) -> PlannerRunResult {
        let mut task = PlannedTask::new(format!("unsupported capability: {capability}"))
            .unwrap_or_else(|_| PlannedTask {
                task_id: uuid::Uuid::new_v4().as_simple().to_string(),
                goal: capability.into(),
                plan: None,
                revisions: Vec::new(),
                outcome: None,
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
                metadata: serde_json::json!({}),
            });
        let outcome = PlannerOutcome::Unsupported {
            reason: "capability is outside Feature 021 scope".into(),
            capability: capability.into(),
        };
        task.outcome = Some(outcome.clone());
        let mut trace = PlanningTrace::new(task.task_id.clone());
        trace.redaction_policy = self.config.redaction_policy.clone();
        let _ = trace.finish(outcome.clone());
        PlannerRunResult {
            task,
            trace,
            final_message: assistant_msg("planner", "Unsupported planner capability."),
            outcome,
        }
    }

    /// Run a planned task to completion and return final trace/result.
    pub async fn run(&self, goal: impl Into<String>) -> Result<PlannerRunResult, PlannerError> {
        let goal = goal.into();
        validate_goal(&goal)?;
        if self.cancel_token.is_cancelled() {
            return Err(PlannerError::new(
                PlannerErrorCategory::Cancelled,
                "planner cancelled",
            ));
        }

        let mut task = PlannedTask::new(goal.clone())?;
        let mut trace = PlanningTrace::new(task.task_id.clone());
        trace.redaction_policy = self.config.redaction_policy.clone();
        trace.push_event(
            PlanningEventType::PlanningStarted,
            None,
            None,
            Some(goal.clone()),
        )?;

        let mut plan = self.generate_plan(&task.task_id, &goal).await?;
        self.validate_step_limit(&plan)?;
        plan.status = PlanStatus::Active;
        plan.validate()?;
        trace.push_event(
            PlanningEventType::PlanningCompleted,
            Some(plan.plan_id.clone()),
            None,
            Some(plan.objective.clone()),
        )?;
        task.plan = Some(plan.clone());

        let mut final_message = assistant_msg("planner", "");
        let mut replan_count = 0u32;
        let mut idx = 0usize;

        while idx < plan.steps.len() {
            if self.cancel_token.is_cancelled() {
                if let Some(step) = plan.steps.get_mut(idx) {
                    step.finish_with_reason(PlanStepStatus::Cancelled, "cancelled");
                    trace.push_event(
                        PlanningEventType::StepCancelled,
                        Some(plan.plan_id.clone()),
                        Some(step.step_id.clone()),
                        Some("cancelled".into()),
                    )?;
                }
                let outcome = PlannerOutcome::Cancelled {
                    reason: "cancelled".into(),
                };
                trace.finish(outcome.clone())?;
                task.plan = Some(plan);
                task.outcome = Some(outcome.clone());
                return Ok(PlannerRunResult {
                    task,
                    trace,
                    final_message: assistant_msg("planner", "The planned task was cancelled."),
                    outcome,
                });
            }

            let mut step = plan.steps[idx].clone();
            step.start()?;
            trace.push_event(
                PlanningEventType::StepStarted,
                Some(plan.plan_id.clone()),
                Some(step.step_id.clone()),
                Some(step.objective.clone()),
            )?;

            match self.execute_step(&step.objective).await {
                Ok(msg) => {
                    step.complete();
                    final_message = msg;
                    trace.push_event(
                        PlanningEventType::StepCompleted,
                        Some(plan.plan_id.clone()),
                        Some(step.step_id.clone()),
                        Some("completed".into()),
                    )?;
                    plan.steps[idx] = step;
                    idx += 1;
                }
                Err(err) if replan_count < self.config.max_replans => {
                    let planner_err = PlannerError::from(err).with_context(
                        Some(task.task_id.clone()),
                        Some(plan.plan_id.clone()),
                        Some(step.step_id.clone()),
                    );
                    step.finish_with_reason(PlanStepStatus::Failed, planner_err.message.clone());
                    trace.push(
                        PlanningEvent::new(PlanningEventType::StepFailed, task.task_id.clone())
                            .with_plan(plan.plan_id.clone())
                            .with_step(step.step_id.clone())
                            .with_error(planner_err.clone()),
                    )?;
                    plan.steps[idx] = step.clone();
                    trace.push_event(
                        PlanningEventType::ReplanningStarted,
                        Some(plan.plan_id.clone()),
                        Some(step.step_id.clone()),
                        Some(planner_err.message.clone()),
                    )?;

                    let mut new_plan = self
                        .generate_plan(
                            &task.task_id,
                            &format!(
                                "Replan after failed step '{}': {}",
                                step.objective, planner_err.message
                            ),
                        )
                        .await?;
                    self.validate_step_limit(&new_plan)?;
                    new_plan.version = plan.version + 1;
                    new_plan.status = PlanStatus::Active;
                    let revision = self.create_revision(
                        &task.task_id,
                        &plan.plan_id,
                        &new_plan.plan_id,
                        PlanRevisionTrigger::RecoverableFailure,
                        &planner_err.message,
                    )?;
                    task.revisions.push(revision);
                    plan.status = PlanStatus::Revised;
                    trace.push_event(
                        PlanningEventType::ReplanningCompleted,
                        Some(new_plan.plan_id.clone()),
                        None,
                        Some(new_plan.objective.clone()),
                    )?;
                    task.plan = Some(new_plan.clone());
                    plan = new_plan;
                    idx = 0;
                    replan_count += 1;
                }
                Err(err) => {
                    let planner_err = PlannerError::from(err).with_context(
                        Some(task.task_id.clone()),
                        Some(plan.plan_id.clone()),
                        Some(step.step_id.clone()),
                    );
                    step.finish_with_reason(PlanStepStatus::Failed, planner_err.message.clone());
                    trace.push(
                        PlanningEvent::new(PlanningEventType::StepFailed, task.task_id.clone())
                            .with_plan(plan.plan_id.clone())
                            .with_step(step.step_id.clone())
                            .with_error(planner_err.clone()),
                    )?;
                    plan.steps[idx] = step;
                    let (reason, category) = if replan_count >= self.config.max_replans {
                        (
                            format!("replanning limit exceeded: {}", planner_err.message),
                            PlannerErrorCategory::ReplanLimitExceeded.to_string(),
                        )
                    } else {
                        (
                            planner_err.message.clone(),
                            planner_err.category.to_string(),
                        )
                    };
                    let outcome = if plan
                        .steps
                        .iter()
                        .any(|s| s.status == PlanStepStatus::Completed)
                    {
                        PlannerOutcome::PartiallyCompleted {
                            summary: "planned task partially completed".into(),
                            reason: reason.clone(),
                        }
                    } else {
                        PlannerOutcome::Failed {
                            reason: reason.clone(),
                            category,
                        }
                    };
                    trace.finish(outcome.clone())?;
                    task.plan = Some(plan);
                    task.outcome = Some(outcome.clone());
                    return Ok(PlannerRunResult {
                        task,
                        trace,
                        final_message: assistant_msg("planner", &reason),
                        outcome,
                    });
                }
            }
        }

        plan.status = PlanStatus::Completed;
        task.plan = Some(plan);
        let outcome = PlannerOutcome::Completed {
            summary: "planned task completed".into(),
        };
        trace.finish(outcome.clone())?;
        task.outcome = Some(outcome.clone());
        Ok(PlannerRunResult {
            task,
            trace,
            final_message,
            outcome,
        })
    }

    fn validate_step_limit(&self, plan: &Plan) -> Result<(), PlannerError> {
        if plan.steps.len() > self.config.max_steps {
            return Err(PlannerError::new(
                PlannerErrorCategory::StepLimitExceeded,
                format!(
                    "plan contains {} steps, max is {}",
                    plan.steps.len(),
                    self.config.max_steps
                ),
            ));
        }
        Ok(())
    }

    /// Run a planned task and expose a stream of planner events.
    pub async fn run_stream(
        &self,
        goal: impl Into<String>,
    ) -> Result<Pin<Box<dyn Stream<Item = agent_scope_event::AgentEvent> + Send>>, PlannerError>
    {
        let result = self.run(goal).await?;
        Ok(planner_stream::trace_to_stream(result.trace))
    }

    async fn generate_plan(&self, task_id: &str, goal: &str) -> Result<Plan, PlannerError> {
        let msg = user_msg("user", goal).map_err(|e| {
            PlannerError::new(
                PlannerErrorCategory::InvalidGoal,
                format!("failed to create planning message: {e:?}"),
            )
        })?;
        let result = self
            .planner_model
            .call(&[msg], None, None)
            .await
            .map_err(|e| {
                PlannerError::new(PlannerErrorCategory::PlanGenerationFailed, e.to_string())
            })?;
        let raw = match result {
            ModelCallResult::Complete(resp) => resp.get_text_content("\n"),
            ModelCallResult::Stream(_) => {
                return Err(PlannerError::new(
                    PlannerErrorCategory::UnsupportedCapability,
                    "streaming plan generation is not supported in Feature 021",
                ));
            }
        };
        parse_plan_json(task_id.to_string(), &raw)
    }

    async fn execute_step(&self, objective: &str) -> Result<Msg, AgentError> {
        let msg = user_msg("planner", objective).map_err(|e| AgentError::ValidationError {
            message: format!("invalid step objective: {e:?}"),
        })?;
        self.agent.reply(Some(vec![msg])).await
    }

    /// Create an explicit revision record and preserve superseded plan metadata.
    pub fn create_revision(
        &self,
        task_id: &str,
        from_plan_id: &str,
        to_plan_id: &str,
        trigger: PlanRevisionTrigger,
        rationale: &str,
    ) -> Result<PlanRevision, PlannerError> {
        let revision = PlanRevision::new(task_id, from_plan_id, to_plan_id, trigger, rationale);
        revision.validate()?;
        Ok(revision)
    }
}
