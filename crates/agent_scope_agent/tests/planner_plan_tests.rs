use agent_scope_agent::{
    Plan, PlanRevision, PlanRevisionTrigger, PlanStepStatus, PlannedTask, PlannerOutcome,
    parse_plan_json, validate_goal,
};

#[test]
fn plan_entities_round_trip() {
    let mut plan = Plan::new(
        "plan-1",
        "task-1",
        "Compare documents",
        vec!["Read first document".into(), "Read second document".into()],
    );
    plan.steps[0].start().unwrap();
    plan.steps[0].complete();

    let json = serde_json::to_string(&plan).unwrap();
    let restored: Plan = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.plan_id, "plan-1");
    assert_eq!(restored.steps.len(), 2);
    assert_eq!(restored.steps[0].status, PlanStepStatus::Completed);
}

#[test]
fn planned_task_round_trip_with_outcome() {
    let mut task = PlannedTask::new("summarize a report").unwrap();
    task.outcome = Some(PlannerOutcome::Completed {
        summary: "done".into(),
    });

    let json = serde_json::to_string(&task).unwrap();
    let restored: PlannedTask = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.goal, "summarize a report");
    assert!(matches!(
        restored.outcome,
        Some(PlannerOutcome::Completed { .. })
    ));
}

#[test]
fn invalid_goal_is_rejected() {
    assert!(validate_goal("   ").is_err());
}

#[test]
fn duplicate_step_ids_are_rejected() {
    let mut plan = Plan::new(
        "plan-1",
        "task-1",
        "duplicate test",
        vec!["one".into(), "two".into()],
    );
    plan.steps[1].step_id = plan.steps[0].step_id.clone();

    let err = plan.validate().unwrap_err();
    assert!(err.to_string().contains("unique"));
}

#[test]
fn empty_plan_is_rejected() {
    let plan = Plan::new("plan-1", "task-1", "empty", vec![]);
    let err = plan.validate().unwrap_err();
    assert!(err.to_string().contains("at least one step"));
}

#[test]
fn terminal_step_cannot_restart() {
    let mut plan = Plan::new("plan-1", "task-1", "terminal", vec!["one".into()]);
    plan.steps[0].complete();
    let err = plan.steps[0].start().unwrap_err();
    assert!(err.to_string().contains("cannot be restarted"));
}

#[test]
fn revision_must_point_to_different_plan() {
    let revision = PlanRevision::new(
        "task-1",
        "plan-1",
        "plan-1",
        PlanRevisionTrigger::RecoverableFailure,
        "retry",
    );
    assert!(revision.validate().is_err());
}

#[test]
fn parse_plan_json_accepts_strings_and_objects() {
    let raw = r#"{
        "objective": "Do work",
        "steps": ["first", {"objective": "second"}]
    }"#;
    let plan = parse_plan_json("task-1", raw).unwrap();
    assert_eq!(plan.steps.len(), 2);
    assert_eq!(plan.steps[0].objective, "first");
    assert_eq!(plan.steps[1].objective, "second");
}
