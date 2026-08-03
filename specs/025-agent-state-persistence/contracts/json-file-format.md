# Contract: JSON 文件存储格式（{session_id}.json）

**Feature**: 025-agent-state-persistence | **Date**: 2026-08-03

本契约定义内置 `JsonFileSessionStore` 的磁盘文件格式，作为数据持久化格式的稳定规范（宪法第十二条：稳定数据协议）。

> **对应 spec**: FR-002 / FR-003 / FR-004 / FR-011 / FR-013

## 布局

- 存储根目录：默认工作区 `sessions/`，可通过 `JsonFileSessionStore::new(dir)` 配置
- 每个会话一个文件：`{session_id}.json`
- 目录自动创建；文件使用 UTF-8 编码 JSON

## 文件 JSON 结构

```json
{
  "session_id": "a1b2c3d4",
  "status": "Active",
  "message_count": 5,
  "created_at": "2026-08-03T08:00:00Z",
  "last_active": "2026-08-03T08:15:00Z",
  "state": {
    "session_id": "a1b2c3d4",
    "summary": {},
    "context": [],
    "max_context_messages": null,
    "reply_context": { "reply_id": "", "cur_iter": 0, "structured_schema": null, "structured_output": null },
    "permission_context": {},
    "tool_context": { "max_cache_files": 100, "max_cache_bytes": 25000, "read_file_cache": [], "activated_groups": [] },
    "tasks_context": { "tasks": [] },
    "middle_context": {}
  }
}
```

| 顶层字段 | 类型 | 说明 |
|----------|------|------|
| `session_id` | string | 会话标识（= 文件名去 `.json`） |
| `status` | string | `Active` / `Closed` |
| `message_count` | int | 上下文消息数（`state.context` 长度） |
| `created_at` | ISO-8601 datetime | 创建时间 |
| `last_active` | ISO-8601 datetime | 最后活跃时间 |
| `state` | object | 完整 `AgentState`（核心载荷） |

`state` 对象为 `AgentState` 的 serde JSON 表示，字段与 Python 参考实现 `AgentState` 对齐（见 data-model.md §3）。

## 稳定性契约

- **缺省字段**：`AgentState` 所有字段 `#[serde(default)]`，旧版本文件缺字段按默认值兼容加载，不失败
- **未知字段**：serde 默认忽略未知字段（未设置 `deny_unknown_fields`），加载不受上游/未来新增字段影响
- **字段语义**：已发布字段名与含义不得随意修改；修改视为 MAJOR 版本变更
- **`session_id` 一致性**：加载后以文件名 `session_id` 为准，`state.session_id` 与之一致

## 写入原子性

- **保存过程**：写 `{session_id}.json.tmp` → `fsync` → `rename` 为 `{session_id}.json`
- **保证**：进程在任何时刻崩溃，目标文件要么是旧完整内容，要么是新完整内容，绝不出现半写（spec FR-004 / Edge Case）
- **清理**：成功后删除临时文件（若存在）

## 会话标识校验

- 禁止字符：路径分隔符（`/` `\`）、`.`、空字符串、目录分隔符等非法文件名
- 非法标识在保存/加载前拒绝（返回 `SessionError`），防止路径穿越与覆盖无关文件（spec Edge Case）

## 错误契约

| 场景 | 错误 |
|------|------|
| 文件不存在（load） | `SessionError::NotFound` |
| JSON 解析失败 / 截断 / 损坏 | `SessionError::SerializationError` |
| 目录创建 / 文件读写失败 | `SessionError::StorageError` |
| 非法会话标识 | `SessionError::StorageError`（ValidationError 变体可选） |

## 验收要点

- [ ] `save → load` 往返后 `AgentState` 全字段无损（含各 context、summary、tasks）
- [ ] 写过程中任意时刻 kill 进程，目标文件不损坏
- [ ] 缺字段 / 含未知字段的旧文件可正常加载
- [ ] 损坏文件返回 `SerializationError`，不崩溃、不静默空状态
- [ ] `{session_id}.json` 文件与目录创建成功，跨进程可读
