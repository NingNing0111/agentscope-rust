# Tool Contracts: Pi Coding Agent (Rust)

**Feature**: 023-pi-coding-agent
**Audience**: Tool implementers, test authors, and model prompt authors

## Common Tool Result Shape

Every tool returns a structured result that can be summarized to the model and rendered to the user.

```json
{
  "ok": true,
  "summary": "human readable summary",
  "content": "optional full or truncated content",
  "metadata": {}
}
```

On failure:

```json
{
  "ok": false,
  "error": {
    "code": "stable_error_code",
    "category": "validation|tool|permission|io|internal",
    "message": "safe non-secret message",
    "retryable": false
  }
}
```

## Tool: Read

Reads a UTF-8 text file from the configured project working directory.

### Input Schema

```json
{
  "type": "object",
  "required": ["path"],
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to read, relative to the project working directory unless absolute paths are explicitly allowed."
    },
    "offset": {
      "type": "integer",
      "minimum": 0,
      "description": "Optional 0-based line offset."
    },
    "limit": {
      "type": "integer",
      "minimum": 1,
      "description": "Optional maximum number of lines to return."
    }
  },
  "additionalProperties": false
}
```

### Behavior

- Rejects directories.
- Rejects paths outside the configured project working directory.
- Returns line-numbered UTF-8 content.
- Large files are truncated with an explicit truncation note.
- Binary or invalid UTF-8 files return `unsupported_file_type`.

## Tool: Write

Creates or replaces a UTF-8 text file.

### Input Schema

```json
{
  "type": "object",
  "required": ["path", "content"],
  "properties": {
    "path": { "type": "string" },
    "content": { "type": "string" },
    "overwrite": {
      "type": "boolean",
      "default": false,
      "description": "If false, existing files are not overwritten."
    }
  },
  "additionalProperties": false
}
```

### Behavior

- Creates parent directories only when explicitly allowed by implementation tasks.
- If target exists and `overwrite` is false, returns `file_exists`.
- If target exists and overwrite is potentially destructive, requests confirmation.
- Rejects writes outside the project working directory.
- Writes UTF-8 only.

## Tool: Edit

Performs exact string replacement in a UTF-8 text file.

### Input Schema

```json
{
  "type": "object",
  "required": ["path", "old_string", "new_string"],
  "properties": {
    "path": { "type": "string" },
    "old_string": { "type": "string" },
    "new_string": { "type": "string" },
    "replace_all": {
      "type": "boolean",
      "default": false
    }
  },
  "additionalProperties": false
}
```

### Behavior

- Reads the target file before editing.
- If `replace_all` is false, `old_string` must occur exactly once.
- If `old_string` does not occur, returns `pattern_not_found`.
- If `old_string` occurs more than once and `replace_all` is false, returns `ambiguous_edit`.
- Rejects edits outside the project working directory.
- Writes atomically where practical.

## Tool: Bash

Executes a shell command in the configured project working directory.

### Input Schema

```json
{
  "type": "object",
  "required": ["command"],
  "properties": {
    "command": {
      "type": "string",
      "description": "Shell command to run."
    },
    "timeout_secs": {
      "type": "integer",
      "minimum": 1,
      "description": "Optional per-command timeout."
    }
  },
  "additionalProperties": false
}
```

### Behavior

- Executes from configured project working directory.
- Captures stdout, stderr, and exit code.
- Times out after configured timeout and returns partial output.
- Truncates large output with an explicit note.
- Requires confirmation for commands classified as potentially destructive.
- Never injects API keys or secret environment variables into output.

### Potentially Destructive Command Examples

Commands matching these categories require confirmation:

- File deletion: `rm`, `unlink`, `rmdir`
- Git history/worktree mutation: `git reset`, `git clean`, `git checkout .`, `git stash`
- Package install or lockfile mutation
- Commands containing shell redirection that writes files
- Commands that run arbitrary network scripts

## Error Codes

| Code | Category | Meaning |
|------|----------|---------|
| `invalid_arguments` | validation | Tool input failed schema validation. |
| `path_outside_workspace` | permission | Path escapes configured project directory. |
| `file_not_found` | io | Target file does not exist. |
| `file_exists` | validation | Write target exists and overwrite was not allowed. |
| `unsupported_file_type` | validation | File is binary or not UTF-8 text. |
| `pattern_not_found` | validation | Edit target string was absent. |
| `ambiguous_edit` | validation | Edit target string occurred multiple times. |
| `permission_denied` | permission | User or policy denied operation. |
| `command_timeout` | tool | Bash command exceeded timeout. |
| `command_failed` | tool | Bash command exited non-zero. |
| `internal_error` | internal | Unexpected implementation failure. |
