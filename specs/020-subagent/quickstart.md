# Quickstart: SubAgent Collaboration

**Feature**: 020-subagent | **Date**: 2026-08-02

This guide defines validation scenarios for the SubAgent Collaboration feature. It is a runnable/verification guide, not an implementation task list.

## Prerequisites

- Rust workspace dependencies installed.
- Current feature artifacts generated:
  - `specs/020-subagent/spec.md`
  - `specs/020-subagent/plan.md`
  - `specs/020-subagent/research.md`
  - `specs/020-subagent/data-model.md`
  - `specs/020-subagent/contracts/subagent-api.md`
  - `specs/020-subagent/contracts/delegation-trace.md`
- Deterministic test agents or scripted agents available during implementation.
- No live model credentials are required for acceptance tests.

## Validation Commands

Run the standard workspace checks after implementation:

```bash
rtk cargo fmt --check
rtk cargo check --workspace
rtk cargo test --workspace
rtk cargo clippy --workspace --all-targets -- -D warnings
```

If the implementation adds focused SubAgent tests, they should also be runnable directly, for example:

```bash
rtk cargo test -p agent_scope_agent subagent
```

## Scenario 1: Template validation

**Purpose**: Prove reusable SubAgent templates validate before runtime use.

**Setup**:

- Create one valid template with name, description, instructions, capability scope, and context policy.
- Create invalid templates missing each required field.

**Expected outcome**:

- Valid template emits or records `TemplateValidated`.
- Missing name/description/instructions/capability scope returns `InvalidTemplate`.
- Validation does not invoke a model, tool, workspace, or SubAgent run.

**Contracts**:

- `contracts/subagent-api.md` — Validate a template
- `contracts/delegation-trace.md` — `TemplateValidated`

## Scenario 2: Successful single SubAgent delegation

**Purpose**: Prove the primary parent-to-SubAgent-to-parent lifecycle.

**Setup**:

- Register a primary agent named `planner`.
- Register one SubAgent named `researcher` with deterministic scripted output.
- Send a parent task that explicitly delegates a bounded research subtask.

**Expected outcome**:

- Registry lists `researcher` with description and enabled state.
- Delegation trace contains, in order:
  1. `DelegationRequested`
  2. `SubAgentSelected`
  3. `SubAgentStarted`
  4. `SubAgentCompleted`
  5. `ResultObservedByParent` when promotion is enabled
- `CollaborationResult.status == Succeeded`.
- Result message has `Msg.name == "researcher"`.
- Parent final response may use the SubAgent result only after the terminal SubAgent outcome.

**Contracts**:

- `contracts/subagent-api.md` — Delegate a task
- `contracts/delegation-trace.md` — terminal outcome rules

## Scenario 3: Multiple SubAgents with preserved speaker identity

**Purpose**: Prove that more than one collaborator can participate without losing attribution.

**Setup**:

- Register `researcher` and `writer` with distinct responsibilities.
- Parent task requires both: one produces facts, one produces a summary.
- Use deterministic outputs.

**Expected outcome**:

- Each SubAgent receives only its intended scoped task.
- Multi-agent conversation includes user, parent, `researcher`, and `writer` participants.
- Every SubAgent-authored message preserves the corresponding `Msg.name`.
- Trace sequence numbers allow reconstruction even if completion order differs from invocation order.
- Parent final answer clearly reflects both results without flattening source identity in trace artifacts.

**Contracts**:

- `contracts/subagent-api.md` — CollaborationResult and message attribution
- `contracts/delegation-trace.md` — event ordering

## Scenario 4: Failure, timeout, and cancellation

**Purpose**: Prove every non-successful terminal outcome is typed and observable.

**Setup**:

- Configure one SubAgent that returns an execution error.
- Configure one SubAgent that exceeds timeout.
- Start one SubAgent and cancel the parent task before completion.

**Expected outcome**:

- Execution failure returns `CollaborationStatus::Failed` with category `ExecutionFailure`.
- Timeout returns `CollaborationStatus::TimedOut` with category `Timeout`.
- Parent cancellation returns `CollaborationStatus::Cancelled` with category `Cancellation`.
- No failed/timed-out/cancelled outcome is converted into a successful assistant message.
- Trace ends with the matching terminal event.

**Contracts**:

- `contracts/subagent-api.md` — Error Categories
- `contracts/delegation-trace.md` — Terminal Outcome Rules

## Scenario 5: Scope-denied access

**Purpose**: Prove SubAgent capabilities are bounded by policy.

**Setup**:

- Parent agent has access to a tool, memory store, workspace path, or sandbox capability.
- SubAgent is registered without that capability.
- Delegated task attempts to use the denied capability.

**Expected outcome**:

- The denied operation returns `PermissionDenied`.
- Trace contains `ScopeDenied`.
- Parent receives a typed failure instead of fabricated success.
- Default trace output contains a safe denial summary without raw secrets.

**Contracts**:

- `contracts/subagent-api.md` — Context sharing and capability scope
- `contracts/delegation-trace.md` — Redaction Rules

## Scenario 6: Unsupported distributed/app-service pattern

**Purpose**: Prove deferred Python app-service/message-bus/distributed patterns fail honestly.

**Setup**:

- Request a remote worker, durable external queue, cross-host migration, or full application-service dispatch pattern that is out of scope for this feature.

**Expected outcome**:

- The system returns `UnsupportedFeature`.
- Capability matrix records the unsupported or deferred pattern.
- No no-op success, empty result, or silent local fallback is reported.

**Contracts**:

- `contracts/subagent-api.md` — Compatibility Requirements
- `specs/020-subagent/spec.md` — Compatibility Scope

## Acceptance Checklist

Validation evidence recorded during implementation:

- `rtk cargo test -p agent_scope_agent subagent` passes focused deterministic SubAgent tests.
- `rtk cargo check --workspace` passes after public exports are wired.
- Distributed/runtime patterns are exposed through `UnsupportedFeature` helpers instead of no-op success.
- Compatibility matrix entries were added for in-process delegation support, SubAgentTemplate support, deferred provider formatter parity, and unsupported distributed/app-service patterns.

- [x] Template validation passes and fails deterministically.
- [x] Single SubAgent delegation succeeds and preserves `Msg.name`.
- [x] Multiple SubAgents preserve participant identity.
- [x] Failure, timeout, and cancellation return typed terminal outcomes.
- [x] Scope denial is enforced and traced.
- [x] Unsupported distributed/app-service patterns return `UnsupportedFeature`.
- [x] Existing single-agent tests pass with no behavior change when no SubAgents are configured.
- [x] Trace output is safe by default and includes no raw secret values.
- [x] Capability matrix records supported, deferred, and unsupported SubAgent-related patterns.
