# Contract: Edit 工具

**Feature**: 029-agent-workspace-tools | **Status**: Draft
**兼容基准**: Python `agentscope/tool/_builtin/_edit.py`（上游 `9d1026fa`）

## 工具名

`Edit`

## 描述（对齐 Python `_edit.py:31`）

对文件执行精确字符串替换。**必须先使用 Read 读取文件**才能编辑——未读取时工具报错。编辑 Read 输出文本时，确保保留行号前缀之后的精确缩进（tab/空格）。`old_string` 必须唯一，否则编辑失败。仅 emoji 若用户明确要求才使用。

## 输入 Schema

```json
{
  "type": "object",
  "properties": {
    "file_path": {
      "type": "string",
      "description": "The absolute path to the file to edit."
    },
    "old_string": {
      "type": "string",
      "description": "The exact string to replace. Must match exactly including whitespace and indentation."
    },
    "new_string": {
      "type": "string",
      "description": "The string to replace old_string with."
    },
    "replace_all": {
      "type": "boolean",
      "description": "If true, replace all occurrences. If false (default), only replace if there is exactly one occurrence.",
      "default": false
    }
  },
  "required": ["file_path", "old_string", "new_string"]
}
```

## 行为契约（对齐 Python `_edit.py:253`）

| 步骤 | 行为 |
|------|------|
| 路径校验 | 绝对路径；`..`/符号链接逃逸 → `PermissionDenied` |
| 文件存在 | 不存在 → `ExecutionFailure`（`file_not_found`） |
| 空/相同字符串 | `old_string == new_string` → 拒绝（"identical. No changes to make."） |
| **读-改守卫** | 已存在文件未在 `WorkspaceToolSession.read_files` → 拒绝：`Error: To edit a file, you must first read it using the Read tool.` |
| 未找到 | `old_string` 不在内容中 → 拒绝：`Error: old_string not found in {file_path}` |
| 非唯一 | 出现多次且 `replace_all=false` → 拒绝：提示次数并建议 `replace_all=true` 或更具体 |
| 替换 | `replace_all=true` → 全部替换；否则仅替换第一处（`replace(..., 1)`） |
| 写入 | 经 `WorkspaceBackend::write_file`，原子（临时文件 + rename） |
| 返回 | 成功信息 `Successfully replaced N occurrence(s) in {file_path}`，metadata 含 unified diff |

## 错误契约

| 场景 | 类别 | 错误码 |
|------|------|--------|
| 路径逃逸 | `PermissionDenied` | `path_outside_workspace` |
| 未读先改 | `PermissionDenied` | `read_before_modify_required` |
| 文件不存在 | `ExecutionFailure` | `file_not_found` |
| old_string 为空/缺失 | `ValidationFailure` | `invalid_arguments` |
| old_string 未找到 | `ValidationFailure` | `pattern_not_found` |
| 非唯一（无 replace_all） | `ValidationFailure` | `ambiguous_edit` |
| old_string == new_string | `ValidationFailure` | `invalid_arguments` |

## 元属性

`is_read_only = false` | `is_concurrency_safe = false` | `is_state_injected = true`

## 交叉引用

- Python: `_edit.py`
- Spec: FR-006~009
- 相关: `contracts/workspace-tool-session.md`（read-state）
