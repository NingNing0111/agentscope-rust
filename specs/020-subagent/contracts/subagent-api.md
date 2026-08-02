# Contract: SubAgent Public API

**Feature**: 020-subagent | **Date**: 2026-08-02

This contract defines the user-facing library API semantics for in-process SubAgent collaboration. Names may be adapted idiomatically during implementation, but the observable behavior and error categories must remain stable.

## Scope

Covered:

- SubAgent template declaration and validation
- SubAgent registration and lookup
- Explicit delegation requests
- Collaboration result attribution
- Context sharing and capability scope
- Timeout, cancellation, and typed errors
- Trace correlation for parent and SubAgent lifecycle

Not covered:

- Distributed runtime or remote workers
- Durable queues or external message buses
- Full application service compatibility
- Provider-specific multi-agent formatter parity

## Core Types

### SubAgentTemplate

Required observable fields:

| Field | Requirement |
|-------|-------------|
| `name` | Non-empty default collaborator name or prefix. |
| `description` | Non-empty responsibility summary used for routing and documentation. |
| `instructions` | Non-empty SubAgent creation or behavior instructions. |
| `capability_scope` | Explicit effective capabilities. |
| `context_policy` | Explicit default sharing policy. |
| `default_budget` | Optional default invocation limits. |

### SubAgentRegistry

The registry MUST support these observable operations:

| Operation | Behavior |
|-----------|----------|
| `register_template(template)` | Validates and stores a reusable template. |
| `register_subagent(subagent)` | Stores a concrete collaborator with unique name. |
| `get(name)` | Returns an enabled SubAgent or typed missing/disabled error. |
| `list()` | Returns registered collaborators with name, description, and availability. |
| `disable(name)` / `enable(name)` | Changes availability without deleting identity. |

Validation behavior:

- Duplicate collaborator names MUST return `DuplicateSubAgent`.
- Missing target names MUST return `MissingSubAgent`.
- Disabled targets MUST return `DisabledSubAgent`.
- Invalid templates MUST return `InvalidTemplate` with stable reason codes.

### DelegationRequest

Required observable fields:

| Field | Requirement |
|-------|-------------|
| `delegation_id` | Stable correlation ID. |
| `parent_agent_name` | Parent speaker identity. |
| `target_subagent_name` | Target speaker identity. |
| `task` | Non-empty delegated task. |
| `context` | Context allowed by policy. |
| `budget` | Limits for depth, calls, timeout, and context size. |
| `reply_mode` | Final-only, stream-events, or observe-only. |

### CollaborationResult

Terminal outcomes MUST be represented by exactly one status:

| Status | Required payload |
|--------|------------------|
| `Succeeded` | A valid output `Msg` whose `name` equals the SubAgent name. |
| `Failed` | Typed `SubAgentErrorInfo`. |
| `TimedOut` | Typed timeout error and trace reference. |
| `Cancelled` | Typed cancellation error and trace reference. |
| `PermissionDenied` | Typed policy denial error. |
| `UnsupportedFeature` | Typed unsupported feature error. |

The system MUST NOT return `Succeeded` when the SubAgent was not invoked, failed, timed out, was cancelled, or returned malformed output.

## Required Operations

### Validate a template

```text
validate_template(template) -> ValidatedTemplate | SubAgentError
```

Acceptance requirements:

- Empty name, description, or instructions fail with `InvalidTemplate`.
- Capability scope that requests unavailable inherited access fails unless explicitly allowed.
- Validation does not start an agent run.

### Register a SubAgent

```text
registry.register_subagent(subagent) -> RegisteredSubAgent | SubAgentError
```

Acceptance requirements:

- Registered SubAgent appears in `registry.list()`.
- Duplicate name fails deterministically.
- Registration preserves the SubAgent `name` used later in `Msg.name`.

### Delegate a task

```text
delegate(request) -> CollaborationResult
```

Acceptance requirements:

- Missing, disabled, or ambiguous target fails before invoking unrelated collaborators.
- Valid requests create a trace with `DelegationRequested`, `SubAgentSelected`, `SubAgentStarted`, and a terminal event.
- The delegated task is passed with only allowed context.
- The result is attributable to `target_subagent_name`.
- Parent-visible failure uses typed error categories rather than string matching.

### Delegate with event stream

```text
delegate_stream(request) -> stream<DelegationEvent or AgentEvent> + terminal CollaborationResult
```

Acceptance requirements:

- Each forwarded SubAgent event is correlated to `delegation_id`.
- Parent and SubAgent events remain distinguishable.
- Stream termination includes a terminal collaboration result.
- Dropping or cancelling the parent stream triggers cancellation propagation.

### Observe result in parent

```text
parent.observe(collaboration_result.message) -> Result
```

Acceptance requirements:

- Successful SubAgent messages may be observed by the parent according to `ContextSharingPolicy.promote_results_to_parent`.
- Failed results are not converted into successful assistant messages.
- When observed, the original SubAgent speaker identity is preserved.

## Error Categories

The API MUST expose stable machine-readable categories:

| Category | Example scenario |
|----------|------------------|
| `InvalidTemplate` | Missing required template field. |
| `DuplicateSubAgent` | Registering duplicate name. |
| `MissingSubAgent` | Delegating to unknown target. |
| `DisabledSubAgent` | Delegating to disabled collaborator. |
| `AmbiguousSubAgent` | Automatic selection finds multiple equal matches. |
| `InvalidDelegation` | Empty task or invalid context. |
| `ExecutionFailure` | Underlying agent returns an error. |
| `Timeout` | Invocation exceeds budget. |
| `Cancellation` | Parent cancels work. |
| `PermissionDenied` | Scope blocks a capability. |
| `BudgetExceeded` | Depth/call/context budget exceeded. |
| `UnsupportedFeature` | Distributed/remote behavior requested in this feature. |
| `InternalError` | Framework invariant violation. |

## Compatibility Requirements

- All public SubAgent data structures intended for serialization MUST preserve unknown metadata fields where applicable.
- Message history MUST preserve `Msg.name` for every participant.
- Existing single-agent behavior MUST remain unchanged when no SubAgents are configured.
- Unsupported Python app-service/message-bus patterns MUST return stable unsupported outcomes, not no-op success.
