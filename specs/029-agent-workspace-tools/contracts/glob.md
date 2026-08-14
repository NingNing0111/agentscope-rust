# Contract: Glob 工具

**Feature**: 029-agent-workspace-tools | **Status**: Draft
**兼容基准**: Python `agentscope/tool/_builtin/_glob.py`（上游 `9d1026fa`）

## 工具名

`Glob`

## 描述（对齐 Python `_glob.py:48`）

快速文件模式匹配工具，适用于任意规模代码库。支持 glob 模式（如 `**/*.js` 或 `src/**/*.ts`），返回匹配文件路径，按修改时间排序（最新优先）。需要按模式跨代码库找文件时使用。

## 输入 Schema

```json
{
  "type": "object",
  "properties": {
    "pattern": {
      "type": "string",
      "description": "The glob pattern to match against (e.g., '**/*.py', 'src/**/*.ts')"
    },
    "path": {
      "type": "string",
      "description": "The base directory to search from (defaults to current working directory)"
    }
  },
  "required": ["pattern"]
}
```

## 行为契约（对齐 Python `_glob.py:206`）

| 步骤 | 行为 |
|------|------|
| 模式校验 | `pattern` 非空；无效 glob → 拒绝 |
| 基础目录 | 默认 workspace 根；限定在 workspace 范围内（`..`/符号链接逃逸拒绝） |
| 目录校验 | `path` 必须为存在的目录，否则 `Directory not found: {base_dir}` |
| 实现 | **Native Rust 实现**（遍历 + globset 匹配），不 shell out（spec 决策） |
| 结果排序 | 按 mtime 最新优先（对齐 Python `_glob_helper.py`）；或确定性排序（降级） |
| 结果上限 | 有界（防大量匹配淹没上下文） |
| 无匹配 | `No files found matching pattern: {pattern}` |

## 错误契约

| 场景 | 类别 | 错误码 |
|------|------|--------|
| pattern 为空 | `ValidationFailure` | `invalid_arguments` |
| 无效 glob 模式 | `ValidationFailure` | `invalid_pattern` |
| 搜索根逃逸 | `PermissionDenied` | `path_outside_workspace` |
| 目录不存在 | `ExecutionFailure` | `file_not_found` |

## 元属性

`is_read_only = true` | `is_concurrency_safe = true` | `is_state_injected = false`

## 交叉引用

- Python: `_glob.py` + `_scripts/_glob_helper.py`
- Spec: FR-015~016
- 相关: `pi-rust/src/tools.rs` 的 `glob_tool`（`globset` 依赖、`MAX_GLOB_SCAN_ENTRIES` 可复用）
