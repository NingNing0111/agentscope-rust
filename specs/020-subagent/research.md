# Research: SubAgent Collaboration

**Feature**: 020-subagent | **Date**: 2026-08-02

## 1. Feature boundary and compatibility target

### Decision

Feature 020 implements in-process SubAgent collaboration: a primary agent can register reusable SubAgent templates or concrete SubAgents, delegate bounded tasks, receive attributable results, preserve speaker identity, and expose traceable lifecycle outcomes. Distributed runtime, remote workers, durable queues, full Python app-service compatibility, and provider-specific multi-agent formatter parity are explicitly deferred.

### Rationale

- The Python compatibility baseline exposes `agentscope.app._types.SubAgentTemplate`, app service run triggers, event projection, message bus references, and multiple `*MultiAgentFormatter` symbols, but the Rust project currently has no direct SubAgentTemplate, message hub, or handoff implementation.
- Current Rust `Agent` trait already supports `reply`, `reply_stream`, `observe`, `name`, and `state`, making in-process collaboration the smallest useful step that remains compatible with existing architecture.
- The project constitution requires small-step delivery and forbids pseudo-compatibility; claiming FastAPI service, distributed message bus, or full provider formatter compatibility in this feature would overstate the current scope.

### Alternatives Considered

1. **Implement full Python app service and message bus now**: Rejected — too broad, mixes SubAgent with distributed/application runtime, and risks violating small-step delivery.
2. **Only document SubAgent as unsupported**: Rejected — the existing Agent/Msg/Event abstractions are sufficient for a meaningful in-process MVP.
3. **In-process SubAgent collaboration with explicit deferred boundaries**: Adopted — delivers user value while preserving compatibility honesty.

## 2. Placement in crate architecture

### Decision

Add SubAgent collaboration to `agent_scope_agent` as an agent-layer capability, with optional integrations to existing `agent_scope_message`, `agent_scope_event`, `agent_scope_state`, `agent_scope_tool`, `agent_scope_memory`, `agent_scope_workspace`, and `agent_scope_sandbox` concepts through already defined abstractions. Do not create a provider-specific or runtime-specific crate for this feature.

### Rationale

- SubAgent collaboration is about composing agents, not introducing a new provider, storage backend, or distributed runtime.
- `agent_scope_agent` already owns `Agent`, `ReActAgent`, middleware, permission integration, cancellation, and streaming behavior.
- Existing dependency direction allows `agent_scope_agent` to depend on message/event/state/tool/memory/types while preventing core/provider pollution.
- Keeping the first API in the agent crate makes `Arc<dyn Agent>` the central extension point and avoids duplicating the reasoning loop.

### Alternatives Considered

1. **New `agent_scope_multi_agent` crate**: Deferred — may be appropriate for a larger distributed/team runtime, but overkill for in-process delegation.
2. **Implement inside provider formatter crates**: Rejected — SubAgent lifecycle is not provider-specific, and formatter parity is deferred.
3. **Implement inside `agent_scope_agent`**: Adopted — closest to the existing `Agent` abstraction and minimal dependency impact.

## 3. SubAgent template semantics

### Decision

Represent `SubAgentTemplate` as a reusable declaration for constructing or registering a SubAgent collaborator. It must include a stable name, responsibility description, instructions or creation metadata, capability scope, context sharing policy, and validation status. The template is configuration, not a running dispatcher.

### Rationale

- Python inventory describes `SubAgentTemplate` as “a reusable blueprint for sub-agent creation within a team” and marks it as structure-like rather than a standalone runtime behavior.
- Treating the template as a blueprint allows alignment with Python semantics while using Rust-native construction and validation.
- Validation is essential for deterministic errors: missing name, duplicate name, missing responsibility, and invalid capability scope should fail before delegation.

### Alternatives Considered

1. **Template as serialized Python-compatible service object**: Rejected for this feature — full app-service compatibility is out of scope.
2. **No template; only register concrete agents**: Rejected — misses a named Python compatibility concept.
3. **Rust-native template blueprint with validation**: Adopted.

## 4. Delegation execution model

### Decision

Use an explicit `DelegationRequest` and `CollaborationResult` flow. The primary agent or collaborator manager selects a SubAgent, sends a bounded task plus context according to policy, awaits a terminal result, and returns success or typed failure to the parent. Multiple SubAgents may be supported sequentially first; concurrency must be explicit and observable if enabled.

### Rationale

- The feature spec requires parent ownership, terminal outcome observation, timeout, cancellation, and no fabricated success.
- Existing `ReActAgent` rejects overlapping `reply`/`reply_stream` with `AlreadyStreaming`; a delegation model must respect this rather than hide it.
- Sequential default behavior is simpler, deterministic, and sufficient for P1/P2 validation. A later concurrent mode can preserve event sequence numbers and correlation IDs.

### Alternatives Considered

1. **Implicit LLM-only tool call that invokes SubAgents**: Rejected — hard to validate and obscures lifecycle semantics.
2. **Fire-and-forget SubAgent triggers**: Rejected — parent must observe terminal outcomes for correctness and cancellation.
3. **Explicit request/result lifecycle**: Adopted.

## 5. Context sharing and capability scope

### Decision

Default to least privilege: a SubAgent receives only delegated task instructions and explicitly shared context. Access to tools, memory, session state, workspace resources, sandbox behavior, and promotion of SubAgent side effects must be governed by explicit `ContextSharingPolicy` and `CapabilityScope` settings.

### Rationale

- The spec requires preventing accidental leakage of unrelated parent history or sensitive capability access.
- Existing memory/session/workspace/sandbox modules have their own boundaries; SubAgent should compose those rather than bypassing them.
- Default isolation makes security and deterministic testing easier.

### Alternatives Considered

1. **Always share full parent context**: Rejected — too much leakage and difficult to reason about.
2. **Never share context beyond the task text**: Rejected — too limiting for useful collaboration.
3. **Explicit policy with least-privilege default**: Adopted.

## 6. Speaker identity and multi-agent conversation history

### Decision

Use `Msg.name` as the stable speaker identity and preserve it in all multi-agent conversation records, delegation traces, and result attribution. User, primary agent, and each SubAgent must remain distinguishable even when messages are later formatted for models.

### Rationale

- Python baseline includes multiple `*MultiAgentFormatter` symbols, implying speaker identity affects observable provider input behavior.
- Rust `Msg` already has a `name` field documented as sender/agent name.
- Provider-specific multi-agent formatting is deferred, but losing speaker identity now would block future formatter compatibility.

### Alternatives Considered

1. **Flatten SubAgent output into primary assistant text immediately**: Rejected — loses attribution and harms future compatibility.
2. **Store speaker identity only in trace metadata**: Rejected — message history itself must preserve identity.
3. **Use `Msg.name` consistently**: Adopted.

## 7. Eventing and trace design

### Decision

Introduce stable delegation-level trace records and, if needed, explicit agent-layer events for SubAgent invocation, result, failure, timeout, cancellation, and scope denial. Existing `AgentEvent` streams from SubAgents must be correlated with parent delegation using parent reply ID, SubAgent name, delegation ID, and sequence information.

### Rationale

- Constitution requires trace as a core acceptance artifact.
- Existing `AgentEvent` covers reply/model/tool lifecycle but does not by itself identify a delegation boundary.
- Correlation avoids confusing interleaved parent/SubAgent events and allows deterministic compatibility tests.

### Alternatives Considered

1. **Rely only on logging**: Rejected — logs are not a stable contract.
2. **Do not expose SubAgent internals**: Rejected — debugging nested execution would be impractical.
3. **Structured delegation trace with safe summaries**: Adopted.

## 8. Error, timeout, and cancellation semantics

### Decision

Define typed SubAgent errors that map to stable categories: invalid template, duplicate SubAgent, missing SubAgent, disabled SubAgent, invalid delegation target, execution failure, timeout, cancellation, permission denied, unsupported feature, and internal framework failure. Parent cancellation must propagate to active SubAgent work within the configured cancellation window.

### Rationale

- The feature spec requires no fabricated success and 100% terminal outcomes.
- Existing `AgentError` already contains categories such as `AlreadyStreaming` and cancellation-related errors; SubAgent errors can wrap or map these without string matching.
- Timeout and cancellation must be observable in trace and returned to parent logic.

### Alternatives Considered

1. **Return plain text errors in CollaborationResult**: Rejected — violates stable error model.
2. **Panic on invalid registry/delegation state**: Rejected — violates safe Rust and long-running agent expectations.
3. **Typed error categories with redacted messages**: Adopted.

## 9. Test strategy

### Decision

Use deterministic unit and integration tests with scripted/mock agents, fixed IDs, fixed clocks where applicable, and controlled errors. Required trace scenarios: template validation, successful single delegation, multiple SubAgents, SubAgent failure, timeout or cancellation, and scope-denied access. Live model examples are optional and not acceptance-critical.

### Rationale

- Constitution requires deterministic compatibility tests and forbids relying solely on live LLM text.
- Existing project patterns already use cargo tests, clippy, fmt, and event sequence validation.
- Mock/scripted agents allow precise verification of ordering, attribution, and error categories.

### Alternatives Considered

1. **Validate through live DashScope only**: Rejected — non-deterministic and environment-dependent.
2. **Only unit-test data structures**: Rejected — misses parent-to-SubAgent lifecycle behavior.
3. **Deterministic unit + integration + quickstart scenarios**: Adopted.
