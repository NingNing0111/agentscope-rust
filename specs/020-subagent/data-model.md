# Data Model: SubAgent Collaboration

**Feature**: 020-subagent | **Date**: 2026-08-02

## Entity Relationship

```text
┌────────────────────┐       creates/registers       ┌────────────────────┐
│  SubAgentTemplate  │───────────────────────────────▶│      SubAgent      │
└─────────┬──────────┘                                └─────────┬──────────┘
          │ validates                                      owns │
┌─────────▼──────────┐                                ┌─────────▼──────────┐
│  TemplateStatus    │                                │ CapabilityScope    │
└────────────────────┘                                └─────────┬──────────┘
                                                                 │
┌────────────────────┐        lists/selects             ┌────────▼──────────┐
│ SubAgentRegistry   │─────────────────────────────────▶│ ContextSharing    │
└─────────┬──────────┘                                  │     Policy        │
          │                                             └───────────────────┘
          │ creates
┌─────────▼──────────┐       targets       ┌────────────────────┐
│ DelegationRequest  │────────────────────▶│      SubAgent      │
└─────────┬──────────┘                     └─────────┬──────────┘
          │ produces                                  │ produces events
┌─────────▼──────────┐       appends to     ┌─────────▼──────────┐
│ CollaborationResult│────────────────────▶│  DelegationTrace   │
└─────────┬──────────┘                     └─────────┬──────────┘
          │ updates                                  │ references
┌─────────▼──────────┐                     ┌─────────▼──────────┐
│MultiAgentConversation│◀──────────────────│ DelegationEvent    │
└────────────────────┘                     └────────────────────┘

┌────────────────────┐
│ DelegationBudget   │ configures DelegationRequest and lifecycle limits
└────────────────────┘
```

## Entity Definitions

### 1. SubAgentTemplate

**Purpose**: A reusable blueprint for creating or configuring SubAgents consistently across parent-agent workflows or teams.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `template_id` | `String` | Yes | Stable template identifier. |
| `name` | `String` | Yes | Default SubAgent name or name prefix. |
| `description` | `String` | Yes | Human-readable responsibilities and routing hint. |
| `instructions` | `String` | Yes | Task/system instructions used when creating the SubAgent. |
| `capability_scope` | `CapabilityScope` | Yes | Capabilities available to SubAgents from this template. |
| `context_policy` | `ContextSharingPolicy` | Yes | Default context sharing policy. |
| `default_budget` | `DelegationBudget` | No | Default limits for invocations created from this template. |
| `metadata` | `serde_json::Value` | No | Non-sensitive extension metadata. |
| `status` | `TemplateStatus` | Yes | Validation and availability status. |

**Validation Rules**:
- `template_id`, `name`, `description`, and `instructions` must be non-empty after trimming.
- `name` must be unique in the registry after normalization.
- `description` must be present because it is the user-visible routing basis.
- `capability_scope` must not grant capabilities unavailable to the parent unless explicitly allowed by policy.
- Invalid templates must fail before delegation with a typed configuration error.

### 2. TemplateStatus

| Variant | Description |
|---------|-------------|
| `Draft` | Constructed but not yet validated. |
| `Validated` | Can create or register a SubAgent. |
| `Disabled` | Valid but not selectable for delegation. |
| `Invalid { reasons }` | Cannot be used; includes stable validation reason codes. |

### 3. SubAgent

**Purpose**: A configured collaborator agent with a bounded lifecycle inside parent-agent workflows.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `agent_id` | `String` | Yes | Stable collaborator identity. |
| `name` | `String` | Yes | Speaker identity used in messages and trace. |
| `description` | `String` | Yes | Responsibilities and routing summary. |
| `template_id` | `Option<String>` | No | Source template when created from a template. |
| `state` | `SubAgentState` | Yes | Runtime availability/lifecycle state. |
| `capability_scope` | `CapabilityScope` | Yes | Effective capability scope. |
| `context_policy` | `ContextSharingPolicy` | Yes | Effective context-sharing policy. |
| `default_budget` | `DelegationBudget` | Yes | Invocation limits. |
| `metadata` | `serde_json::Value` | No | Safe non-sensitive metadata. |

**Validation Rules**:
- `name` must be non-empty and unique within a parent registry.
- `name` must be copied to `Msg.name` for messages authored by the SubAgent.
- Disabled SubAgents cannot be selected unless explicitly re-enabled.
- A SubAgent cannot claim capabilities outside its effective scope.

### 4. SubAgentState

| Variant | Description |
|---------|-------------|
| `Configured` | Registered but no current invocation. |
| `Selected` | Chosen for a delegation request. |
| `Running` | Currently processing a delegated task. |
| `Completed` | Last invocation completed successfully. |
| `Failed` | Last invocation failed. |
| `TimedOut` | Last invocation exceeded timeout. |
| `Cancelled` | Last invocation was cancelled. |
| `Disabled` | Not selectable. |

**State Transitions**:

```text
Configured ── select ──▶ Selected ── invoke ──▶ Running
Running ── success ───▶ Completed ── reset ───▶ Configured
Running ── failure ───▶ Failed    ── reset ───▶ Configured
Running ── timeout ───▶ TimedOut  ── reset ───▶ Configured
Running ── cancel ────▶ Cancelled ── reset ───▶ Configured
Configured/Selected ── disable ──▶ Disabled
Disabled ── enable ──▶ Configured
```

### 5. SubAgentRegistry

**Purpose**: The user-visible set of SubAgents and templates available to a parent agent.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `registry_id` | `String` | Yes | Registry identity, usually scoped to a parent agent or team. |
| `parent_agent_name` | `String` | Yes | Owner/primary agent speaker name. |
| `templates` | `Vec<SubAgentTemplate>` | Yes | Registered reusable templates. |
| `subagents` | `Vec<SubAgent>` | Yes | Registered concrete collaborators. |
| `selection_policy` | `SelectionPolicy` | Yes | Rules for explicit or assisted collaborator selection. |

**Validation Rules**:
- Duplicate names are errors, not warnings.
- Missing target names must return `MissingSubAgent`.
- Disabled targets must return `DisabledSubAgent`.
- Ambiguous automatic selection must return `AmbiguousSubAgent` unless a deterministic policy resolves it.

### 6. SelectionPolicy

| Variant | Description |
|---------|-------------|
| `ExplicitOnly` | Caller must select a target SubAgent by name or ID. |
| `ResponsibilityMatch` | System may select using responsibility descriptions and deterministic ranking. |
| `ManualApprovalRequired` | Selection suggestion must be confirmed by the caller/user. |

### 7. DelegationRequest

**Purpose**: The parent-created request that describes a subtask, shared context, limits, and correlation information for a SubAgent invocation.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `delegation_id` | `String` | Yes | Stable correlation ID. |
| `parent_agent_name` | `String` | Yes | Primary agent speaker identity. |
| `target_subagent_name` | `String` | Yes | Target collaborator speaker identity. |
| `task` | `String` | Yes | Delegated task instruction. |
| `context` | `SharedContext` | Yes | Context allowed by policy. |
| `budget` | `DelegationBudget` | Yes | Invocation limits. |
| `reply_mode` | `DelegationReplyMode` | Yes | Wait/final or streaming behavior. |
| `metadata` | `serde_json::Value` | No | Safe correlation metadata. |

**Validation Rules**:
- `task` must be non-empty.
- Target must exist, be enabled, and be within capability scope.
- Context must be derived according to `ContextSharingPolicy`.
- Budget must not exceed parent or registry limits.

### 8. SharedContext

| Field | Type | Description |
|-------|------|-------------|
| `messages` | `Vec<Msg>` | Explicitly shared message subset. |
| `summary` | `Option<String>` | Parent-provided context summary. |
| `memory_refs` | `Vec<String>` | References to memory entries made available. |
| `session_refs` | `Vec<String>` | Session references made available. |
| `workspace_refs` | `Vec<String>` | Workspace paths or artifact references made available. |
| `redaction_notes` | `Vec<String>` | Safe summary of content withheld or redacted. |

**Validation Rules**:
- Shared messages must preserve original `Msg.name` and `role`.
- Redacted content must not be reconstructable from default trace output.
- Empty shared context is valid if the delegated task is self-contained.

### 9. DelegationReplyMode

| Variant | Description |
|---------|-------------|
| `FinalOnly` | Parent waits for final SubAgent result. |
| `StreamEvents` | Parent receives correlated SubAgent events and final result. |
| `ObserveOnly` | Parent records output without immediately composing final response; must still observe terminal outcome. |

### 10. DelegationBudget

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_depth` | `u32` | `1` | Maximum nested SubAgent delegation depth. |
| `max_calls` | `u32` | `1` | Maximum SubAgent invocations for one parent task unless overridden. |
| `timeout_ms` | `u64` | implementation-defined | Invocation timeout. |
| `max_context_messages` | `usize` | implementation-defined | Maximum messages shared with a SubAgent. |
| `allow_concurrent` | `bool` | `false` | Whether multiple SubAgents may run concurrently. |

**Validation Rules**:
- `max_depth == 0` means delegation is not allowed.
- Exceeding depth or call count returns a stable budget error.
- Concurrent execution must not violate the target agent's single-reply guard.

### 11. ContextSharingPolicy

| Field | Type | Description |
|-------|------|-------------|
| `message_policy` | `MessageContextPolicy` | Controls none/summary/selected/full message sharing. |
| `memory_policy` | `ResourceSharingPolicy` | Controls memory visibility and promotion. |
| `session_policy` | `ResourceSharingPolicy` | Controls session state visibility and promotion. |
| `workspace_policy` | `ResourceSharingPolicy` | Controls workspace visibility and side effects. |
| `tool_policy` | `ResourceSharingPolicy` | Controls tool availability. |
| `promote_results_to_parent` | `bool` | Whether successful results are automatically observed by parent. |

### 12. MessageContextPolicy

| Variant | Description |
|---------|-------------|
| `None` | No prior parent messages shared. |
| `SummaryOnly` | Only parent-provided summary shared. |
| `Selected` | Explicit message subset shared. |
| `Full` | Full parent context shared; must be explicitly enabled. |

### 13. ResourceSharingPolicy

| Variant | Description |
|---------|-------------|
| `None` | Resource unavailable. |
| `ReadOnly` | Resource visible but side effects denied. |
| `Scoped` | Resource available only within explicit refs or prefixes. |
| `Inherited` | Inherits parent access; must be explicitly configured. |

### 14. CapabilityScope

| Field | Type | Description |
|-------|------|-------------|
| `tools` | `Vec<String>` | Tool names or groups available to the SubAgent. |
| `memory` | `ResourceSharingPolicy` | Memory operation permission. |
| `session` | `ResourceSharingPolicy` | Session operation permission. |
| `workspace` | `ResourceSharingPolicy` | Workspace operation permission. |
| `sandbox` | `ResourceSharingPolicy` | Sandbox/command execution permission. |
| `model_access` | `ModelAccessPolicy` | Model usage permission. |
| `side_effects` | `SideEffectPolicy` | Whether persistent side effects are allowed. |

### 15. CollaborationResult

**Purpose**: The attributable success or failure outcome returned by a SubAgent to the parent agent.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `delegation_id` | `String` | Yes | Correlates to request and trace. |
| `subagent_name` | `String` | Yes | Speaker identity of result producer. |
| `status` | `CollaborationStatus` | Yes | Terminal status. |
| `message` | `Option<Msg>` | No | Successful output message. |
| `error` | `Option<SubAgentErrorInfo>` | No | Typed failure details. |
| `trace_id` | `String` | Yes | Delegation trace reference. |
| `side_effects` | `Vec<SideEffectRecord>` | Yes | Memory/session/workspace/tool side effects attributed to the SubAgent. |

**Validation Rules**:
- `Succeeded` requires `message` and no terminal `error`.
- Failure/timeout/cancellation/permission/unsupported statuses require `error`.
- The result message `name` must equal `subagent_name`.

### 16. CollaborationStatus

| Variant | Description |
|---------|-------------|
| `Succeeded` | SubAgent completed and returned a valid result. |
| `Failed` | SubAgent execution failed. |
| `TimedOut` | Delegation exceeded timeout. |
| `Cancelled` | Parent or caller cancelled the work. |
| `PermissionDenied` | Scope/policy denied requested capability. |
| `UnsupportedFeature` | Requested pattern is outside supported scope. |

### 17. SubAgentErrorInfo

| Field | Type | Description |
|-------|------|-------------|
| `code` | `String` | Stable machine-readable code. |
| `category` | `SubAgentErrorCategory` | Typed category. |
| `message` | `String` | Redacted user-facing diagnostic. |
| `source` | `Option<String>` | Safe source summary, such as wrapped agent error category. |

### 18. SubAgentErrorCategory

| Variant | Description |
|---------|-------------|
| `InvalidTemplate` | Template failed validation. |
| `DuplicateSubAgent` | Duplicate name or ID. |
| `MissingSubAgent` | Target not registered. |
| `DisabledSubAgent` | Target disabled. |
| `AmbiguousSubAgent` | Automatic selection could not choose one target. |
| `InvalidDelegation` | Delegation request invalid. |
| `ExecutionFailure` | Underlying agent returned an execution error. |
| `Timeout` | Work timed out. |
| `Cancellation` | Work was cancelled. |
| `PermissionDenied` | Capability or context policy denied access. |
| `BudgetExceeded` | Depth/call/time/context budget exceeded. |
| `UnsupportedFeature` | Requested unsupported pattern. |
| `InternalError` | Framework invariant failure. |

### 19. MultiAgentConversation

**Purpose**: Ordered conversation record containing user, parent, and SubAgent messages with speaker attribution.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `conversation_id` | `String` | Yes | Stable conversation identity. |
| `participants` | `Vec<Participant>` | Yes | User, parent agent, and SubAgents. |
| `messages` | `Vec<Msg>` | Yes | Ordered messages; `Msg.name` preserves speaker identity. |
| `delegations` | `Vec<String>` | Yes | Delegation IDs associated with the conversation. |

**Validation Rules**:
- Every non-user message must reference a registered participant name.
- SubAgent messages must not be flattened into parent messages without preserving attribution.
- Message order must be reconstructable for deterministic trace comparison.

### 20. DelegationTrace

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `trace_id` | `String` | Yes | Stable trace identity. |
| `parent_reply_id` | `String` | Yes | Parent reply correlation ID. |
| `delegation_id` | `String` | Yes | Delegation correlation ID. |
| `events` | `Vec<DelegationEvent>` | Yes | Ordered delegation lifecycle events. |
| `redactions` | `Vec<String>` | Yes | Redaction summaries. |

### 21. DelegationEvent

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `sequence` | `u64` | Yes | Monotonic order within trace. |
| `event_type` | `DelegationEventType` | Yes | Lifecycle event type. |
| `agent_name` | `String` | Yes | Parent or SubAgent speaker identity. |
| `delegation_id` | `String` | Yes | Correlation ID. |
| `summary` | `String` | Yes | Redacted human-readable summary. |
| `error` | `Option<SubAgentErrorInfo>` | No | Error for terminal failure events. |

### 22. DelegationEventType

| Variant | Description |
|---------|-------------|
| `TemplateValidated` | Template passed validation. |
| `SubAgentRegistered` | SubAgent became available. |
| `DelegationRequested` | Parent created request. |
| `SubAgentSelected` | Target collaborator selected. |
| `SubAgentStarted` | Target began processing. |
| `SubAgentEventForwarded` | Underlying agent event was correlated to delegation. |
| `SubAgentCompleted` | Target completed successfully. |
| `SubAgentFailed` | Target failed. |
| `SubAgentTimedOut` | Target timed out. |
| `SubAgentCancelled` | Target was cancelled. |
| `ScopeDenied` | Capability/context policy denied access. |
| `ResultObservedByParent` | Parent observed or incorporated result. |

### 23. SideEffectRecord

| Field | Type | Description |
|-------|------|-------------|
| `effect_id` | `String` | Stable side-effect identity. |
| `subagent_name` | `String` | Producer attribution. |
| `effect_type` | `SideEffectType` | Memory/session/workspace/tool/model side effect. |
| `scope` | `String` | SubAgent-only, parent-promoted, or shared. |
| `summary` | `String` | Redacted summary. |

### 24. SideEffectType

| Variant | Description |
|---------|-------------|
| `MemoryWrite` | Memory created or modified. |
| `SessionUpdate` | Session state changed. |
| `WorkspaceWrite` | Workspace artifact created or modified. |
| `ToolInvocation` | Tool called. |
| `ModelCall` | Model interaction occurred. |
