# Feature Specification: Agent System

**Feature Branch**: `007-agent-system`

**Created**: 2026-07-29

**Status**: Draft

**Input**: User description: "Agent System — AgentBase trait, ReActAgent (reasoning→acting loop), hooks/middleware integration, memory context management. Builds on agent_scope_model, agent_scope_tool, agent_scope_state, agent_scope_message, agent_scope_event. Python upstream reference: agentscope/src/agentscope/agent/"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Create a Basic Text Agent (Priority: P1)

A developer wants to create a simple conversational agent that receives user messages, calls an LLM, and returns a text reply. The agent should handle the full reply lifecycle: receiving input, making model calls, emitting events, and producing the final response message.

**Why this priority**: This is the minimum viable agent — it validates that all 6 foundation crates (model, tool, state, message, event, types) integrate correctly through the agent orchestration layer. Without this, nothing else works.

**Independent Test**: Create an agent with a MockModel that returns a fixed text response. Send a user message and verify: (a) the correct sequence of events (ReplyStart → ModelCallStart → ModelCallEnd → TextBlockStart → TextBlockDelta → TextBlockEnd → ReplyEnd) is emitted, (b) the final Msg contains the expected content, (c) the AgentState records the reply context correctly.

**Acceptance Scenarios**:

1. **Given** an agent bound to a mock model that echoes the input, **When** the developer calls `agent.reply(user_msg("Hello"))`, **Then** the agent emits events in the order: ReplyStart → ModelCallStart → ModelCallEnd → TextBlockStart → TextBlockDelta → TextBlockEnd → ReplyEnd, and returns a Msg with role=assistant.
2. **Given** an agent with no input provided (None), **When** `agent.reply(None)` is called with messages already in state context, **Then** the agent proceeds using the existing context without appending new input.
3. **Given** an agent with an empty state (no prior context), **When** `agent.reply(None)` is called, **Then** the agent returns an error indicating no content to reply to.

---

### User Story 2 - ReAct Agent with Tool Calls (Priority: P2)

A developer wants an agent that can reason about a task, decide which tools to call, execute those tools, and incorporate the results into its response. This is the standard ReAct (Reasoning + Acting) loop.

**Why this priority**: Tool usage is the defining characteristic of an AI agent beyond a simple chatbot. This validates the tool-call lifecycle within the agent loop (ToolCallStart → ToolCallEnd → ToolResultStart → ToolResultEnd) and tests the iterative reasoning-acting pattern.

**Independent Test**: Create a ReActAgent with a MockModel that first returns a tool_call for "calculator" then returns a text response. Register a calculator tool. Verify: (a) the agent detects the tool call from the model response, (b) executes the tool, (c) sends the tool result back to the model, (d) emits the full tool lifecycle events, (e) produces the final text response.

**Acceptance Scenarios**:

1. **Given** a ReActAgent with a tool that computes `a + b`, and a mock model that first returns `tool_call("calculator", {"a": 1, "b": 2})` then `"The answer is 3"`, **When** `agent.reply(user_msg("What is 1+2?"))` is called, **Then** the agent emits: TextBlockStart→...→ToolCallStart→ToolCallEnd→ToolResultStart→ToolResultEnd→TextBlockStart→...→TextBlockEnd→ReplyEnd, and the final message contains "The answer is 3".
2. **Given** a ReActAgent with `max_iters=1` and a mock model that insists on calling a tool in every response, **When** the max iterations are exceeded, **Then** the agent emits an ExceedMaxItersEvent and returns an assistant message with the last model response.
3. **Given** a ReActAgent processing a tool call that results in a ToolError, **When** the tool execution fails, **Then** the agent emits ToolResultEnd with state=execution_error and feeds the error back to the model for continued reasoning.

---

### User Story 3 - Hook/Middleware Integration (Priority: P3)

A developer wants to intercept agent behavior at specific hook points without modifying the agent's source code. For example, logging every model call, modifying the system prompt before each reply, or approving tool executions before they run.

**Why this priority**: Middleware is the extensibility mechanism of AgentScope. While the agent works without it, middleware enables enterprise features like audit logging, content filtering, and custom permission policies.

**Independent Test**: Create an agent with a middleware that logs `pre_reply` and `post_reply` invocations. Trigger a reply and verify both hook points fire. Then test each remaining hook point independently (pre_reasoning, post_reasoning, pre_acting, post_acting, pre_observe, post_observe, pre_print, post_print).

**Acceptance Scenarios**:

1. **Given** an agent registered with a middleware implementing `pre_reply` and `post_reply`, **When** `agent.reply(msg)` is called, **Then** `pre_reply` is invoked before the model call, and `post_reply` is invoked after the reply completes (success or error).
2. **Given** a ReActAgent with a middleware implementing `pre_acting`, **When** a tool call is about to be executed, **Then** `pre_acting` fires with the tool name and arguments, and the middleware can modify or reject the tool execution.
3. **Given** an agent with a middleware implementing `pre_observe`, **When** `agent.observe(msgs)` is called, **Then** `pre_observe` fires with the incoming messages before they are appended to state context.

---

### User Story 4 - Interruption and Cancellation (Priority: P4)

An application orchestrating agent conversations needs to interrupt a long-running agent reply (e.g., user clicks "stop") and optionally continue the conversation afterward.

**Why this priority**: Real-world agent deployments require graceful interruption. This is critical for user-facing applications but can be deferred until the core agent loop is validated.

**Independent Test**: Start a long-running reply with a mock model that yields slowly. Interrupt with a UserInterruptEvent. Verify: (a) the agent emits UserInterruptEvent → ToolResultEnd(for pending tools) → ReplyEnd with finished_reason=interrupted, (b) the reply returns the interruption message. Then send a follow-up message to verify the agent can continue.

**Acceptance Scenarios**:

1. **Given** a ReActAgent in the middle of tool execution, **When** a UserInterruptEvent is sent, **Then** the agent marks all pending tool calls as interrupted, emits ReplyEnd with finished_reason=interrupted, and returns the configured interruption message.
2. **Given** an interrupted agent that returned an interruption message, **When** a new user message is sent as a follow-up reply, **Then** the agent resumes normally with the new message context.

---

### Edge Cases

- What happens when the model returns an empty response (no content blocks)?
- What happens when the model returns a response with only a tool_call but the agent has no tools registered?
- What happens when context length exceeds the model's context window before compression is triggered?
- What happens when a middleware panics during hook execution?
- What happens when context compression is triggered but the compression model fails?
- What happens when `observe()` is called while a reply is in progress?

## Requirements *(mandatory)*

### Functional Requirements

**Agent trait (core interface)**:

- **FR-001**: System MUST define an `Agent` trait with methods: `reply()`, `reply_stream()`, `observe()`, `name()`, `state()`.
- **FR-002**: `reply()` MUST accept input as `Msg | Vec<Msg> | None` and return a `Result<Msg, AgentError>` representing the final assistant message.
- **FR-003**: `reply_stream()` MUST accept the same input types as `reply()` and return a `Stream` yielding `AgentEvent` items (and optionally the final `Msg`).
- **FR-004**: `observe()` MUST accept `Msg | Vec<Msg> | None` and append the messages to the agent's state context without triggering a reply.
- **FR-005**: Agent MUST own an `AgentState` (from `agent_scope_state`), creating a default one if not provided.

**ReActAgent (reasoning-acting loop)**:

- **FR-006**: `ReActAgent` MUST implement the `Agent` trait and provide a reasoning-acting loop.
- **FR-007**: The reasoning step MUST call the bound `ChatModel` with the current context messages and available tools.
- **FR-008**: The acting step MUST execute tool calls returned by the model and feed results back into context.
- **FR-009**: The loop MUST respect `max_iters` from `ReActConfig`, emitting `ExceedMaxItersEvent` and returning the last model response when exceeded.
- **FR-010**: The agent MUST emit events in the order defined by AgentScope protocol: ReplyStart → (ModelCallStart → ModelCallEnd → [streaming blocks]) → (ToolCallStart → ToolCallEnd → ToolResultStart → ToolResultEnd)* → ... → ReplyEnd.
- **FR-011**: `ReActAgent` MUST support structured output via `structured_schema` parameter, using the tool-calling bypass mechanism already defined in `ChatModel::generate_structured_output()`.

**Agent configuration**:

- **FR-012**: System MUST define `AgentConfig` containing: `name` (String), `system_prompt` (String), `model` (Arc<dyn ChatModel>), `toolkit` (Option<ToolKit>).
- **FR-013**: System MUST define `ReActConfig` with: `max_iters` (u32, default 20), `stop_on_reject` (bool, default false), `interruption_message` (String).
- **FR-014**: `ReActAgent` MUST accept optional `middlewares: Vec<Arc<dyn Middleware>>` and dispatch hooks at each defined hook point.
- **FR-015**: Config validation MUST reject invalid combinations: e.g., `structured_output_grace_iters` must be > 0, `reserve_ratio` must be < `trigger_ratio`.

**Hook/Middleware system**:

- **FR-016**: System MUST define a `Middleware` trait with 8 optional hook methods matching the hook constants in `agent_scope_types::hook`: `on_reply`, `on_reasoning`, `on_acting`, `on_observe`, `on_print`, `on_model_call`, `on_system_prompt`, `on_compress_context`.
- **FR-017**: Each hook method MUST default to no-op, so middleware can implement only the hooks they need.
- **FR-018**: ReActAgent MUST invoke `pre_reply` / `post_reply` hooks around the entire reply flow, and `pre_reasoning` / `post_reasoning` / `pre_acting` / `post_acting` hooks around their respective steps.
- **FR-019**: Hooks MUST be invoked in registration order (FIFO) per hook point.

**Context compression**:

- **FR-020**: ReActAgent MUST monitor context length against `ContextConfig::trigger_ratio` before each model call.
- **FR-021**: When context exceeds the trigger threshold, the agent MUST invoke `compress_context()` which calls a model to summarize older messages, replacing them with a SummaryContent block.
- **FR-022**: `ContextConfig` MUST define: `trigger_ratio` (f64, 0< <0.9, default 0.8), `reserve_ratio` (f64, default 0.1).

**Interruption**:

- **FR-023**: ReActAgent MUST handle `UserInterruptEvent` by canceling pending tool calls, emitting `ReplyEnd(finished_reason=interrupted)`, and returning the configured interruption message.
- **FR-024**: The agent MUST NOT enter the reasoning-acting loop when interrupted; it MUST produce the interruption response and exit.

**Tool permission checking**:

- **FR-025**: Before executing a tool call, the agent MUST check permissions via `PermissionEngine` (from `agent_scope_state::permission`).
- **FR-026**: If permission is denied, the agent MUST emit a `RequireUserConfirmEvent` and wait for external confirmation (unless `stop_on_reject` is true, in which case the agent stops).

### Key Entities

- **Agent (trait)**: The common interface for all agent types. Methods: `reply()`, `reply_stream()`, `observe()`. Holds a reference to AgentState.
- **ReActAgent**: The primary agent implementation. Combines a ChatModel, a ToolKit, optional Middlewares, and configuration (ReActConfig, ContextConfig, InjectionConfig, ModelConfig) to drive the reasoning-acting loop.
- **AgentConfig**: Constructor configuration — `name`, `system_prompt`, `model`, `toolkit`.
- **ReActConfig**: Loop configuration — `max_iters`, `stop_on_reject`, `interruption_message`, `structured_output_grace_iters`.
- **ContextConfig**: Memory management — `trigger_ratio`, `reserve_ratio`, `compression_prompt`, `tool_result_limit`.
- **Middleware (trait)**: Extension hook interface with 8 optional methods mapping to `agent_hooks` + `react_agent_hooks` constants.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A developer can create a basic text agent with fewer than 10 lines of setup code (instantiate model, create agent, call reply).
- **SC-002**: The agent emits the correct sequence of 10+ event types for a complete ReAct cycle (input → reasoning → tool call → tool result → reasoning → text response), in the exact order defined by the AgentScope protocol.
- **SC-003**: All 8 middleware hook points can be intercepted independently without modifying agent source code.
- **SC-004**: An interrupted agent returns control to the caller within a configurable grace period (default 5 seconds) after receiving an interrupt signal.
- **SC-005**: Context compression reduces token count to within configurable bounds while preserving task-relevant information.
- **SC-006**: The agent loop completes at least 3 reasoning-acting iterations without state corruption when the model alternates between tool calls and text responses.

## Assumptions

- The `Middleware` trait definition lives in a new crate `agent_scope_agent` alongside `Agent` and `ReActAgent` — no separate middleware crate for this feature.
- `PermissionEngine` already exists in `agent_scope_state::permission` (defined as a stub) and will be fully implemented in this feature.
- Mock/scripted model implementations (MockModel, ScriptedModel) exist in test utilities and are NOT part of this feature's deliverable — they are test infrastructure.
- Context compression uses the agent's own model unless an alternative compression model is configured in `ContextConfig`.
- The initial implementation focuses on a single ReActAgent type. Other agent types (e.g., multi-agent orchestrators) are out of scope for this feature.
- Runtime state injection (time, tasks, context length) via `InjectionConfig` is deferred to a future feature (007-agent-system-advanced or similar), as it depends on the basic agent loop being stable first.
- Streaming output (yielding AgentEvent items from `reply_stream()`) is supported at the API level but full streaming from the model through the agent is scoped here; the event yield mechanism is required for US1 completeness.
- All 8 hook constants defined in `agent_scope_types::hook` remain as-is; this feature implements the middleware trait and dispatch mechanism that uses them.
