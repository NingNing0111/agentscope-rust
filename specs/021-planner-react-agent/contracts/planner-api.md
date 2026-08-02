# Contract: Planner Public API Behavior

**Feature**: 021 Planner + ReActAgent Compatibility  
**Scope**: Public behavior for creating, running, streaming, inspecting, and failing planned tasks.

## Compatibility Intent

Planner is an additive agent-layer capability. Existing `ReActAgent` behavior remains unchanged when planning is not enabled. Planner must reuse existing AgentScope Rust abstractions for model calls, tool execution, permissions, middleware, context, events, and cancellation.

## Public Capabilities

### Create a Planner-enabled agent or planner wrapper

Callers must be able to configure Planner with:

- A backing ReAct-capable agent or equivalent agent implementation.
- A model-backed or deterministic planning strategy.
- Limits for maximum steps, per-step ReAct iterations, and replanning attempts.
- Cancellation and timeout configuration.
- Trace redaction policy.

**Behavioral requirements**:

- Invalid configuration returns a typed validation/configuration error.
- Planner creation must not mutate the backing agent context.
- Planner configuration must not grant extra tool, memory, workspace, sandbox, or permission capabilities.

### Run a planned task non-streaming

Input:

- User goal as message(s) or goal text, depending on final Rust API shape.
- Optional execution metadata.

Output:

- Final assistant message or task result summary.
- Complete `PlanningTrace`.
- Final `PlannerOutcome`.

**Lifecycle**:

1. Validate goal.
2. Generate plan.
3. Validate plan.
4. Execute steps in order.
5. Replan if recoverable failure or new information requires it and limits allow.
6. Produce final outcome.

**Errors**:

- Empty goal → `InvalidGoal` or equivalent validation error.
- Empty/malformed plan → `MalformedPlan` or `NonActionablePlan`.
- Step limit exceeded → `StepLimitExceeded`.
- Replanning limit exceeded → `ReplanLimitExceeded`.
- Unsupported capability → `UnsupportedCapability` / `UnsupportedFeature` equivalent.
- Model/tool/permission/timeout/cancellation errors must preserve typed source category.

### Run a planned task streaming

Input is equivalent to non-streaming execution.

Output stream must include:

- Planner lifecycle events.
- Existing ReActAgent events for model/tool/reply lifecycle.
- A terminal task event.

**Ordering rules**:

- `PlanningStarted` occurs before initial plan generation output.
- `PlanningCompleted` occurs before first executable `StepStarted`.
- `StepStarted` occurs before any ReAct events associated with that step.
- Each step has exactly one terminal event: completed, failed, skipped, cancelled, or unsupported.
- `ReplanningStarted` occurs after the triggering step terminal event and before any replacement step starts.
- Task terminal event is the final planner lifecycle event.

**Backpressure/cancellation**:

- Streaming must respect bounded delivery/backpressure semantics.
- If the consumer or caller cancels, cancellation propagates to planning, replanning, or current ReAct step.
- Cancellation produces a typed terminal outcome, not a silent stream end.

### Inspect a planned task trace

Callers must be able to inspect:

- Initial goal.
- Plans and revisions.
- Step statuses and reasons.
- ReAct event correlations.
- Tool activity summaries.
- Final outcome.

Trace inspection must return redacted-safe data by default.

## Backward Compatibility Contract

- Existing `Agent::reply`, `Agent::reply_stream`, and `Agent::observe` behavior for non-planning `ReActAgent` must not change.
- Existing event order for normal ReAct text/tool flows must not change.
- Existing `AlreadyStreaming`/single-active-reply guard must remain effective.
- Existing middleware ordering and permission checks must remain effective.

## Unsupported Capability Contract

Planner must return explicit unsupported outcomes for capabilities outside this feature, including but not limited to:

- Distributed task scheduling.
- Durable external task queues.
- Remote planner workers.
- Parallel DAG execution.
- Full Python app-service runtime parity.
- Provider-specific natural-language planner output parity that cannot be deterministically tested.

Unsupported behavior must be recorded in the compatibility matrix and must not appear as successful no-op behavior.
