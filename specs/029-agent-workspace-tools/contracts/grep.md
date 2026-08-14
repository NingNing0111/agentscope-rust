# Contract: Grep 工具

**Feature**: 029-agent-workspace-tools | **Status**: Draft
**兼容基准**: Python `agentscope/tool/_builtin/_grep.py`（上游 `9d1026fa`）

## 工具名

`Grep`

## 描述（对齐 Python `_grep.py:45`）

基于 ripgrep 的强大搜索工具。**ALWAYS 使用 Grep 做搜索任务，绝不用 Bash 调用 `grep`/`rg`**。支持完整正则语法、glob/type 文件过滤、三种输出模式（content/files_with_matches/count）、上下文行、大小写不敏感、multiline 正则、结果限制（head_limit）。

## 输入 Schema

```json
{
  "type": "object",
  "properties": {
    "pattern": {
      "type": "string",
      "description": "The regular expression pattern to search for in file contents."
    },
    "path": {
      "type": "string",
      "description": "File or directory to search in. Defaults to current working directory."
    },
    "output_mode": {
      "type": "string",
      "enum": ["content", "files_with_matches", "count"],
      "description": "Output mode: 'content' shows matching lines (supports -A/-B/-C context, -n line numbers, head_limit), 'files_with_matches' shows file paths, 'count' shows match counts. Defaults to 'files_with_matches'.",
      "default": "files_with_matches"
    },
    "glob": {
      "type": "string",
      "description": "Glob pattern to filter files (e.g., '*.js', '*.{ts,tsx}')."
    },
    "type": {
      "type": "string",
      "description": "File type to search (rg --type). Common types: js, py, rust, go, java, etc."
    },
    "-A": { "type": "integer", "description": "Number of lines to show after each match. Requires output_mode: 'content'." },
    "-B": { "type": "integer", "description": "Number of lines to show before each match. Requires output_mode: 'content'." },
    "-C": { "type": "integer", "description": "Alias for context." },
    "context": { "type": "integer", "description": "Number of context lines to show before and after matches. Requires output_mode: 'content'." },
    "n": { "type": "boolean", "description": "Show line numbers in output. Requires output_mode: 'content'. Defaults to true.", "default": true },
    "i": { "type": "boolean", "description": "Case insensitive search.", "default": false },
    "case_insensitive": { "type": "boolean", "description": "Case insensitive search (alias for i).", "default": false },
    "multiline": { "type": "boolean", "description": "Enable multiline mode where . matches newlines and patterns can span lines. Default: false.", "default": false },
    "head_limit": { "type": "integer", "description": "Limit output to first N lines/entries. Defaults to 250 when unspecified. Pass 0 for unlimited.", "minimum": 0 },
    "offset": { "type": "integer", "description": "Skip first N lines/entries before applying head_limit. Defaults to 0.", "default": 0, "minimum": 0 }
  },
  "required": ["pattern"]
}
```

## 行为契约（对齐 Python `_grep.py:320`）

| 步骤 | 行为 |
|------|------|
| 模式校验 | `pattern` 非空；`head_limit < 0`/`offset < 0` → 拒绝 |
| 搜索根 | 默认 workspace 根；限定在 workspace 范围内（`..`/符号链接逃逸拒绝） |
| 排除 | VCS 目录（`.git/.svn/.hg/.bzr/.jj/.sl`）排除 |
| 实现 | **Native Rust 实现**（遍历 + 正则匹配），不 shell out 到 rg/find（spec 决策） |
| 大小写 | `i`/`case_insensitive` → 大小写不敏感 |
| 上下文 | `context`/`-A`/`-B`/`-C`（content 模式） |
| 结果上限 | `head_limit`（默认 250），`offset` 偏移；**硬上限防淹没上下文**（spec SC-007：>1000 匹配返回有界响应） |
| 输出截断 | 超长行截断、结果截断 |
| 无匹配 | `No matches found for pattern: {pattern}`（Success） |

## 错误契约

| 场景 | 类别 | 错误码 |
|------|------|--------|
| pattern 为空 | `ValidationFailure` | `invalid_arguments` |
| head_limit/offset 为负 | `ValidationFailure` | `invalid_arguments` |
| 搜索路径逃逸 | `PermissionDenied` | `path_outside_workspace` |
| 搜索路径不存在 | `ExecutionFailure` | `file_not_found` |

## 元属性

`is_read_only = true` | `is_concurrency_safe = true` | `is_state_injected = false`

## 交叉引用

- Python: `_grep.py`
- Spec: FR-013~014
- 相关: `pi-rust/src/tools.rs` 的 `grep_tool`（`MAX_GREP_RESULTS` 等界限可复用）
