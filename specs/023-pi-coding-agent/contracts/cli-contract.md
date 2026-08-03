# CLI Contract: Pi Coding Agent (Rust)

**Feature**: 023-pi-coding-agent
**Audience**: CLI users and implementation tasks

## Executable

```bash
cargo run -p pi-rust -- [OPTIONS]
```

The final binary name is `pi-rust`.

## Environment Variables

| Name | Required | Description |
|------|----------|-------------|
| `API_KEY` | yes, unless `--api-key` is provided | Default provider API key. Must be masked in all output. |
| `DASHSCOPE_API_KEY` | no | Optional alias for DashScope credentials if `API_KEY` is absent. |
| `RUST_LOG` | no | Controls diagnostics output. Defaults to concise user-facing output. |

## Options

| Option | Default | Description | Validation |
|--------|---------|-------------|------------|
| `--api-key <KEY>` | env `API_KEY` | Provider API key. | Non-empty after trimming. |
| `--model <MODEL>` | `qwen-plus` | Chat model name. | Non-empty. |
| `--workdir <DIR>` | `.pi-rust` | Runtime storage directory. | Non-empty path. |
| `--cwd <DIR>` | current directory | Project working directory for file/shell tools. | Must exist and be a directory. |
| `--prompt <TEXT>` | none | Run one prompt and exit. | Non-empty if provided. |
| `--resume [SESSION_ID]` | false | Resume latest session or a selected session. | Selected session must exist. |
| `--list-sessions` | false | Print known sessions and exit. | N/A |
| `--no-tools` | false | Disable all tools; chat only. | N/A |
| `--no-memory` | false | Disable long-term memory. | N/A |
| `--no-rag` | false | Disable retrieval middleware. | N/A |
| `--max-iters <N>` | `20` | Maximum ReAct iterations per turn. | `N > 0`. |
| `--command-timeout-secs <N>` | `30` | Bash command timeout. | `N > 0`. |
| `--show-events` | false | Show lifecycle/tool events. | N/A |
| `--show-json-events` | false | Print redacted event JSON lines. | N/A |

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success. |
| `1` | Runtime failure after configuration succeeded. |
| `2` | Invalid CLI configuration or missing required credentials. |

## REPL Commands

| Command | Behavior |
|---------|----------|
| `/help` | Show commands, active config summary, and sample prompts. |
| `/model` | Show active provider/model names, without printing secrets. |
| `/tools` | Show registered tool names and permission behavior. |
| `/sessions` | List persisted sessions. |
| `/save` | Force-save the current session. |
| `/events on` | Enable human-readable event rendering. |
| `/events off` | Disable human-readable event rendering. |
| `/json on` | Enable redacted JSON event output. |
| `/json off` | Disable redacted JSON event output. |
| `/exit` or `/quit` | Save session and terminate. |

Unknown slash commands must not be sent to the model. They produce a concise local error.

## One-shot Mode Contract

When `--prompt` is supplied:

1. CLI validates config.
2. CLI builds runtime.
3. CLI sends exactly one user turn to the Agent.
4. CLI streams answer and tool events.
5. CLI saves session.
6. CLI exits with code `0` on success.

## Interactive Mode Contract

When `--prompt` is absent:

1. CLI validates config.
2. CLI creates or resumes a session.
3. CLI prints a short banner with masked credentials.
4. CLI reads one input line at a time.
5. Empty input is ignored.
6. Slash commands are handled locally.
7. Non-command input is sent to the Agent.
8. Each completed turn is saved.

## Safety Contract

- API keys and tokens must never appear unmasked in stdout, stderr, session JSON, tool results, or event JSON.
- Potentially destructive Bash commands require confirmation before execution.
- File writes outside the configured project working directory are denied unless the implementation explicitly defines a safe allowlist.
- Tool output may be truncated, but truncation must be visible to the user.
