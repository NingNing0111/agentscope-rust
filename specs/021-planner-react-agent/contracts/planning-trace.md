# Contract: Planning Trace and Event Ordering

**Feature**: 021 Planner + ReActAgent Compatibility  
**Scope**: Stable trace, event ordering, correlation, normalization, and redaction requirements.

## Trace Purpose

The planning trace is the primary acceptance artifact for Planner + ReActAgent behavior. It must allow deterministic comparison between Rust behavior and Python AgentScope reference scenarios after normalizing non-deterministic fields.

## Required Trace Sections

A complete trace must capture:

- Input goal/messages.
- Planner configuration relevant to observable behavior.
- Generated plans and plan revisions.
- Step status transitions.
- Model requests/responses involved in planning and step execution.
- Existing ReActAgent lifecycle events.
- Tool calls, tool arguments, tool results, and tool errors.
- Middleware hook observations if relevant to behavior.
- State/context transitions relevant to plan execution.
- Errors and stable error categories.
- Cancellation state and source.
- Final task outcome and final response.

This should align with the existing compatibility baseline trace schema categories: `input`, `model_requests`, `model_responses`, `streaming_chunks`, `tool_calls`, `tool_results`, `events`, `memory_mutations`, `state_transitions`, `errors`, `cancellation`, and `final_result`.

## Event Ordering Contract

For a successful planned task:

```text
PlanningStarted
PlanningCompleted
StepStarted(step-1)
  ReplyStart / ModelCallStart / ModelCallEnd / ... / Tool... / ReplyEnd
StepCompleted(step-1)
StepStarted(step-2)
  ReplyStart / ... / ReplyEnd
StepCompleted(step-2)
TaskCompleted
```

For replanning after recoverable failure:

```text
StepStarted(step-n)
  ... ReAct events ...
StepFailed(step-n)
ReplanningStarted(trigger=step-n)
ReplanningCompleted(new_plan_version)
StepStarted(next-step)
...
TaskCompleted | TaskPartiallyCompleted | TaskFailed
```

For cancellation:

```text
PlanningStarted | StepStarted | ReplanningStarted
Cancellation observed
Current boundary terminal event
TaskCancelled
```

## Correlation Rules

- Every planning event must include `task_id`.
- Events tied to a plan must include `plan_id`.
- Events tied to a step must include `step_id`.
- ReActAgent events emitted while executing a plan step must be correlatable to that step.
- Tool activity must be correlatable to both the ReAct event lifecycle and the plan step.
- Plan revisions must reference both superseded and replacement plan IDs.

## Normalization Rules

Compatibility tests may normalize:

- Timestamps.
- UUIDs or generated IDs, if stable mapping is preserved.
- Provider request IDs.
- Network latency and token timing.
- Map key ordering.
- Non-semantic whitespace in model text, if explicitly documented.

Compatibility tests must not normalize away:

- Event order.
- Step status transitions.
- Tool names.
- Tool argument values, except redacted secrets.
- Tool result success/failure category.
- Error category.
- Cancellation state.
- Final outcome.
- Unsupported/deferred capability markers.

## Redaction Rules

Default traces and event summaries must not contain:

- API keys.
- Access tokens.
- Raw passwords.
- Credentials.
- Unredacted personal or sensitive data that is not required for behavior comparison.
- Full sensitive conversation content unless explicitly enabled by an insecure/debug option.

Redacted values must preserve enough structure for debugging and compatibility comparison, for example:

```text
"api_key": "[REDACTED]"
"tool_arguments": {"path": "workspace/report.md", "token": "[REDACTED]"}
```

## Serialization Contract

- Trace entities must support stable serialization for golden fixtures.
- Unknown non-sensitive metadata fields should round-trip where possible.
- New public event variants must have serde round-trip tests.
- New block/event integration must have append/event-sequence tests when it affects message reconstruction.

## Failure Contract

A trace for a failed planned task must include:

- The active plan and current step at failure time.
- The source error category.
- Whether the failure was retryable/replannable.
- Whether replanning was attempted.
- The final `PlannerOutcome`.

A trace ending without a task terminal event is invalid unless the test explicitly models a harness crash outside Planner control.
