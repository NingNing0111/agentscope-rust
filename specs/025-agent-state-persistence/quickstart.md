# Quickstart: Agent 状态持久化验证指南

**Feature**: 025-agent-state-persistence | **Date**: 2026-08-03

本指南提供可运行的验证场景，证明特性端到端可用。实现细节见 `tasks.md` 与实现阶段；数据格式见 [data-model.md](data-model.md)、[contracts/session-store.md](contracts/session-store.md)、[contracts/json-file-format.md](contracts/json-file-format.md)、[contracts/agent-config.md](contracts/agent-config.md)。

## 前置条件

- Rust stable toolchain（workspace edition 2024）
- 无外部服务依赖（内置 JSON 文件后端为本地文件）

## 验证命令

```bash
cargo test -p agent_scope_state           # SessionStore/JsonFileSessionStore 单测 + 往返测试
cargo test -p agent_scope_agent           # ReActAgent 恢复/自动落盘集成测试
cargo clippy --workspace -- -D warnings   # 无警告
cargo fmt --all -- --check                 # 格式通过
```

## 场景

### 场景 1：JSON 文件后端保存/加载往返（P1）

**目的**: 证明 `JsonFileSessionStore` 可持久化完整 `AgentState` 并无损恢复。

**验证方式**: `agent_scope_state` 集成测试（或临时测试 bin）。

**步骤**:
1. 创建临时目录，构造 `JsonFileSessionStore::new(dir)`
2. 构造含多轮消息、summary、tasks、middle_context 的 `SessionImpl`
3. `store.save(&session)` → 断言 `dir/{session_id}.json` 存在
4. `store.load(session_id)` → 断言 `AgentState` 全字段（context、summary、reply/permission/tool/tasks/middle context）与保存前一致

**预期**: 往返无损；文件内容符合 [json-file-format.md](contracts/json-file-format.md) 结构。

### 场景 2：跨进程重启恢复（P1）

**目的**: 证明进程重启后按 `session_id` 可恢复完整历史并继续作答。

**验证方式**: `agent_scope_agent` 集成测试（Scripted/Mock Model，宪法第六条）。

**步骤**:
1. 用 Scripted Model 构建 Agent（指定 `session_id="s-1"`、默认 store），完成一轮 reply，状态落盘
2. 丢弃 Agent 实例（模拟进程重启）
3. 用同一 `session_id="s-1"` 重新构建 Agent
4. 断言恢复后 `try_state().context` 包含步骤 1 的历史；用 Scripted Model 触发第二轮，断言基于完整历史作答

**预期**: 恢复成功；第二轮基于含历史的上下文正确推理。

### 场景 3：自动落盘（P1）

**目的**: 证明 reply 结束后自动保存最新状态。

**验证方式**: `agent_scope_agent` 集成测试。

**步骤**:
1. 构建 Agent（默认 `auto_persist=true`、默认 store），指定临时 `session_id`
2. 触发一轮 reply（含工具调用）
3. 断言 `store.load(session_id)` 返回的状态包含该轮产生的上下文与工具结果

**预期**: reply 结束后状态已落盘。

### 场景 4：中断/取消时保存（P1）

**目的**: 证明 reply 被取消时保存中断时刻最新状态。

**验证方式**: `agent_scope_agent` 集成测试（取消场景，对齐 Feature 008 既有取消测试模式）。

**步骤**:
1. 构建 Agent，触发会中途取消的 reply
2. 取消后断言 `store.load` 返回的状态包含中断前已产生的消息，且状态一致

**预期**: 取消后状态可加载，无半写、无丢失。

### 场景 5：auto_persist 关闭 = 零写入（P1/P3）

**目的**: 证明关闭自动持久化后无磁盘写入（spec SC-007）。

**验证方式**: 集成测试。

**步骤**:
1. 构建 Agent 设置 `auto_persist(false)`，指定临时目录 store
2. 完成多轮 reply
3. 断言目录内无任何会话文件创建

**预期**: 0 个会话文件。

### 场景 6：损坏文件与非法标识（P1/P3）

**目的**: 证明损坏文件返回结构化错误、非法标识被拒绝。

**验证方式**: `agent_scope_state` 单测。

**步骤**:
1. 手动写入损坏 JSON 到 `{dir}/bad.json`
2. `store.load("bad")` → 断言返回 `SessionError::SerializationError`
3. `store.save` / `load` 传入含 `/` 或 `.` 的非法标识 → 断言返回 `SessionError`，且未创建越界文件

**预期**: 结构化错误，无崩溃、无静默空状态、无路径穿越。

### 场景 7：自定义后端接入（P2）

**目的**: 证明 `SessionStore` 作为扩展点，自定义实现可无缝接入（spec FR-008 / SC-004）。

**验证方式**: 集成测试中内联一个最小自定义 `SessionStore` 实现（如基于内存 HashMap + 简单路径，模拟 SQLite 语义）。

**步骤**:
1. 实现最小 `SessionStore`（save/load/delete/list_ids/list_meta）
2. 用 `AgentConfig::builder().session_store(Arc::new(custom_store)).session_id("c-1")` 构建 Agent
3. 完成往返，断言行为与内置后端一致

**预期**: 无需改框架代码，自定义后端行为一致。

### 场景 8：会话管理（P3）

**目的**: 证明 list/delete 语义正确且轻量。

**验证方式**: `agent_scope_state` 单测。

**步骤**:
1. 保存多个会话
2. `list_ids()` 返回全部 ID；`list_meta()` 返回元数据且按 `last_active` 降序，不加载完整状态
3. `delete(id)` 幂等；`load` 删除后返回 `NotFound`

**预期**: 列表轻量、删除幂等。

## 完整验收（宪法第十七条）

```bash
cargo test --workspace          # 全部测试通过（含本特性测试）
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
# 文档更新：docs/zh/modules/agent.md、docs/en/modules/agent.md
# 兼容性矩阵更新：存储模块 L2/L3
```

## 参考

- 数据模型：[data-model.md](data-model.md)
- 存储接口契约：[contracts/session-store.md](contracts/session-store.md)
- 文件格式契约：[contracts/json-file-format.md](contracts/json-file-format.md)
- AgentConfig 接入契约：[contracts/agent-config.md](contracts/agent-config.md)
