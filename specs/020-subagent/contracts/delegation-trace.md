# Contract: Delegation Trace

**Feature**: 020-subagent | **Date**: 2026-08-02

This contract defines the stable trace requirements for SubAgent collaboration. The trace is a core acceptance artifact and must be safe to inspect by default.

## Goals

- Reconstruct parent-to-SubAgent delegation lifecycle.
- Preserve event ordering and speaker attribution.
- Correlate SubAgent events with parent reply and delegation IDs.
- Represent every terminal outcome explicitly.
- Avoid exposing secrets or unnecessary sensitive conversation content.

## Trace Identity

Each delegation trace MUST include:

| Field | Requirement |
|-------|-------------|
| `trace_id` | Unique trace identity. |
| `parent_reply_id` | Parent reply or run correlation ID. |
| `delegation_id` | Delegation request correlation ID. |
| `parent_agent_name` | Primary agent speaker identity. |
| `target_subagent_name` | Target collaborator speaker identity. |
| `events` | Ordered lifecycle events. |
| `redactions` | Summary of values omitted from default trace. |

## Event Ordering

Each `DelegationEvent` MUST include:

| Field | Requirement |
|-------|-------------|
| `sequence` | Monotonic integer within the trace. |
| `event_type` | Stable event type. |
| `agent_name` | Parent or SubAgent speaker identity. |
| `delegation_id` | Correlation ID. |
| `summary` | Redacted human-readable summary. |
| `error` | Optional typed error for failure events. |

Sequence numbers MUST allow a reviewer to reconstruct ordering even if multiple SubAgents complete out of invocation order.

## Required Event Types

| Event Type | When emitted |
|------------|--------------|
| `TemplateValidated` | A template is validated for use. |
| `SubAgentRegistered` | A collaborator becomes available. |
| `DelegationRequested` | Parent creates a delegation request. |
| `SubAgentSelected` | Target collaborator is selected. |
| `SubAgentStarted` | Target begins processing. |
| `SubAgentEventForwarded` | A SubAgent agent-level event is correlated to this trace. |
| `SubAgentCompleted` | Target returns a successful result. |
| `SubAgentFailed` | Target returns an execution failure. |
| `SubAgentTimedOut` | Target exceeds timeout. |
| `SubAgentCancelled` | Parent or caller cancels work. |
| `ScopeDenied` | Capability/context policy denies access. |
| `ResultObservedByParent` | Parent observes or incorporates successful result. |

## Required Trace Scenarios

The implementation MUST produce deterministic traces for at least these scenarios:

1. Template validation success and failure.
2. Successful single SubAgent delegation.
3. Multiple SubAgents invoked in one parent task.
4. SubAgent execution failure.
5. Timeout or cancellation.
6. Scope-denied access to a tool, memory, session, workspace, or sandbox capability.

## Redaction Rules

Default trace output MUST NOT include:

- API keys, access tokens, credentials, or raw secrets.
- Full sensitive prompts or private conversation content unless explicitly enabled by a debugging mode.
- Raw tool arguments that contain secrets.
- Host-specific private absolute paths when a relative or redacted path is sufficient.

Default trace output SHOULD include:

- Agent names.
- Delegation ID and trace ID.
- Safe task summary.
- Safe result summary.
- Error category and stable code.
- Redaction notes explaining omitted data.

## Terminal Outcome Rules

Every delegation trace MUST end with exactly one terminal event:

| Terminal Event | CollaborationResult status |
|----------------|----------------------------|
| `SubAgentCompleted` | `Succeeded` |
| `SubAgentFailed` | `Failed` |
| `SubAgentTimedOut` | `TimedOut` |
| `SubAgentCancelled` | `Cancelled` |
| `ScopeDenied` | `PermissionDenied` |

`UnsupportedFeature` outcomes MUST be represented as a typed failure with category `UnsupportedFeature` and must not be recorded as successful completion.

## Compatibility Rules

- Parent final response MUST occur after terminal outcomes for all SubAgent results it claims to use.
- SubAgent messages MUST retain `Msg.name` and must not be flattened into parent messages in trace artifacts.
- Forwarded SubAgent `AgentEvent` records must remain distinguishable from parent `AgentEvent` records.
- Fields that are non-deterministic, such as timestamps and generated IDs, may be normalized in compatibility tests, but event order, speaker names, statuses, error categories, and side-effect attribution MUST NOT be ignored.
