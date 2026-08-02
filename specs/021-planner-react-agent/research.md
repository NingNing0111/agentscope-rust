# Research: Planner + ReActAgent Compatibility

**Feature**: 021 Planner + ReActAgent Compatibility  
**Date**: 2026-08-02

## Decision 1: Implement Planner as an additive `agent_scope_agent` orchestration layer

**Decision**: Planner capabilities will live primarily in `crates/agent_scope_agent` as additive modules (`planner`, `plan`, `planning_trace`, `planner_error`, optional stream helpers) and will reuse the existing `Agent` trait and `ReActAgent` execution flow.

**Rationale**: Planner must coordinate agent context, model calls, tool execution, middleware, permissions, cancellation, and streaming events. These behaviors already belong to the agent layer. Keeping Planner in `agent_scope_agent` preserves dependency direction and avoids introducing a new runtime abstraction before distributed execution is in scope.

**Alternatives considered**:
- **New standalone planner crate**: Rejected for this feature because it would either duplicate agent-layer concerns or require broader public API stabilization across multiple crates.
- **Modify `ReActAgent` into a planner by default**: Rejected because existing non-planning ReActAgent behavior must remain backward compatible.
- **Implement planner as a tool only**: Rejected because planner state, replanning, event lifecycle, and trace contracts are first-class orchestration concerns, not just tool execution.

## Decision 2: Reuse existing `ReActAgent` for plan step execution

**Decision**: Each executable plan step will be run through the existing ReAct reasoning→acting flow when it needs model reasoning, tool usage, or both.

**Rationale**: Existing `ReActAgent` already owns model requests, tool calls, permission checks, middleware hook order, event emission, cancellation semantics, and regression tests. Reusing it reduces behavioral drift and keeps Python compatibility focused on observable planner behavior rather than reimplementing the agent loop.

**Alternatives considered**:
- **Separate planner-specific tool loop**: Rejected because it risks divergent tool lifecycle events and permission behavior.
- **Direct tool dispatch from Planner**: Rejected except for explicitly modeled validation-only steps; direct dispatch would bypass ReActAgent semantics and trace expectations.

## Decision 3: Model plan state with stable data structures and explicit terminal states

**Decision**: Define `PlannedTask`, `Plan`, `PlanStep`, `PlanRevision`, `PlanningTrace`, and `PlannerOutcome` as stable public data entities. Every step must end in exactly one terminal status: `Completed`, `Skipped`, `Failed`, `Cancelled`, or `Unsupported`.

**Rationale**: Feature requirements and success criteria require auditable history, reproducible trace comparison, and no hidden plan mutation. Stable entities make compatibility testing, serialization round-trip tests, and documentation possible.

**Alternatives considered**:
- **Represent plans as raw model text**: Rejected because raw text cannot support robust validation, status transitions, or trace-level diff tests.
- **Only return final answer**: Rejected because AgentScope compatibility requires trace and state transitions as acceptance artifacts.

## Decision 4: Use explicit replanning records instead of mutating plans silently

**Decision**: Replanning creates `PlanRevision` records and preserves failed, skipped, replaced, or obsolete steps in the trace.

**Rationale**: Silent replanning violates the specification and the constitution's trace requirements. Users and tests must see why a step changed, which step triggered the revision, and what new steps were introduced.

**Alternatives considered**:
- **Replace the whole plan with the new plan**: Rejected because it loses audit history.
- **Append only the final plan to the trace**: Rejected because it cannot explain failures or skipped work.

## Decision 5: Prefer stable Planner lifecycle events over opaque logs

**Decision**: Planner progress must be observable through structured lifecycle events for plan creation, step start/end, replanning start/end, task completion, task failure, and task cancellation. If existing `AgentEvent` variants can express a boundary without losing semantics, reuse them; otherwise add planning-specific event variants additively and cover them with serde/event sequence tests.

**Rationale**: Existing `AgentEvent` is the repository-wide observable event contract. Planner must not introduce a parallel event system. Structured events are required for streaming consumers, trace comparison, and debugging.

**Alternatives considered**:
- **Use plain text logs**: Rejected because logs are not stable contracts and cannot support deterministic trace comparison.
- **Use only `Custom` events permanently**: Considered acceptable for early internal prototyping, but not sufficient for stable public compatibility if Planner lifecycle becomes public API.
- **Use `HintBlock` only**: Useful for planner annotations, but insufficient for task/step/replanning terminal lifecycle semantics.

## Decision 6: Deterministic tests must use scripted/mock models and fixed tools

**Decision**: Planner tests will use deterministic scripted/mock models, fixed tools, fixed or normalized IDs/timestamps, and compatibility fixtures. Live LLM output can be used in examples but not as the core compatibility oracle.

**Rationale**: Constitution §6 and §7 require deterministic compatibility tests and trace comparison. Planner output from live models is inherently nondeterministic.

**Alternatives considered**:
- **Use live model output for planner correctness**: Rejected because it is nondeterministic and cannot be a release gate.
- **Only Rust unit tests without Python reference fixtures**: Rejected because Python AgentScope remains the behavior baseline.

## Decision 7: Define typed Planner errors while interoperating with `AgentError`

**Decision**: Introduce a typed planner error model only where stable planner-specific failure categories are needed, and provide interop/mapping to existing `AgentError` categories for model, tool, timeout, cancellation, validation, permission, and unsupported outcomes.

**Rationale**: Existing `AgentError` already covers many categories. Planner still needs stable categories for malformed plans, non-actionable plans, step limit exceeded, replanning limit exceeded, and unsupported planner capabilities.

**Alternatives considered**:
- **Use string messages only**: Rejected by constitution §13.
- **Force all errors into existing generic categories**: Rejected because callers need to distinguish planner validation/replanning/unsupported outcomes programmatically.

## Decision 8: Keep storage in-memory for v1 and integrate with existing session/memory/workspace boundaries

**Decision**: Planned task state and trace are held in memory by default. Persistence or side effects use existing session, memory, workspace, or trace facilities; no new database, queue, or durable scheduler is introduced in Feature 021.

**Rationale**: The feature scope is Planner + ReActAgent compatibility, not distributed runtime. In-memory state is sufficient for deterministic tests and library use while preserving future extension points.

**Alternatives considered**:
- **Add durable task store**: Rejected as premature and closer to distributed runtime.
- **Use workspace files as the default state store**: Rejected because planning should not require file-system side effects by default.

## Decision 9: Respect least-privilege capability boundaries

**Decision**: Planner does not grant additional tool, memory, workspace, sandbox, or permission capabilities. Each plan step executes under the same configured ReActAgent boundaries unless explicitly configured by future features.

**Rationale**: Planning should not become a permission escalation path. Existing permission and capability checks must remain the enforcement points.

**Alternatives considered**:
- **Planner grants tools based on generated plan text**: Rejected because model output must not authorize capabilities.
- **Planner bypasses permission checks for planned steps**: Rejected because it violates security and existing ReActAgent semantics.

## Decision 10: Target compatibility level L2/L3 for Feature 021

**Decision**: Feature 021 targets L2 core behavior compatibility and L3 public API semantics for supported Planner + ReActAgent scenarios. L4 example migration is limited to documented examples. Distributed runtime, parallel DAG planning, remote workers, and unsupported Python planner capabilities remain deferred/unsupported.

**Rationale**: The feature can provide immediate value with deterministic in-process planning while avoiding false claims of full Python parity. This aligns with the constitution's compatibility-level requirements.

**Alternatives considered**:
- **Claim full L4 compatibility**: Rejected because not all Python planner/app-service/runtime capabilities are in scope.
- **Only L1 protocol compatibility**: Rejected because the user requested implementation of Planner + ReActAgent behavior, not only data structures.
