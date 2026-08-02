# Feature Specification: SubAgent Collaboration

**Feature Branch**: `[020-subagent]`

**Created**: 2026-08-02

**Status**: Draft

**Input**: User description: "参考python版本的agentscope，实现subagent"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Delegate a task to a SubAgent and receive its result (Priority: P1)

As an AgentScope Rust user building a complex agent, I want a primary agent to delegate a well-scoped task to a SubAgent so that specialized work can be completed independently and returned as part of the primary conversation.

**Why this priority**: Delegation is the core user value of SubAgent support. Without a reliable parent-to-subagent request/response flow, higher-level multi-agent collaboration cannot be built or validated.

**Independent Test**: Can be fully tested by creating a primary agent with one configured SubAgent, sending a user request that requires delegation, and confirming that the SubAgent receives the delegated task, produces a result, and the primary agent incorporates that result into the final response.

**Acceptance Scenarios**:

1. **Given** a primary agent has a configured SubAgent with a name, description, and supported responsibility, **When** the primary agent receives a user request matching that responsibility, **Then** the task is routed to the SubAgent and the SubAgent result is made available to the primary agent.
2. **Given** a SubAgent completes successfully, **When** control returns to the primary agent, **Then** the final response includes the SubAgent's relevant result without exposing internal execution details that were not requested by the user.
3. **Given** the same deterministic task is delegated in a controlled test setup, **When** the trace is compared across runs, **Then** the delegated message flow, SubAgent lifecycle events, and final result are reproducible after normalizing non-deterministic identifiers.

---

### User Story 2 - Coordinate multiple SubAgents in one parent task (Priority: P2)

As a user building a richer multi-agent workflow, I want a primary agent to coordinate multiple SubAgents with distinct responsibilities so that a complex request can be decomposed into specialized subtasks while preserving a coherent parent conversation.

**Why this priority**: Python AgentScope users expect agent composition patterns that support multiple participating agents. Supporting more than one SubAgent validates that delegation is not a one-off shortcut but a reusable collaboration capability.

**Independent Test**: Can be tested by configuring at least two SubAgents with distinct responsibilities, sending a request that requires both, and confirming that each SubAgent receives only its intended subtask and the parent response combines their outputs coherently.

**Acceptance Scenarios**:

1. **Given** two configured SubAgents with non-overlapping responsibilities, **When** the primary agent delegates subtasks to both, **Then** each SubAgent receives its own scoped input and returns an independently attributable result.
2. **Given** multiple SubAgent results are returned, **When** the primary agent produces the final answer, **Then** the user can understand which results informed the answer without needing to inspect internal state.
3. **Given** one SubAgent is not applicable to the current user request, **When** delegation is considered, **Then** that SubAgent is not invoked and no empty or misleading result is fabricated.

---

### User Story 3 - Observe and debug SubAgent execution safely (Priority: P3)

As a maintainer or application developer, I want SubAgent execution to be observable through structured trace information so that I can diagnose routing, lifecycle, cancellation, and error behavior without leaking sensitive data.

**Why this priority**: SubAgent workflows introduce nested execution and more failure modes. Traceability is necessary for compatibility verification and for practical debugging in applications.

**Independent Test**: Can be tested by running a SubAgent scenario with tracing enabled and confirming that the trace records parent task start, SubAgent invocation, SubAgent completion or failure, returned result, cancellation state, and final parent response.

**Acceptance Scenarios**:

1. **Given** a SubAgent is invoked, **When** trace output is inspected, **Then** the trace includes the parent agent identity, SubAgent identity, delegated task summary, lifecycle status, and result status.
2. **Given** a SubAgent fails, times out, or is cancelled, **When** the primary agent handles the outcome, **Then** the trace records the typed failure category and the user receives an actionable non-secret error message.
3. **Given** SubAgent execution uses messages, tools, memory, session, or workspace context, **When** trace output is produced, **Then** sensitive values are redacted and scope boundaries are visible.

---

### User Story 4 - Preserve context and resource boundaries between parent and SubAgents (Priority: P4)

As an application owner, I want each SubAgent to receive the context and capabilities explicitly assigned to it so that delegation does not accidentally leak unrelated conversation history, memory, workspace access, tools, or session state.

**Why this priority**: Correct isolation is essential for safe multi-agent use, but the simplest valuable SubAgent feature can be delivered before advanced context policies are expanded.

**Independent Test**: Can be tested by configuring a SubAgent with limited context and capabilities, delegating a task, and confirming the SubAgent cannot observe or act on information outside its assigned scope.

**Acceptance Scenarios**:

1. **Given** a SubAgent is configured with a bounded context policy, **When** a task is delegated, **Then** the SubAgent receives only the user-visible task input and any explicitly shared supporting context.
2. **Given** the parent agent has tools or workspace permissions not granted to the SubAgent, **When** the SubAgent runs, **Then** those capabilities are unavailable to the SubAgent unless explicitly configured.
3. **Given** a SubAgent writes memory or session state, **When** the parent task completes, **Then** the write is either scoped to the SubAgent or explicitly promoted according to the configured sharing policy.

---

### Edge Cases

- A delegated task does not match any configured SubAgent responsibility.
- Multiple SubAgents appear equally suitable for the same task.
- A SubAgent returns no answer, malformed output, or an output that cannot be incorporated into the parent response.
- A SubAgent fails, times out, is cancelled, or is interrupted while the parent task is still active.
- The parent task is cancelled while one or more SubAgents are running.
- A SubAgent attempts to access tools, memory, session state, workspace resources, or credentials outside its assigned scope.
- A SubAgent recursively delegates to another agent and reaches the configured nesting or budget limit.
- Multiple SubAgents complete in a different order than they were invoked.
- A SubAgent emits streaming progress while the parent agent is also streaming.
- A SubAgent result contains sensitive data that should not be forwarded to the final user response or default trace output.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST allow a primary agent to register one or more SubAgents as named collaborators with human-readable descriptions of their responsibilities.
- **FR-002**: The system MUST provide reusable SubAgent templates that describe how a SubAgent should be created or configured for a team or parent-agent workflow.
- **FR-003**: The system MUST validate SubAgent templates before use and report missing names, responsibilities, capability scope, or required creation information as typed configuration errors.
- **FR-004**: The system MUST allow a primary agent to delegate a specific task to a selected SubAgent during an agent run.
- **FR-005**: The delegated task MUST include enough user-visible context for the SubAgent to complete the task while respecting configured context-sharing boundaries.
- **FR-006**: The system MUST return the SubAgent's result to the primary agent as an attributable collaboration result rather than as an indistinguishable user message.
- **FR-007**: The primary agent MUST be able to incorporate one or more SubAgent results into its final response.
- **FR-008**: The system MUST support at least one successful parent-to-SubAgent-to-parent execution cycle without requiring external orchestration by the application user.
- **FR-009**: The system MUST support multiple configured SubAgents in the same parent agent configuration.
- **FR-010**: The system MUST avoid invoking SubAgents that are not selected or applicable for the current delegated task.
- **FR-011**: The system MUST expose deterministic trace evidence for SubAgent invocation, delegated input summary, lifecycle status, returned result status, and parent response completion.
- **FR-012**: The system MUST preserve observable event ordering for parent and SubAgent lifecycles so compatibility tests can compare nested execution traces.
- **FR-013**: The system MUST preserve speaker identity for multi-agent conversations so user, primary agent, and each SubAgent message remain distinguishable in history, trace, and downstream formatting.
- **FR-014**: The system MUST propagate cancellation from the parent task to active SubAgent work within a bounded and observable time.
- **FR-015**: The system MUST apply timeout behavior to SubAgent work and return a typed timeout outcome to the parent agent when the limit is reached.
- **FR-016**: The system MUST represent SubAgent failures with typed, machine-readable errors that distinguish invalid delegation, SubAgent execution failure, timeout, cancellation, permission denial, unsupported capability, and internal framework failure.
- **FR-017**: The system MUST NOT fabricate successful SubAgent results when a SubAgent was not invoked, failed, timed out, or returned unsupported output.
- **FR-018**: The system MUST enforce configured boundaries for SubAgent access to tools, memory, session state, workspace resources, and other capabilities.
- **FR-019**: The system MUST provide a clear default context-sharing policy that prevents accidental leakage of unrelated parent conversation history or sensitive capability access.
- **FR-020**: The system MUST support deterministic tests using controlled model, tool, clock, and identifier behavior rather than relying solely on live model text.
- **FR-021**: The system MUST document all observable compatibility differences from the locked Python AgentScope reference behavior, including unsupported SubAgent patterns.
- **FR-022**: The system MUST expose unsupported SubAgent patterns through stable `UnsupportedFeature` outcomes rather than silent no-op behavior or misleading success.
- **FR-023**: The system MUST support user-facing examples that demonstrate successful delegation, multi-SubAgent coordination, cancellation or timeout handling, and safe trace inspection.
- **FR-024**: The system MUST ensure default trace and error output does not expose API keys, credentials, raw secrets, or unnecessary sensitive conversation content.
- **FR-025**: The system MUST define how SubAgent-generated memory, session changes, tool results, and workspace side effects are attributed and scoped.
- **FR-026**: The system MUST define a maximum supported delegation nesting depth or budget policy and return a stable error when that policy is exceeded.
- **FR-027**: The system MUST preserve compatibility with existing single-agent behavior when no SubAgents are configured.
- **FR-028**: The system MUST clearly report duplicate SubAgent names, missing SubAgents, disabled SubAgents, and invalid delegation targets without falling back to an unrelated collaborator.

### Compatibility Scope

- **Target compatibility level**: L2 Core Behavior Compatibility for SubAgent delegation lifecycle and trace behavior, with L3 Public API Semantic Compatibility where Python AgentScope exposes stable SubAgent-facing semantics that can be represented idiomatically.
- **Python reference baseline**: The locked Python AgentScope version defined by the project's compatibility baseline remains the behavioral source of truth for observable SubAgent semantics.
- **Supported in this feature**: Parent-to-SubAgent delegation, reusable SubAgent templates, in-process SubAgent registration and selection, SubAgent result return, multiple configured SubAgents, speaker identity preservation for multi-agent histories, lifecycle tracing, cancellation, timeout, typed errors, and scoped capability access.
- **Out of scope for this feature**: Distributed multi-process agent runtime, remote worker scheduling, durable external queues, full application service compatibility, provider-specific multi-agent formatter parity, autonomous long-running agent swarms, and cross-host SubAgent migration.

### Input/Output Contract

- **SubAgent Definition**: Identifies a collaborator available to a primary agent, including stable name, role description, responsibility summary, and configured capability scope.
- **SubAgent Template**: A reusable blueprint for creating or configuring a SubAgent in a team or parent-agent workflow, including name, description, instructions, required capabilities, and validation status.
- **Delegated Task**: The parent-selected task sent to a SubAgent, including task instructions, relevant context summary, correlation identity, and execution limits.
- **SubAgent Result**: The outcome returned to the primary agent, including success or failure status, result content or typed error, attribution, and safe trace metadata.
- **Multi-Agent Conversation**: A conversation history containing messages from the user, primary agent, and one or more SubAgents while preserving each speaker's identity.
- **Parent Response**: The user-facing response produced after the parent agent evaluates any SubAgent results.
- **Trace Record**: The structured observable record of parent and SubAgent lifecycle events, including invocation, completion, failure, cancellation, timeout, and final response.

### Lifecycle Requirements

- A SubAgent MUST move through observable lifecycle states equivalent to: configured, selected, invoked, running, completed, failed, timed out, or cancelled.
- A parent task MUST retain ownership of SubAgent work it starts and MUST be able to observe every terminal SubAgent outcome.
- A completed SubAgent result MUST be returned to the parent task before the parent final response claims to use it.
- A cancelled parent task MUST prevent orphaned SubAgent work from continuing beyond the configured cancellation window.

### Concurrency and Ordering Requirements

- If multiple SubAgents are invoked for one parent task, the system MUST define whether they are executed sequentially or concurrently for that run and MUST make the observed order clear in trace output.
- SubAgent completion order MAY differ from invocation order when concurrent execution is supported, but trace output MUST preserve enough sequence information to reconstruct what happened.
- Parent response completion MUST occur after all required SubAgent outcomes for that response have reached a terminal state.
- Backpressure or capacity limits MUST produce observable waiting, rejection, timeout, or cancellation outcomes rather than unbounded hidden work.

### Key Entities *(include if feature involves data)*

- **Primary Agent**: The agent that receives the user request, decides whether to delegate, owns SubAgent work, and produces the final user-facing response.
- **SubAgent**: A configured collaborator agent with a name, responsibility description, context policy, capability scope, and lifecycle within a parent task.
- **SubAgent Template**: A reusable declaration for creating or configuring SubAgents consistently across parent-agent workflows or teams.
- **SubAgent Registry**: The user-visible set of registered SubAgents and templates available to a parent agent, including lookup, duplicate-name handling, and disabled or missing collaborator states.
- **Delegation Request**: The parent-created request that describes the subtask, shared context, execution limits, and correlation information for a SubAgent invocation.
- **Collaboration Result**: The attributable success or failure outcome returned by a SubAgent to the parent agent.
- **Multi-Agent Conversation**: The ordered conversation record containing user, primary agent, and SubAgent messages with preserved speaker attribution.
- **Context Sharing Policy**: The rule set controlling which parent messages, memory, session data, workspace resources, tools, and metadata are visible to a SubAgent.
- **Capability Scope**: The set of tools, memory operations, session operations, workspace permissions, model access, and side-effect permissions available to a SubAgent.
- **Delegation Trace**: The structured record used to verify nested agent execution, event order, errors, cancellation, and final response composition.
- **Delegation Budget**: The configured limit for nesting depth, number of SubAgent calls, elapsed time, or resource use during a parent task.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A developer can configure one primary agent with one SubAgent and complete a deterministic delegated task in under 10 minutes using project documentation and examples.
- **SC-002**: Deterministic compatibility tests verify at least 6 core SubAgent traces: template validation, successful single delegation, multiple SubAgents, SubAgent failure, timeout or cancellation, and scope-denied access.
- **SC-003**: 100% of SubAgent terminal outcomes are represented as success, typed failure, timeout, cancellation, permission denial, or unsupported feature; no terminal outcome is silently ignored.
- **SC-004**: Parent cancellation stops all active SubAgent work within the configured cancellation window in deterministic tests.
- **SC-005**: A trace reviewer can reconstruct parent invocation, each SubAgent invocation, each terminal SubAgent outcome, speaker identity for every multi-agent message, and final parent response order from a single trace without reading internal state.
- **SC-006**: Existing single-agent scenarios continue to pass without behavior changes when no SubAgents are configured.
- **SC-007**: Default SubAgent traces and user-facing errors contain zero raw secret values in validation scenarios with representative credential-like inputs.
- **SC-008**: All supported and unsupported SubAgent patterns are recorded in the compatibility matrix with their target compatibility level and any documented deviation from Python AgentScope.

## Assumptions

- The primary user is a developer or maintainer building multi-agent applications on AgentScope Rust.
- "SubAgent" means an agent configured as a subordinate collaborator of a parent agent for a bounded task, not a distributed worker process or independent long-running service.
- SubAgent template support is interpreted as a reusable in-process configuration and creation blueprint; full Python application-service behavior is deferred unless explicitly planned later.
- The Python AgentScope reference implementation remains the source of truth for externally observable behavior, but Rust may use idiomatic internal structures as long as visible behavior remains compatible.
- This feature builds on existing Agent, Tool, Streaming, Memory, Session, Workspace, Sandbox, and trace capabilities already present in the project.
- Live model behavior is useful for examples but deterministic model and tool behavior is required for compatibility acceptance.
- Safe defaults should prioritize explicit context sharing and least-privilege capability scope over convenience.
- Advanced distributed runtime behavior and complete provider-specific multi-agent formatting parity are reserved for later features and should not be implicitly introduced here.
