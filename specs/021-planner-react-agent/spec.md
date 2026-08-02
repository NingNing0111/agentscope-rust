# Feature Specification: Planner + ReActAgent Compatibility

**Feature Branch**: `021-planner-react-agent`

**Created**: 2026-08-02

**Status**: Draft

**Input**: User description: "参考python版的agentscope，实现planner + ReActAgent"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Plan and Execute a Multi-Step Task (Priority: P1)

A developer wants to give an agent a goal that requires multiple steps, have the system produce an explicit plan, execute each step through the existing reasoning-and-acting flow, and return both the final answer and a traceable record of what was planned versus what actually happened.

**Why this priority**: This is the minimum useful Planner experience. Users need more than a single-turn tool-calling agent when tasks require decomposition, progress tracking, and controlled execution.

**Independent Test**: Provide a deterministic task such as "inspect two provided documents and summarize the differences" with fixed tools and fixed model responses. Verify that a plan is produced, each step is attempted in order, tool usage is recorded for the appropriate steps, and the final response identifies completed and skipped work.

**Acceptance Scenarios**:

1. **Given** a developer configures an agent with planning enabled and provides a goal with three clear subtasks, **When** the developer starts the task, **Then** the system produces an ordered plan before execution and records every step's status until completion.
2. **Given** a plan step requires tool usage, **When** the step is executed, **Then** the system uses the existing reasoning-and-acting flow for that step and records the tool call, tool result, and step outcome in the task trace.
3. **Given** all required plan steps complete successfully, **When** the final answer is returned, **Then** the answer includes the task result and a concise summary of completed steps.

---

### User Story 2 - Revise a Plan When Execution Changes the Situation (Priority: P2)

A developer wants the agent to adapt when a plan step fails, discovers new information, or becomes unnecessary. The revised plan should remain understandable and auditable rather than silently changing course.

**Why this priority**: Real planning tasks rarely proceed exactly as first proposed. Explicit replanning prevents hidden behavior changes and makes long-running agent work easier to trust.

**Independent Test**: Provide a task where the first planned tool action returns a recoverable error and the deterministic model proposes an alternative step. Verify that the original step is marked with its failure reason, a revised step is added or substituted, and the final trace preserves both the original and revised plan decisions.

**Acceptance Scenarios**:

1. **Given** a plan step fails with a recoverable tool error, **When** replanning is allowed, **Then** the system records the failure reason and produces a revised plan before continuing.
2. **Given** a completed step reveals that a later step is no longer needed, **When** the system revises the plan, **Then** the obsolete step is marked as skipped with a reason instead of being silently removed.
3. **Given** replanning reaches the configured attempt limit, **When** another failure occurs, **Then** the system stops safely and returns a typed failure summary with the current plan state.

---

### User Story 3 - Use Planner and ReActAgent in Streaming Applications (Priority: P3)

An application developer wants to show users live progress while a planned task runs: plan creation, step start, reasoning progress, tool calls, replanning, step completion, and final answer should all be observable in order.

**Why this priority**: Long-running planned tasks need transparent progress updates. This is especially important for user-facing applications and debugging tools.

**Independent Test**: Run a deterministic planned task through the streaming interface and verify that events arrive in chronological order, include all planning and reasoning milestones, and end with a final task completion event.

**Acceptance Scenarios**:

1. **Given** a planned task is run in streaming mode, **When** the system creates the initial plan, **Then** consumers receive plan-start and plan-complete progress events before step execution begins.
2. **Given** a step invokes tools through the reasoning-and-acting flow, **When** the stream is consumed, **Then** planning events and existing reasoning/tool events appear in a stable order without duplicate or missing lifecycle boundaries.
3. **Given** replanning happens during streaming execution, **When** the revised plan is accepted, **Then** consumers receive explicit replanning events and can correlate them with the affected step.

---

### User Story 4 - Preserve Python AgentScope Compatibility Evidence (Priority: P4)

A maintainer wants the Rust Planner and ReActAgent behavior to be checked against the Python AgentScope reference so compatibility claims are backed by reproducible evidence rather than assumptions.

**Why this priority**: The project goal is AgentScope compatibility. Planner behavior interacts with task state, events, tools, and agent loops, so trace-level evidence is required before declaring the feature complete.

**Independent Test**: Run matched deterministic scenarios against the Python reference and the Rust implementation, normalize non-deterministic fields, and verify that supported behavior matches at the level of plan structure, step status transitions, tool lifecycle, errors, and final outcome.

**Acceptance Scenarios**:

1. **Given** a supported Python reference scenario for a successful planned task, **When** the same scenario is run in Rust, **Then** the normalized plan trace, step transitions, and final result are equivalent.
2. **Given** a supported Python reference scenario with a failed step and replanning, **When** the same scenario is run in Rust, **Then** both systems record equivalent failure and revision semantics.
3. **Given** a Python Planner capability is intentionally out of scope, **When** a user attempts to use that capability, **Then** the Rust system reports it as unsupported with a stable machine-readable reason instead of silently ignoring it.

---

### Edge Cases

- The model produces an empty or malformed plan.
- The model produces a plan with no executable steps.
- A plan step repeatedly fails and replanning cannot make progress.
- A tool required by a step is unavailable, denied, or returns an execution error.
- The task is cancelled while the system is planning, executing a step, or replanning.
- Plan execution exceeds maximum allowed steps or replanning attempts.
- Streaming consumers stop reading before the planned task completes.
- Existing agent context is near its limit before planning or before a later step.
- Sensitive data appears in plan text, tool arguments, or trace summaries.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST let developers submit a user goal and receive an explicit ordered plan before task execution starts.
- **FR-002**: System MUST represent each plan step with a stable identifier, human-readable objective, status, and optional reason for failure, skip, or revision.
- **FR-003**: System MUST execute plan steps through the existing reasoning-and-acting agent behavior when a step requires model reasoning, tool usage, or both.
- **FR-004**: System MUST preserve a task trace that links the original goal, generated plan, step status changes, tool activity, replanning decisions, errors, and final answer.
- **FR-005**: System MUST support successful completion, partial completion, user cancellation, unrecoverable failure, and unsupported capability outcomes as distinct final task states.
- **FR-006**: System MUST allow recoverable step failures to trigger explicit replanning when replanning is enabled.
- **FR-007**: System MUST preserve replaced, skipped, or failed steps in the trace rather than deleting them from history.
- **FR-008**: System MUST enforce configured limits for total steps, reasoning-and-acting iterations per step, and replanning attempts.
- **FR-009**: System MUST stop safely with a typed error summary when configured limits are exceeded.
- **FR-010**: System MUST expose progress events for plan creation, step start, step completion, replanning start, replanning completion, task completion, task failure, and task cancellation.
- **FR-011**: System MUST interleave planning progress events with existing reasoning, tool, and final-response events in a stable chronological order.
- **FR-012**: System MUST support non-streaming use where callers receive the final answer and complete trace after execution.
- **FR-013**: System MUST support streaming use where callers can observe the task lifecycle as it happens.
- **FR-014**: System MUST handle empty, malformed, or non-actionable plans with typed validation errors.
- **FR-015**: System MUST treat unavailable or unsupported Python reference capabilities as explicit unsupported outcomes with stable machine-readable reasons.
- **FR-016**: System MUST avoid exposing secrets in default plan summaries, event payloads, trace output, or error messages.
- **FR-017**: System MUST provide deterministic compatibility scenarios for successful planning, tool-using step execution, replanning after recoverable failure, cancellation, and unsupported capability handling.
- **FR-018**: System MUST update the compatibility matrix with the supported Planner + ReActAgent compatibility level and any documented deviations from the Python reference.
- **FR-019**: System MUST document how developers create a planned task, observe progress, inspect traces, and handle failure states.
- **FR-020**: System MUST ensure existing reasoning-and-acting agent behavior remains backward compatible for users who do not enable planning.

### Key Entities *(include if feature involves data)*

- **Planned Task**: A user goal under planner control. It has an input goal, current plan, execution state, trace, final outcome, and cancellation status.
- **Plan**: An ordered collection of steps plus metadata explaining when it was created or revised.
- **Plan Step**: A single actionable objective with an identifier, status, attempt count, optional tool/reasoning activity, and optional failure or skip reason.
- **Plan Revision**: A recorded change to a plan, including the trigger, previous step state, new or updated steps, and rationale.
- **Planning Trace**: The auditable record connecting goals, plans, step transitions, events, tool activity, errors, and final output.
- **Planner Outcome**: The final state of a planned task: completed, partially completed, cancelled, failed, or unsupported.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Developers can complete a basic planned task setup and execution path with one agent configuration, one goal submission, and one final result inspection.
- **SC-002**: In deterministic tests, 100% of plan steps have exactly one terminal status: completed, skipped, failed, cancelled, or unsupported.
- **SC-003**: In deterministic streaming tests, all planning, step, reasoning, tool, and final lifecycle events appear in chronological order with no missing start/end boundaries.
- **SC-004**: Recoverable step-failure scenarios produce a revised plan and complete or stop with a typed failure summary in 100% of deterministic test runs.
- **SC-005**: Cancellation during planning, step execution, or replanning returns control to the caller within the configured grace period in 100% of deterministic cancellation tests.
- **SC-006**: Compatibility tests cover at least five Python reference scenarios and document every unsupported or intentionally different behavior in the compatibility matrix.
- **SC-007**: Existing non-planning ReActAgent behavior continues to pass all prior compatibility and regression tests.
- **SC-008**: Default traces and user-facing errors contain zero detected API keys, access tokens, or raw secret values in secret-scanning validation.

## Assumptions

- The scope is Planner orchestration plus its integration with the already existing reasoning-and-acting agent behavior; unrelated distributed multi-agent runtime features remain out of scope.
- Python AgentScope remains the behavior reference, and supported behavior will be verified by trace-level deterministic scenarios rather than natural-language output alone.
- The first compatibility target is L2/L3 for core planning behavior and public API semantics, with L4 example migration considered only for scenarios covered by user documentation and examples.
- Planning may use the same model interaction facilities as existing agents, but the specification does not require a particular internal architecture.
- Existing memory, workspace, tool permission, session, streaming, and trace facilities are available as dependencies for planned-task execution.
- Unsupported Python Planner capabilities must be explicit and documented instead of silently approximated.
