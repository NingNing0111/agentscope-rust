# Contract: Write 工具

**Feature**: 029-agent-workspace-tools | **Status**: Draft
**兼容基准**: Python `agentscope/tool/_builtin/_write.py`（上游 `9d1026fa`）

## 工具名

`Write`

## 描述（对齐 Python `_write.py:33`）

将文件写入本地文件系统。若目标路径已有文件则**覆盖**；已存在文件必须先经 Read 读取。优先编辑既有文件，除非明确需要新文件。除非用户明确要求，不主动创建文档/README 文件；不主动添加 emoji。

## 输入 Schema

```json
{
  "type": "object",
  "properties": {
    "file_path": {
      "type": "string",
      "description": "The absolute path to the file to write (must be absolute, not relative)"
    },
    "content": {
      "type": "string",
      "description": "The content to write to the file"
    }
  },
  "required": ["file_path", "content"]
}
```

## 行为契约（对齐 Python `_write.py:232`）

| 步骤 | 行为 |
|------|------|
| 路径校验 | 绝对路径；`..`/符号链接逃逸 → `PermissionDenied` |
| **读-改守卫（覆盖）** | 已存在文件未在 `WorkspaceToolSession.read_files` → 拒绝：`Error: File {file_path} exists but has not been read yet. You must read the file first before writing to it.` |
| 父目录 | 自动创建父目录（经 backend） |
| 写入 | 经 `WorkspaceBackend::write_file` |
| 新文件 | 不存在 → 直接创建（无需读） |
| 返回 | 成功信息 `The file {file_path} has been written successfully ({line_count} lines).`，metadata 含 unified diff |

## 错误契约

| 场景 | 类别 | 错误码 |
|------|------|--------|
| 路径逃逸 | `PermissionDenied` | `path_outside_workspace` |
| 未读先覆盖 | `PermissionDenied` | `read_before_modify_required` |
| 写入失败（权限/IO） | `ExecutionFailure` | `permission_denied` |

## 元属性

`is_read_only = false` | `is_concurrency_safe = false` | `is_state_injected = true`

## 交叉引用

- Python: `_write.py`
- Spec: FR-010~012
- 相关: `contracts/workspace-tool-session.md`（read-state）
