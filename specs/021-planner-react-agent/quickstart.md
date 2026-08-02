# Quickstart: Planner + ReActAgent Compatibility Validation

**Feature**: 021 Planner + ReActAgent Compatibility  
**Date**: 2026-08-02

This guide defines validation scenarios for the design and implementation phase. It is not an implementation tutorial; it lists the commands and expected outcomes that prove the feature works end-to-end.

## Prerequisites

- Rust workspace dependencies available.
- Deterministic mock/scripted model fixtures available for agent tests.
- Fixed test tools available for calculator/file-summary/error scenarios.
- Python AgentScope reference fixtures available or generated for compatibility scenarios.
- No live LLM output required for correctness gates.

## Validation Commands

Run from repository root:

```bash
rtk cargo test -p agent_scope_agent planner
rtk cargo test -p agent_scope_event
rtk cargo test --workspace
rtk cargo check --workspace
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo fmt --check
```

If compatibility fixture generation is updated for this feature:

```bash
rtk python tests/compatibility/generate_fixtures.py
rtk cargo test --workspace compatibility
```

## Scenario 1: Successful Planned Task

**Goal**: Verify initial planning and sequential execution.

**Setup**:

- Scripted planner response produces a three-step plan.
- Scripted ReAct responses complete each step.
- Fixed tools return deterministic results.

**Expected outcome**:

- A `PlanningStarted` event appears before plan creation.
- A `PlanningCompleted` event appears before any step starts.
- Each step transitions `Pending → Running → Completed`.
- Final outcome is `Completed`.
- Trace includes plan, steps, ReAct events, tool activity, and final answer.
- No secret-like values appear in trace output.

## Scenario 2: Tool-Using Plan Step

**Goal**: Verify Planner reuses ReActAgent tool lifecycle.

**Setup**:

- Plan contains a step requiring a tool call.
- Scripted model emits a tool call and then a final response.
- Tool returns a fixed result.

**Expected outcome**:

- Step starts before the ReAct reply lifecycle.
- Existing tool lifecycle events appear in order: tool call start/delta/end and tool result start/delta/end as applicable.
- Step completes after the ReAct reply terminal event.
- Tool activity is correlated with the step ID.

## Scenario 3: Recoverable Failure and Replanning

**Goal**: Verify explicit replanning and history preservation.

**Setup**:

- Initial plan contains a step whose tool returns a recoverable error.
- Replanning is enabled.
- Scripted planner response produces a revised plan with an alternate step.

**Expected outcome**:

- Failed step is marked `Failed` with a reason.
- `ReplanningStarted` follows the failed step terminal event.
- `ReplanningCompleted` records a new plan version.
- Superseded plan remains in trace history.
- Final outcome is `Completed` or `PartiallyCompleted` depending on required work.

## Scenario 4: Replanning Limit Exceeded

**Goal**: Verify safe stop when replanning cannot make progress.

**Setup**:

- Configure a low replanning limit.
- Scripted responses repeatedly produce recoverable failures.

**Expected outcome**:

- Replanning attempts stop at the configured limit.
- Final outcome is `Failed` or `PartiallyCompleted` with `ReplanLimitExceeded` category.
- Trace includes all attempted revisions and terminal task event.

## Scenario 5: Cancellation During Planning, Step Execution, and Replanning

**Goal**: Verify cancellation propagation at every lifecycle boundary.

**Setup**:

- Use deterministic delayed planning/step/replanning fixtures.
- Trigger cancellation during each phase in separate tests.

**Expected outcome**:

- Cancellation propagates to the active operation.
- Current lifecycle boundary receives a terminal cancellation event.
- Final outcome is `Cancelled`.
- Caller regains control within the configured grace period.
- No orphan background work remains.

## Scenario 6: Malformed or Non-Actionable Plan

**Goal**: Verify typed validation failures.

**Setup**:

- Scripted planner emits empty content, malformed data, or a plan with no actionable steps.

**Expected outcome**:

- Execution does not begin.
- Error category is `MalformedPlan` or `NonActionablePlan`.
- Trace contains `PlanValidationFailed` and a terminal failure event.
- No fake successful empty plan is returned.

## Scenario 7: Unsupported Python Planner Capability

**Goal**: Verify unsupported behavior is explicit.

**Setup**:

- Request a capability outside Feature 021 scope, such as distributed scheduling or parallel DAG execution.

**Expected outcome**:

- Planner returns an `UnsupportedCapability`/`UnsupportedFeature` outcome.
- Compatibility matrix records the capability as unsupported/deferred.
- Trace includes `TaskUnsupported`.
- No silent fallback or no-op success occurs.

## Scenario 8: Existing ReActAgent Regression

**Goal**: Verify planning is additive and does not alter existing behavior.

**Setup**:

- Run existing ReActAgent tests without enabling Planner.

**Expected outcome**:

- Existing text reply, tool call, middleware, permission, streaming, interruption, and context tests pass unchanged.
- Existing event ordering remains unchanged.
- Existing `AlreadyStreaming` or single-active-reply guard remains effective.

## Scenario 9: Python vs Rust Normalized Trace Comparison

**Goal**: Verify AgentScope compatibility evidence.

**Setup**:

- Generate or load Python reference traces for at least five deterministic scenarios:
  1. successful planned task
  2. tool-using step
  3. recoverable failure with replanning
  4. cancellation
  5. unsupported/deferred capability
- Run equivalent Rust scenarios.
- Normalize timestamps, generated IDs, request IDs, and latency.

**Expected outcome**:

- Supported scenarios match on event order, step transitions, tool names/arguments/results, error categories, cancellation state, and final outcome.
- Any deviation is documented in `specs/001-compatibility-baseline/capability-matrix.json`.

## Required Artifact Updates Before Completion

- `crates/agent_scope_agent` planner tests pass.
- Event serde/sequence tests updated if planning events are added.
- Compatibility fixtures updated when Python reference scenarios are available.
- `specs/001-compatibility-baseline/capability-matrix.json` records supported/deferred Planner capabilities.
- `docs/en/modules/agent.md` and `docs/zh/modules/agent.md` document Planner usage and failure handling.
- Example/demo updates compile if added.


## Validation Notes (2026-08-02)

- Planner lifecycle streaming uses `AgentEvent::Custom` with `name = "planner.lifecycle"`; no new `AgentEvent` variants were required for Feature 021.
- Cancellation coverage is deterministic at the public cancellation boundary (`Planner::cancel()` before non-streaming and streaming planning). Mid-call cooperative cancellation remains constrained by the current synchronous scripted model/agent test doubles.
- Python compatibility evidence is represented by deterministic normalized fixture shapes in `tests/compatibility/fixtures/planner_*_trace.json`: success, tool step, replanning, cancellation, and unsupported capability.
- The live `agent_demo` remains a single real DashScope-backed `ReActAgent`; optional Planner wiring is documented but not enabled as a demo flag because deterministic Planner validation is covered by tests and should not change live interactive behavior.
- Quick validation commands for this update:
  - `rtk python tests/compatibility/generate_fixtures.py tests/compatibility/fixtures`
  - `rtk cargo test -p agent_scope_agent planner`
  - `rtk cargo test -p agent_scope_event planner_lifecycle_custom_event_round_trip`
