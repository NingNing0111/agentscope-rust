# Contract: Read 工具

**Feature**: 029-agent-workspace-tools | **Status**: Draft
**兼容基准**: Python `agentscope/tool/_builtin/_read.py`（上游 `9d1026fa`）

## 工具名

`Read`

## 描述（对齐 Python `_read.py:26`）

从本地文件系统读取文件。`file_path` 必须是绝对路径。默认读取文件开头至多 2000 行；可通过 `offset`/`limit` 控制。结果以 `cat -n` 格式返回（行号从 1 开始）。支持读取图片（多模态展示）与 PDF（大 PDF 需 `pages` 参数）。

## 输入 Schema

```json
{
  "type": "object",
  "properties": {
    "file_path": {
      "type": "string",
      "description": "The absolute path to the file to read."
    },
    "offset": {
      "type": "integer",
      "description": "Optional 1-based line number to start reading from (default: 1)",
      "default": 1,
      "minimum": 1
    },
    "limit": {
      "type": "integer",
      "description": "Optional maximum number of lines to read (default: 2000, max: 2000)",
      "default": 2000,
      "maximum": 2000,
      "minimum": 1
    }
  },
  "required": ["file_path"]
}
```

## 行为契约

| 步骤 | 行为 |
|------|------|
| 路径校验 | 绝对路径；`..`/符号链接逃逸 → `PermissionDenied`（`path_outside_workspace`） |
| 文件存在 | 不存在 → `ExecutionFailure`（`file_not_found`） |
| 目录 | 目标为目录 → `ValidationFailure`（`unsupported_file_type`） |
| 读取 | 经 `WorkspaceBackend::read_file`（Rust 侧 10 MiB 上限） |
| 缓存 | **成功读取后记录到 `WorkspaceToolSession.read_files`**（归一化路径）——这是 Edit/Write 读-改守卫的前提 |
| 输出格式 | `{i:6}\t{content}`，行号 6 位填充 + tab；超长行截断（对齐 Python `max_line_characters=2000`） |

## 读-改守卫（FR-008 前提）

- `Read` 成功读取 → 目标路径记入 session read-state。
- `Edit`/`Write`（已存在文件）→ 要求路径在 read-state 中，否则拒绝。

## 错误契约

| 场景 | 类别 | 错误码 |
|------|------|--------|
| 路径逃逸 | `PermissionDenied` | `path_outside_workspace` |
| 文件不存在 | `ExecutionFailure` | `file_not_found` |
| 目标为目录 | `ValidationFailure` | `unsupported_file_type` |
| 非 UTF-8 | `ValidationFailure` | `unsupported_file_type` |
| 文件过大（超限） | `ValidationFailure` | `file_too_large` |

## 元属性

`is_read_only = true` | `is_concurrency_safe = true` | `is_state_injected = true`

## 交叉引用

- Python: `_read.py`（`cache_file` 在 `_read.py:244`）
- Spec: FR-008, FR-012
- 相关: `contracts/workspace-tool-session.md`（read-state 定义）
