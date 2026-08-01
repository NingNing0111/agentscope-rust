# Feature Specification: Complete Agent Demo

**Feature Branch**: `[019-agent-demo]`

**Created**: 2026-08-01

**Status**: Draft

**Input**: User description: "编写完整的Agent 示例，放到examples/agent-demo， 把所有功能体现在这个demo里"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Run a complete single-agent walkthrough (Priority: P1)

As a developer evaluating AgentScope Rust, I want to run one complete Agent demonstration from `examples/agent-demo` so that I can see the end-to-end Agent experience without assembling multiple scattered examples.

**Why this priority**: This is the core user value: a complete demo must provide a clear, runnable entry point that proves the Agent workflow can be understood and evaluated quickly.

**Independent Test**: Can be fully tested by opening the demo instructions, running the primary demo scenario, and confirming that the walkthrough shows model interaction, reasoning progress, tool use, memory/context behavior, and final output in one coherent flow.

**Acceptance Scenarios**:

1. **Given** a developer has a prepared local environment and any required credentials or mock configuration, **When** they follow the primary demo instructions, **Then** the demo completes successfully and prints a clear summary of the Agent's actions and final answer.
2. **Given** a developer is reading the demo without running it, **When** they inspect the demo documentation and inline explanations, **Then** they can understand what capabilities are demonstrated, what inputs are required, and what output to expect.
3. **Given** external model access is unavailable, **When** the developer chooses the documented offline or deterministic path, **Then** the demo still illustrates the main Agent workflow without depending on unpredictable live model text.

---

### User Story 2 - Observe every major framework capability in context (Priority: P2)

As a developer learning the project, I want the demo to present the major implemented AgentScope capabilities together so that I can understand how the pieces work as an integrated system rather than as isolated APIs.

**Why this priority**: The user specifically asked to reflect all capabilities in the demo; integrated coverage makes the example useful as a capability showcase and regression reference.

**Independent Test**: Can be tested by reviewing the demo output and checklist of demonstrated capabilities, confirming that each major area is either exercised in the main scenario or explicitly marked as a documented optional scenario.

**Acceptance Scenarios**:

1. **Given** the demo starts normally, **When** the primary scenario runs, **Then** it demonstrates Agent interaction, structured messages, events, streaming-visible progress, tool invocation, session continuity, memory/context usage, and observable trace output.
2. **Given** capabilities that require special environment support, **When** the demo cannot safely exercise them by default, **Then** it documents the capability, explains the requirement, and provides an explicit opt-in path or a clear unsupported message.
3. **Given** the demo output is reviewed after completion, **When** a developer maps the output to the capability checklist, **Then** each demonstrated capability has visible evidence in logs, terminal output, generated artifacts, or documented expected behavior.

---

### User Story 3 - Use the demo as a reliable learning and regression artifact (Priority: P3)

As a maintainer, I want the complete Agent demo to be understandable, repeatable, and easy to validate so that future changes can preserve the documented user experience.

**Why this priority**: A showcase demo becomes valuable long-term only if it can be validated and maintained as the framework evolves.

**Independent Test**: Can be tested by running the demo validation path and confirming that the documented scenario completes consistently, reports errors clearly, and avoids exposing sensitive information.

**Acceptance Scenarios**:

1. **Given** a maintainer runs the demo validation instructions, **When** the validation completes, **Then** it confirms that the demo builds or starts, runs the deterministic scenario, and produces the expected high-level trace.
2. **Given** required configuration is missing, **When** the demo starts, **Then** it reports actionable setup guidance rather than failing with an opaque error.
3. **Given** the demo emits logs or trace output, **When** the output is inspected, **Then** it does not expose secrets, raw credentials, or unnecessary sensitive conversation content by default.

---

### Edge Cases

- Required model credentials, endpoint configuration, or optional runtime support are missing.
- Network access is unavailable or a live provider returns an error during an optional live run.
- A tool call receives invalid input or fails during execution.
- Streaming output is interrupted, cancelled, or finishes with partial progress already displayed.
- Session or memory state from a previous run exists and could affect repeatability.
- Optional capabilities cannot be demonstrated on the current platform or environment.
- Demo output includes errors; the user must still be able to identify which capability failed and what to try next.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The project MUST provide a complete Agent demonstration located at `examples/agent-demo` as the canonical entry point for this feature.
- **FR-002**: The demo MUST include clear instructions for setup, configuration, running the primary scenario, running any optional scenarios, and interpreting the expected output.
- **FR-003**: The demo MUST present a single coherent primary scenario that exercises an Agent from user input through intermediate steps to final response.
- **FR-004**: The demo MUST visibly demonstrate structured message exchange, including user input, Agent-generated output, and any intermediate content needed to understand the flow.
- **FR-005**: The demo MUST visibly demonstrate tool invocation, including the user's intent, tool arguments at a safe summary level, tool result handling, and how the Agent uses the result.
- **FR-006**: The demo MUST visibly demonstrate event or progress reporting so users can follow the Agent lifecycle rather than only seeing the final response.
- **FR-007**: The demo MUST visibly demonstrate streaming or incremental output behavior when that capability is available for the selected run mode.
- **FR-008**: The demo MUST visibly demonstrate session continuity or conversation state across more than one user turn.
- **FR-009**: The demo MUST visibly demonstrate memory or contextual recall in a way that can be verified from the scenario output.
- **FR-010**: The demo MUST visibly demonstrate middleware-style cross-cutting behavior such as observation, enrichment, validation, or policy handling.
- **FR-011**: The demo MUST include a capability coverage checklist that maps each major demonstrated capability to the scenario step where users can observe it.
- **FR-012**: The demo MUST include a deterministic or mockable path suitable for validation without relying on unpredictable live model wording.
- **FR-013**: The demo MUST include an optional live-model path when credentials and environment support are available, with clear separation from deterministic validation.
- **FR-014**: The demo MUST handle missing configuration, missing credentials, unsupported optional capabilities, tool failures, and cancellation with clear user-facing messages.
- **FR-015**: The demo MUST avoid exposing secrets or sensitive raw configuration in default terminal output, generated files, and trace summaries.
- **FR-016**: The demo MUST be documented as an example rather than a production template, including boundaries for what it intentionally does not cover.
- **FR-017**: The demo MUST provide validation instructions that maintainers can run to confirm the example remains functional after framework changes.
- **FR-018**: The demo MUST align with the project's compatibility and trace expectations by making observable behavior explicit enough for review.

### Key Entities *(include if feature involves data)*

- **Demo Scenario**: The end-to-end walkthrough users run; includes the user task, expected Agent actions, visible outputs, and completion criteria.
- **Capability Coverage Item**: A documented mapping from a framework capability to the demo step or output where it is demonstrated.
- **Run Mode**: A user-selectable way to run the demo, such as deterministic validation or optional live-model execution, with distinct requirements and expected behavior.
- **Demo Trace**: The observable record of Agent progress, tool use, state changes, errors, and final output produced by the demo.
- **Demo Configuration**: User-provided settings or environment prerequisites required to run optional capabilities safely.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A new developer can start from the demo instructions and complete the deterministic primary scenario in under 10 minutes on a prepared development machine.
- **SC-002**: The primary scenario demonstrates at least 8 major framework capabilities with visible evidence in output or documentation.
- **SC-003**: 100% of required setup values and optional credentials are documented with where to provide them and how missing values are reported.
- **SC-004**: The deterministic validation path completes successfully in at least 95% of clean local runs where project prerequisites are already installed.
- **SC-005**: When required configuration is missing, users receive an actionable setup message before any irreversible action or confusing failure occurs.
- **SC-006**: The default demo output contains no raw secret values and no unnecessary sensitive conversation content.
- **SC-007**: A maintainer can verify the capability coverage checklist against a demo run without reading unrelated examples.

## Assumptions

- The demo is intended for developers and maintainers evaluating or learning AgentScope Rust.
- The requested location `examples/agent-demo` is part of the required user-facing outcome because users explicitly specified it.
- The phrase "所有功能" is interpreted as all major capabilities currently relevant to an Agent demonstration, not every internal implementation detail or every historical feature in the repository.
- Capabilities that require external services, platform-specific support, or unsafe side effects may be represented through deterministic, documented, or opt-in scenarios rather than enabled by default.
- Existing project prerequisites, build tooling, and example conventions remain available to users of the demo.
- The demo should prefer observable trace and repeatable behavior over relying solely on natural-language model output.
