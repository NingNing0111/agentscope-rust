//! Plan data model for Planner + ReActAgent orchestration.

#![allow(clippy::result_large_err)]

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::planner_error::{PlannerError, PlannerErrorCategory};

/// Lifecycle status of a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Draft,
    Active,
    Revised,
    Completed,
    Failed,
    Cancelled,
    Unsupported,
}

/// Lifecycle status of a plan step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepStatus {
    Pending,
    Running,
    Completed,
    Skipped,
    Failed,
    Cancelled,
    Unsupported,
}

impl PlanStepStatus {
    /// Returns true when this status is terminal.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Skipped | Self::Failed | Self::Cancelled | Self::Unsupported
        )
    }
}

/// Reason a plan revision was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanRevisionTrigger {
    RecoverableFailure,
    NewInformation,
    ObsoleteStep,
    LimitReached,
    UserCancellation,
    UnsupportedCapability,
}

/// Final outcome of a planned task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlannerOutcome {
    Completed { summary: String },
    PartiallyCompleted { summary: String, reason: String },
    Cancelled { reason: String },
    Failed { reason: String, category: String },
    Unsupported { reason: String, capability: String },
}

/// Redacted summary of tool activity associated with a step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolActivityRecord {
    pub tool_name: String,
    pub call_id: Option<String>,
    #[serde(default)]
    pub arguments: serde_json::Value,
    #[serde(default)]
    pub result_summary: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// A single actionable step within a plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanStep {
    pub step_id: String,
    pub plan_id: String,
    pub index: usize,
    pub objective: String,
    pub status: PlanStepStatus,
    pub attempt_count: u32,
    pub requires_react_execution: bool,
    #[serde(default)]
    pub tool_activity: Vec<ToolActivityRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl PlanStep {
    /// Create a pending step.
    pub fn new(
        step_id: impl Into<String>,
        plan_id: impl Into<String>,
        index: usize,
        objective: impl Into<String>,
    ) -> Self {
        Self {
            step_id: step_id.into(),
            plan_id: plan_id.into(),
            index,
            objective: objective.into(),
            status: PlanStepStatus::Pending,
            attempt_count: 0,
            requires_react_execution: true,
            tool_activity: Vec::new(),
            reason: None,
            started_at: None,
            completed_at: None,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Validate the step shape.
    pub fn validate(&self) -> Result<(), PlannerError> {
        if self.step_id.trim().is_empty() {
            return Err(PlannerError::new(
                PlannerErrorCategory::MalformedPlan,
                "step_id must not be empty",
            ));
        }
        if self.plan_id.trim().is_empty() {
            return Err(PlannerError::new(
                PlannerErrorCategory::MalformedPlan,
                "step plan_id must not be empty",
            ));
        }
        if self.objective.trim().is_empty() {
            return Err(PlannerError::new(
                PlannerErrorCategory::NonActionablePlan,
                "step objective must not be empty",
            ));
        }
        if self.status.is_terminal()
            && !matches!(self.status, PlanStepStatus::Completed)
            && self.reason.as_deref().unwrap_or_default().trim().is_empty()
        {
            return Err(PlannerError::new(
                PlannerErrorCategory::MalformedPlan,
                "terminal non-successful step must include a reason",
            ));
        }
        Ok(())
    }

    /// Mark the step as running.
    pub fn start(&mut self) -> Result<(), PlannerError> {
        if self.status.is_terminal() {
            return Err(PlannerError::new(
                PlannerErrorCategory::MalformedPlan,
                "terminal step cannot be restarted",
            ));
        }
        self.status = PlanStepStatus::Running;
        self.attempt_count = self.attempt_count.saturating_add(1);
        self.started_at = Some(chrono::Utc::now().to_rfc3339());
        Ok(())
    }

    /// Mark the step as completed.
    pub fn complete(&mut self) {
        self.status = PlanStepStatus::Completed;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.reason = None;
    }

    /// Mark the step with a terminal non-success status.
    pub fn finish_with_reason(&mut self, status: PlanStepStatus, reason: impl Into<String>) {
        debug_assert!(status.is_terminal());
        self.status = status;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.reason = Some(reason.into());
    }
}

/// An ordered plan for a task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub plan_id: String,
    pub task_id: String,
    pub version: u32,
    pub objective: String,
    pub steps: Vec<PlanStep>,
    pub status: PlanStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_reason: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl Plan {
    /// Create a new draft plan from step objectives.
    pub fn new(
        plan_id: impl Into<String>,
        task_id: impl Into<String>,
        objective: impl Into<String>,
        step_objectives: Vec<String>,
    ) -> Self {
        let plan_id = plan_id.into();
        let task_id = task_id.into();
        let steps = step_objectives
            .into_iter()
            .enumerate()
            .map(|(index, objective)| {
                PlanStep::new(
                    format!("step-{}", index + 1),
                    plan_id.clone(),
                    index,
                    objective,
                )
            })
            .collect();
        Self {
            plan_id,
            task_id,
            version: 1,
            objective: objective.into(),
            steps,
            status: PlanStatus::Draft,
            created_reason: Some("initial planning".into()),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Validate plan consistency.
    pub fn validate(&self) -> Result<(), PlannerError> {
        if self.plan_id.trim().is_empty() || self.task_id.trim().is_empty() {
            return Err(PlannerError::new(
                PlannerErrorCategory::MalformedPlan,
                "plan_id and task_id must not be empty",
            ));
        }
        if self.version == 0 {
            return Err(PlannerError::new(
                PlannerErrorCategory::MalformedPlan,
                "plan version must be >= 1",
            ));
        }
        if self.objective.trim().is_empty() {
            return Err(PlannerError::new(
                PlannerErrorCategory::NonActionablePlan,
                "plan objective must not be empty",
            ));
        }
        if self.steps.is_empty() {
            return Err(PlannerError::new(
                PlannerErrorCategory::NonActionablePlan,
                "plan must contain at least one step",
            ));
        }

        let mut ids = HashSet::new();
        for step in &self.steps {
            step.validate()?;
            if step.plan_id != self.plan_id {
                return Err(PlannerError::new(
                    PlannerErrorCategory::MalformedPlan,
                    "step plan_id must match parent plan_id",
                ));
            }
            if !ids.insert(step.step_id.clone()) {
                return Err(PlannerError::new(
                    PlannerErrorCategory::MalformedPlan,
                    "step IDs must be unique within a plan",
                ));
            }
        }
        Ok(())
    }

    /// Returns true when every step has a terminal status.
    pub fn all_steps_terminal(&self) -> bool {
        self.steps.iter().all(|step| step.status.is_terminal())
    }
}

/// Explicit record of a plan revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanRevision {
    pub revision_id: String,
    pub task_id: String,
    pub from_plan_id: String,
    pub to_plan_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_step_id: Option<String>,
    pub trigger: PlanRevisionTrigger,
    pub rationale: String,
    pub created_at: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl PlanRevision {
    /// Create a new revision record.
    pub fn new(
        task_id: impl Into<String>,
        from_plan_id: impl Into<String>,
        to_plan_id: impl Into<String>,
        trigger: PlanRevisionTrigger,
        rationale: impl Into<String>,
    ) -> Self {
        Self {
            revision_id: uuid::Uuid::new_v4().as_simple().to_string(),
            task_id: task_id.into(),
            from_plan_id: from_plan_id.into(),
            to_plan_id: to_plan_id.into(),
            trigger_step_id: None,
            trigger,
            rationale: rationale.into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Validate revision consistency.
    pub fn validate(&self) -> Result<(), PlannerError> {
        if self.revision_id.trim().is_empty()
            || self.task_id.trim().is_empty()
            || self.from_plan_id.trim().is_empty()
            || self.to_plan_id.trim().is_empty()
        {
            return Err(PlannerError::new(
                PlannerErrorCategory::MalformedPlan,
                "revision IDs must not be empty",
            ));
        }
        if self.from_plan_id == self.to_plan_id {
            return Err(PlannerError::new(
                PlannerErrorCategory::MalformedPlan,
                "revision must point to a different replacement plan",
            ));
        }
        if self.rationale.trim().is_empty() {
            return Err(PlannerError::new(
                PlannerErrorCategory::MalformedPlan,
                "revision rationale must not be empty",
            ));
        }
        Ok(())
    }
}

/// State container for a planned task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannedTask {
    pub task_id: String,
    pub goal: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<Plan>,
    #[serde(default)]
    pub revisions: Vec<PlanRevision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<PlannerOutcome>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl PlannedTask {
    /// Create a new planned task with no plan yet.
    pub fn new(goal: impl Into<String>) -> Result<Self, PlannerError> {
        let goal = goal.into();
        validate_goal(&goal)?;
        let now = chrono::Utc::now().to_rfc3339();
        Ok(Self {
            task_id: uuid::Uuid::new_v4().as_simple().to_string(),
            goal,
            plan: None,
            revisions: Vec::new(),
            outcome: None,
            created_at: now.clone(),
            updated_at: now,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
        })
    }

    /// Validate task shape.
    pub fn validate(&self) -> Result<(), PlannerError> {
        if self.task_id.trim().is_empty() {
            return Err(PlannerError::new(
                PlannerErrorCategory::MalformedPlan,
                "task_id must not be empty",
            ));
        }
        validate_goal(&self.goal)?;
        if let Some(plan) = &self.plan {
            plan.validate()?;
        }
        for revision in &self.revisions {
            revision.validate()?;
        }
        Ok(())
    }
}

/// Validate a planner goal.
pub fn validate_goal(goal: &str) -> Result<(), PlannerError> {
    if goal.trim().is_empty() {
        return Err(PlannerError::new(
            PlannerErrorCategory::InvalidGoal,
            "goal must not be empty",
        ));
    }
    Ok(())
}

/// Parse a deterministic JSON plan response.
///
/// Accepted shape:
/// `{ "objective": "...", "steps": ["step one", {"objective":"step two"}] }`.
pub fn parse_plan_json(task_id: impl Into<String>, raw: &str) -> Result<Plan, PlannerError> {
    let task_id = task_id.into();
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|e| {
        PlannerError::new(
            PlannerErrorCategory::MalformedPlan,
            format!("failed to parse plan JSON: {e}"),
        )
    })?;
    let objective = value
        .get("objective")
        .and_then(|v| v.as_str())
        .unwrap_or("planned task")
        .to_string();
    let steps_value = value
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            PlannerError::new(
                PlannerErrorCategory::NonActionablePlan,
                "plan JSON must contain a steps array",
            )
        })?;
    let mut steps = Vec::new();
    for step in steps_value {
        if let Some(text) = step.as_str() {
            steps.push(text.to_string());
        } else if let Some(text) = step.get("objective").and_then(|v| v.as_str()) {
            steps.push(text.to_string());
        } else {
            return Err(PlannerError::new(
                PlannerErrorCategory::MalformedPlan,
                "each plan step must be a string or object with objective",
            ));
        }
    }
    let plan = Plan::new(
        format!("plan-{}", uuid::Uuid::new_v4().as_simple()),
        task_id,
        objective,
        steps,
    );
    plan.validate()?;
    Ok(plan)
}

/// A compact serializable representation useful for trace exports.
pub type Metadata = HashMap<String, serde_json::Value>;
