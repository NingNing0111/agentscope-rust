# Data Model: Planner + ReActAgent Compatibility

**Feature**: 021 Planner + ReActAgent Compatibility  
**Date**: 2026-08-02

## Overview

Planner state is modeled as explicit, serializable entities so planned tasks can be validated, streamed, traced, compared against Python reference behavior, and inspected after completion. The model is intentionally additive to existing Agent/ReActAgent state and does not change non-planning ReActAgent behavior.

## Entity: PlannedTask

Represents one user goal being handled by Planner orchestration.

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `task_id` | stable string ID | Yes | Correlation ID for this planned task. |
| `goal` | string or message summary | Yes | User-visible goal submitted to the Planner. |
| `plan` | `Plan` | Optional before planning, required after plan creation | Current active plan. |
| `revisions` | list of `PlanRevision` | Yes | Ordered revision history. Empty when no replanning occurred. |
| `trace` | `PlanningTrace` | Yes | Auditable lifecycle record. |
| `outcome` | `PlannerOutcome` | Optional until terminal | Final task state. |
| `created_at` | timestamp | Yes | Creation time; normalized in compatibility tests. |
| `updated_at` | timestamp | Yes | Last update time; normalized in compatibility tests. |
| `metadata` | object | No | Non-sensitive extension metadata. Unknown keys must round-trip. |

### Validation Rules

- `task_id` MUST be non-empty and stable within the task trace.
- `goal` MUST be non-empty after input validation.
- `outcome` MUST be absent until the task reaches a terminal state.
- A terminal task MUST have exactly one `PlannerOutcome`.
- `metadata` MUST NOT contain raw secrets by default.

### Relationships

- Owns one active `Plan`.
- Owns zero or more `PlanRevision` records.
- Owns one `PlanningTrace`.

## Entity: Plan

Represents an ordered set of actionable steps for a user goal.

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `plan_id` | stable string ID | Yes | Identifier for the plan version. |
| `task_id` | stable string ID | Yes | Parent planned task. |
| `version` | positive integer | Yes | Starts at 1; increments for revisions. |
| `objective` | string | Yes | Human-readable objective for this plan. |
| `steps` | list of `PlanStep` | Yes | Ordered step list. |
| `status` | `PlanStatus` | Yes | Lifecycle status of the plan. |
| `created_reason` | string | No | Why this plan was created, e.g. initial planning or replanning trigger. |
| `metadata` | object | No | Non-sensitive extension metadata. |

### Validation Rules

- `plan_id` and `task_id` MUST be non-empty.
- `version` MUST be >= 1.
- `steps` MUST contain at least one executable or explicitly unsupported/skipped step after validation.
- Step IDs MUST be unique within a plan.
- Step order MUST remain stable after plan creation; revisions create new versions instead of silently reordering history.

### PlanStatus Values

| Status | Meaning |
|--------|---------|
| `Draft` | Plan was produced but not accepted for execution yet. |
| `Active` | Plan is currently being executed. |
| `Revised` | Plan has been superseded by a later version. |
| `Completed` | All required steps reached terminal successful or skipped states. |
| `Failed` | Plan cannot continue due to unrecoverable failure. |
| `Cancelled` | Plan execution was cancelled. |
| `Unsupported` | Plan requires unsupported capability. |

## Entity: PlanStep

Represents one actionable unit in a plan.

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `step_id` | stable string ID | Yes | Unique within the plan. |
| `plan_id` | stable string ID | Yes | Parent plan. |
| `index` | zero- or one-based integer as defined by implementation contract | Yes | Stable ordering position. |
| `objective` | string | Yes | Human-readable step objective. |
| `status` | `PlanStepStatus` | Yes | Current step status. |
| `attempt_count` | non-negative integer | Yes | Number of execution attempts. |
| `requires_react_execution` | boolean | Yes | Whether this step is executed via ReActAgent. |
| `tool_activity` | list of summarized tool records | No | Tool calls/results linked to this step. |
| `reason` | string | No | Failure, skip, cancellation, unsupported, or revision reason. |
| `started_at` | timestamp | No | Step start time; normalized in compatibility tests. |
| `completed_at` | timestamp | No | Terminal time; normalized in compatibility tests. |
| `metadata` | object | No | Non-sensitive extension metadata. |

### Validation Rules

- `step_id` MUST be unique within its plan.
- `objective` MUST be non-empty.
- `attempt_count` MUST never decrease.
- A terminal step MUST NOT transition back to a non-terminal state.
- A failed/skipped/unsupported/cancelled step SHOULD include a reason.
- `tool_activity` MUST summarize tool names, arguments, results, and errors without raw secrets.

### PlanStepStatus Values

| Status | Terminal | Meaning |
|--------|----------|---------|
| `Pending` | No | Step exists but has not started. |
| `Running` | No | Step is currently executing. |
| `Completed` | Yes | Step completed successfully. |
| `Skipped` | Yes | Step was intentionally skipped with a reason. |
| `Failed` | Yes | Step failed and was not recovered within this plan version. |
| `Cancelled` | Yes | Step stopped due to cancellation. |
| `Unsupported` | Yes | Step requires unsupported capability. |

## Entity: PlanRevision

Records an explicit plan change caused by failure, new information, or obsolete work.

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `revision_id` | stable string ID | Yes | Identifier for this revision. |
| `task_id` | stable string ID | Yes | Parent planned task. |
| `from_plan_id` | stable string ID | Yes | Superseded plan. |
| `to_plan_id` | stable string ID | Yes | Replacement plan. |
| `trigger_step_id` | stable string ID | No | Step that caused replanning. |
| `trigger` | `PlanRevisionTrigger` | Yes | Reason class for replanning. |
| `rationale` | string | Yes | Human-readable explanation. |
| `created_at` | timestamp | Yes | Revision creation time; normalized in compatibility tests. |
| `metadata` | object | No | Non-sensitive extension metadata. |

### Revision Triggers

| Trigger | Meaning |
|---------|---------|
| `RecoverableFailure` | A step failed but replanning can continue. |
| `NewInformation` | A step result changed the remaining work. |
| `ObsoleteStep` | A future step is no longer needed. |
| `LimitReached` | Replanning or step attempts hit a configured limit. |
| `UserCancellation` | User cancellation caused plan termination. |
| `UnsupportedCapability` | Unsupported capability was discovered. |

### Validation Rules

- `from_plan_id` and `to_plan_id` MUST differ.
- `rationale` MUST be non-empty.
- The superseded plan MUST remain in trace history.

## Entity: PlanningTrace

Auditable record connecting the goal, plans, revisions, ReAct events, tool activity, errors, cancellation, and final outcome.

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `trace_id` | stable string ID | Yes | Trace correlation ID. |
| `task_id` | stable string ID | Yes | Parent task. |
| `events` | list of `PlanningEvent` | Yes | Ordered lifecycle events. |
| `normalized_fields` | list of strings | No | Fields normalized during compatibility comparison. |
| `redaction_policy` | string or object | Yes | Policy used to produce safe summaries. |
| `final_outcome` | `PlannerOutcome` | Optional until terminal | Final state when complete. |
| `metadata` | object | No | Non-sensitive extension metadata. |

### Validation Rules

- Events MUST be strictly ordered by sequence number.
- Each started lifecycle boundary MUST have exactly one matching terminal boundary unless task cancellation interrupts it with a documented terminal event.
- Trace MUST include enough information to correlate plan steps with ReActAgent reply/model/tool events.
- Trace MUST NOT include raw API keys, credentials, access tokens, or unredacted sensitive payloads by default.

## Entity: PlanningEvent

Structured lifecycle event emitted by Planner and correlated with existing AgentEvent entries.

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `sequence` | positive integer | Yes | Monotonic event order. |
| `event_type` | enum/string | Yes | Stable event type. |
| `task_id` | stable string ID | Yes | Parent task. |
| `plan_id` | stable string ID | No | Associated plan. |
| `step_id` | stable string ID | No | Associated step. |
| `agent_event_ref` | stable string ID or sequence | No | Link to underlying ReActAgent event. |
| `summary` | string | No | Redacted human-readable summary. |
| `error` | `PlannerError` | No | Typed error for failure events. |
| `timestamp` | timestamp | No | Normalized in compatibility tests. |
| `metadata` | object | No | Non-sensitive extension metadata. |

### Event Types

- `PlanningStarted`
- `PlanningCompleted`
- `PlanValidationFailed`
- `StepStarted`
- `StepCompleted`
- `StepFailed`
- `StepSkipped`
- `StepCancelled`
- `StepUnsupported`
- `ReplanningStarted`
- `ReplanningCompleted`
- `TaskCompleted`
- `TaskPartiallyCompleted`
- `TaskFailed`
- `TaskCancelled`
- `TaskUnsupported`

## Entity: PlannerOutcome

Final state for a planned task.

### Outcome Values

| Outcome | Meaning |
|---------|---------|
| `Completed` | Required task work completed successfully. |
| `PartiallyCompleted` | Some useful work completed but at least one required step did not complete. |
| `Cancelled` | User or caller cancelled the task. |
| `Failed` | Unrecoverable failure stopped the task. |
| `Unsupported` | Task requires unsupported capability. |

### Validation Rules

- A `PlannerOutcome` MUST include enough summary information for callers to understand the final state.
- `Failed` and `Unsupported` outcomes MUST include a stable machine-readable category.
- `Completed` outcomes MUST have no non-terminal required steps.

## Entity: PlannerError

Typed error details for validation, execution, replanning, cancellation, timeout, permission, and unsupported capabilities.

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `category` | `PlannerErrorCategory` | Yes | Stable machine-readable category. |
| `message` | string | Yes | Redacted user/developer-facing message. |
| `task_id` | stable string ID | No | Associated task. |
| `plan_id` | stable string ID | No | Associated plan. |
| `step_id` | stable string ID | No | Associated step. |
| `source_category` | string | No | Mapped source error category such as model/tool/permission. |
| `retryable` | boolean | Yes | Whether replanning or retry can continue. |
| `metadata` | object | No | Non-sensitive extension metadata. |

### Categories

- `InvalidGoal`
- `PlanGenerationFailed`
- `MalformedPlan`
- `NonActionablePlan`
- `StepExecutionFailed`
- `ReplanningFailed`
- `StepLimitExceeded`
- `ReplanLimitExceeded`
- `Timeout`
- `Cancelled`
- `PermissionDenied`
- `UnsupportedCapability`
- `TraceSerializationFailed`
- `InternalError`

## State Transitions

### PlannedTask Lifecycle

```text
Created
  → Planning
  → Executing
  → Replanning → Executing   (zero or more times)
  → Completed | PartiallyCompleted | Failed | Cancelled | Unsupported
```

### PlanStep Lifecycle

```text
Pending
  → Running
  → Completed | Skipped | Failed | Cancelled | Unsupported
```

### Replanning Lifecycle

```text
TriggerDetected
  → ReplanningStarted
  → NewPlanValidated
  → OldPlanRevised + NewPlanActive
  → Executing
```

Failure path:

```text
TriggerDetected
  → ReplanningStarted
  → ReplanningFailed
  → Failed | PartiallyCompleted
```

## Serialization and Compatibility Rules

- Public data entities SHOULD derive or otherwise support `Serialize` and `Deserialize` where exposed in traces/contracts.
- Unknown metadata fields SHOULD round-trip when possible.
- Non-deterministic fields (`timestamp`, generated IDs, request IDs, latency) MUST be normalizable in compatibility tests.
- Event order, status transitions, tool arguments, error category, and cancellation state MUST NOT be normalized away.
- Raw secrets MUST be redacted before trace serialization by default.
