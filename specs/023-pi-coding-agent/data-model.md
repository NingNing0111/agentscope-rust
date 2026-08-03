# Data Model: Pi Coding Agent (Rust)

**Feature**: 023-pi-coding-agent
**Created**: 2026-08-02
**Phase**: 1 — Design & Contracts

## Entity: RuntimeConfig

Represents all user-provided and default runtime settings for one pi-rust process.

### Fields

| Field | Type | Required | Validation |
|-------|------|----------|------------|
| `api_key` | string | yes | Must be non-empty after trimming. Must never be printed unmasked. |
| `model` | string | yes | Must be non-empty. Defaults to `qwen-plus`. |
| `workdir` | path string | yes | Must be non-empty. Created if missing where safe. |
| `session_id` | optional string | no | If provided, must match an existing session or produce a clear error. |
| `resume` | boolean | no | If true, load latest or selected session. |
| `no_tools` | boolean | no | If true, disables tool registration. |
| `no_memory` | boolean | no | If true, disables long-term memory. |
| `no_rag` | boolean | no | If true, disables RAG middleware. |
| `max_iters` | integer | yes | Must be greater than 0. Defaults to 20. |
| `command_timeout_secs` | integer | yes | Must be greater than 0. Defaults to 30. |

### Relationships

- Used to build `AgentRuntime`.
- Determines where `SessionRecord` and memory files are stored.
- Determines which `ToolDefinition` entries are exposed.

## Entity: AgentRuntime

Represents the active in-memory runtime for a single CLI process.

### Fields

| Field | Type | Required | Validation |
|-------|------|----------|------------|
| `agent` | ReAct agent handle | yes | Must be constructed with a valid model. |
| `toolkit` | optional toolkit | no | Present unless tools are disabled. |
| `memory` | optional memory store | no | Present unless memory is disabled. |
| `workspace` | optional local workspace | no | Present when workspace-dependent tools are enabled. |
| `session` | SessionRecord | yes | Must have a stable session ID. |
| `permissions` | PermissionContext | yes | Must contain explicit rules for each registered tool. |

### Relationships

- Owns the active `SessionRecord` during runtime.
- Emits `ConversationTurn` updates into session storage.
- Uses `ToolDefinition` instances for model tool calls.

## Entity: SessionRecord

Persisted representation of a pi-rust session.

### Fields

| Field | Type | Required | Validation |
|-------|------|----------|------------|
| `id` | string | yes | UUID or stable generated ID. Unique under `<workdir>/sessions/`. |
| `created_at` | timestamp | yes | ISO 8601 or equivalent stable serialized form. |
| `updated_at` | timestamp | yes | Must be >= `created_at`. |
| `cwd` | path string | yes | Original working directory for the session. |
| `model` | string | yes | Model used when session started or last resumed. |
| `turns` | array of ConversationTurn | yes | May be empty for a newly created session. |
| `summary` | optional string | no | Compacted summary if generated. |

### State Transitions

```text
new -> active -> saved -> resumed -> active
active -> exited -> saved
active -> error -> saved
```

### Validation Rules

- Session file must round-trip through JSON serialization.
- Loading a corrupt session returns a typed error and must not crash the CLI.
- API keys and secrets must not appear in persisted session JSON.

## Entity: ConversationTurn

Represents one user-to-Agent interaction.

### Fields

| Field | Type | Required | Validation |
|-------|------|----------|------------|
| `index` | integer | yes | Starts at 0 and increments by 1. |
| `user_input` | string | yes | Empty inputs are not persisted. |
| `events` | array of AgentTraceEvent | yes | Ordered by sequence. |
| `assistant_text` | string | yes | May be empty only if turn ended in error. |
| `started_at` | timestamp | yes | Stable serialized timestamp. |
| `completed_at` | optional timestamp | no | Present for completed/error turns. |
| `error` | optional ErrorRecord | no | Present when turn failed. |

### Relationships

- Belongs to exactly one `SessionRecord`.
- Contains zero or more `ToolInvocation` events.

## Entity: ToolDefinition

Represents a tool exposed to the model.

### Fields

| Field | Type | Required | Validation |
|-------|------|----------|------------|
| `name` | string | yes | One of `Read`, `Write`, `Edit`, `Bash` for MVP. |
| `description` | string | yes | Non-empty and model-facing. |
| `input_schema` | JSON schema | yes | Must validate incoming tool arguments. |
| `permission_level` | enum | yes | `allow`, `confirm`, or `deny`. |

### Relationships

- `ToolInvocation` references a `ToolDefinition` by name.
- `PermissionContext` evaluates tool execution against tool name and arguments.

## Entity: ToolInvocation

Represents one actual tool call during a conversation turn.

### Fields

| Field | Type | Required | Validation |
|-------|------|----------|------------|
| `id` | string | yes | Stable unique ID within the turn. |
| `tool_name` | string | yes | Must match a registered `ToolDefinition`. |
| `arguments` | JSON object | yes | Must pass the target tool schema. |
| `status` | enum | yes | `requested`, `confirmed`, `running`, `succeeded`, `failed`, `denied`. |
| `result_summary` | optional string | no | Redacted/truncated if large or sensitive. |
| `error` | optional ErrorRecord | no | Present on failure/denial. |

### State Transitions

```text
requested -> running -> succeeded
requested -> running -> failed
requested -> confirmation_required -> confirmed -> running -> succeeded
requested -> confirmation_required -> denied
```

## Entity: ErrorRecord

Serializable error information for user-visible failures.

### Fields

| Field | Type | Required | Validation |
|-------|------|----------|------------|
| `code` | string | yes | Stable machine-readable code. |
| `message` | string | yes | Human-readable, non-secret. |
| `category` | enum | yes | `validation`, `model`, `tool`, `permission`, `io`, `session`, `internal`. |
| `retryable` | boolean | yes | True if user may retry without changing input. |

### Validation Rules

- Must never include API keys or raw credentials.
- Must distinguish user error from model/provider/tool/system error.
