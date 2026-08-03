# Implementation Plan: Agent 状态持久化（内置 JSON 文件存储 + 可插拔存储后端）

**Branch**: `025-agent-state-persistence` | **Date**: 2026-08-03 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/025-agent-state-persistence/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

为 Rust 版 Agent 增加开箱即用的状态持久化能力，对齐 Python 参考实现（本地 `agentscope/`，v2.0.5）的会话存储语义：

1. **扩展 `agent_scope_state` 的 `SessionStore`**：该 trait 已定义 upsert/load/delete/list_ids/list_meta（语义对齐 Python `StorageBase` 的 `upsert_session`/`get_session`/`delete_session`/`list_sessions`/`update_session_state`），当前唯一实现为纯内存 `InMemorySessionStore`。新增**内置 `JsonFileSessionStore`** 实现：每会话一个 `{session_id}.json` 文件，含 `SessionMeta` 与完整 `AgentState`（对齐 Python `SessionRecord` 逻辑结构），原子写入（临时文件 + 重命名）。
2. **将 `SessionStore` 接入 ReActAgent 运行时**：`AgentConfig` 新增可选字段——`session_store: Option<Arc<dyn SessionStore>>`、`session_id: Option<String>`、`auto_persist: bool`（默认 true）。构建 Agent 时若指定 `session_id` 则从存储加载既有状态，否则新建；每次 reply 正常结束与被中断/取消时自动保存最新状态（自动持久化失败向调用方报告但不破坏推理循环）。
3. **自定义后端扩展点 = `SessionStore` trait 本身**：SQLite/MySQL/Redis 等由开发者实现该 trait 接入，无需改框架代码、不加入内置配置枚举（spec 用户决策 2）。
4. 持久化数据协议对齐 Python `AgentState` 字段形状（字段名已一致），serde 未知字段/默认值向后兼容（宪法第十二条）。

核心设计决策（详见 [research.md](research.md)）：
1. 复用既有 `SessionStore` trait，仅新增 `JsonFileSessionStore` 实现——零接口改动，保持 `InMemorySessionStore` 不动
2. JSON 文件格式 = 单个文件内联 `SessionMeta` + `AgentState`（对齐 Python `SessionRecord`：`id/created_at/updated_at/status/message_count/state`）
3. ReActAgent 接入为增量式：`AgentConfig` 新增可选字段，不删改现有字段，既有调用完全向后兼容
4. 原子写入（`write-tmp + rename`），崩溃不产生半写文件；损坏文件返回 `SessionError::SerializationError`
5. `SessionStore` 作为自定义后端唯一扩展点；文档给出 SQLite/MySQL 实现契约说明

## Technical Context

**Language/Version**: Rust（workspace edition 2024，见根 `Cargo.toml`）

**Primary Dependencies**: tokio、serde/serde_json、async-trait、chrono、uuid、futures；内部 crate：`agent_scope_state`（SessionStore/AgentState/SessionMeta）、`agent_scope_agent`（ReActAgent/AgentConfig/AgentError）

**Storage**: 本地 JSON 文件（每会话一个 `{session_id}.json`，默认目录工作区 `sessions/`，可配置）；不引入外部服务依赖

**Testing**: `cargo test`（workspace）；序列化往返测试；原子写入/损坏文件测试；`JsonFileSessionStore` 单元 + 集成测试；ReActAgent 恢复集成测试（Scripted/Mock Model，宪法第六条）

**Target Platform**: 跨平台库（Linux / macOS / Windows）

**Project Type**: library（多 crate Cargo workspace）

**Performance Goals**: 无独立性能目标——会话状态读写为低频操作（每 reply 一次），I/O bound 于磁盘；不引入缓存层（宪法第十五条）

**Constraints**: `#![deny(unsafe_code)]`；库代码禁 unwrap/expect/panic；无新后台任务（结构化并发，宪法第十条）；无循环依赖（宪法第十一条）；原子写入避免部分写

**Scale/Scope**: 主改 2 个 crate（`agent_scope_state`：+`json_file_store.rs` 与测试；`agent_scope_agent`：config/react_agent/react_loop/streaming_reactor 增量接入）；更新示例与文档；新增契约与验证指南

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 条款 | 符合性 | 说明 |
|------|--------|------|
| 第一条 兼容性优先 | ✅ | 存储语义对齐 Python `StorageBase`（upsert_session/get_session/delete_session/list_sessions）；持久化 JSON 结构对齐 `SessionRecord`/`AgentState` 形状；字段名与 Python 参考实现一致 |
| 第二条 锁定上游版本 | ✅ | 基线锁定：本地 `agentscope/` v2.0.5（`src/agentscope/_version.py` = "2.0.5"，HEAD `6698d98`），记录于 research.md |
| 第三条 Python 是行为基准 | ✅ | 已阅读上游源码（`app/storage/_base.py`、`app/storage/_model/_session.py`、`state/_state.py`），非凭文档推测；契约见 `contracts/` |
| 第四条 先契约后实现 | ✅ | spec（025 已批准）+ research + data-model + contracts 先行 |
| 第五条 不允许伪兼容 | ✅ | 自定义后端仅定义扩展点，不提供 SQLite/MySQL 空实现；损坏文件返回结构化错误而非静默空状态；加载不存在会话 = 新建会话（Python 行为本身） |
| 第六条 测试驱动兼容性 | ✅ | 序列化往返、原子写入、损坏文件、恢复集成测试为核心手段（quickstart 场景 1-6）；不依赖真实 LLM 判定 |
| 第七条 Trace 是核心验收产物 | ✅ | 自动持久化作为 reply 结束的副作用，通过落盘文件内容与恢复后状态可观测；不新增事件类型故无需 trace 规范变更 |
| 第八条 Rust 原生设计 | ✅ | `Arc<dyn SessionStore>` trait object 注入（对齐 `Arc<dyn ChatModel>`/`Arc<dyn Memory>` 既有模式）；`enum` 表达状态；不模拟 Python 动态类型 |
| 第九条 安全 Rust 优先 | ✅ | 无 unsafe；锁操作遵循 crate 既有模式；无新 panic 路径 |
| 第十条 结构化并发 | ✅ | 零新 spawn、零新 channel；保存为回复结束时的同步 await 操作，无后台孤儿任务 |
| 第十一条 分层与依赖方向 | ✅ | `JsonFileSessionStore` 置 `agent_scope_state`（该 crate 已依赖 tokio/serde）；`agent_scope_agent` 经 `Arc<dyn SessionStore>` 消费；无新 crate 间依赖边、无循环 |
| 第十二条 稳定数据协议 | ✅ | `AgentState` 字段零变更（仅新增 JSON 文件后端实现）；serde 未知字段/默认值兼容；`SessionStore` trait 零接口改动 |
| 第十三条 稳定错误模型 | ✅ | 复用既有 `SessionError`（NotFound/SerializationError/StorageError）；I/O 错误映射为 `StorageError` 保留根因；无字符串匹配判错 |
| 第十四条 可观测性 | ✅ | 复用现有 span/事件；持久化操作 tracing（session_id、路径概要），不含敏感信息 |
| 第十五条 性能不牺牲正确性 | ✅ | 无性能优化诉求；低频 I/O 操作，原子写入正确性优先 |
| 第十六条 小步交付 | ✅ | 单一能力（状态持久化 + 可插拔后端），前置依赖（010 会话管理已交付） |
| 第十七条 完成的定义 | ✅ | quickstart.md 定义完整验收命令（test/clippy/fmt/文档） |
| 第十八条 兼容性分级 | ✅ | 目标等级：**L2**（核心行为兼容：会话保存/恢复语义）+ **L3**（公开 API 语义兼容：`JsonFileSessionStore`、`SessionStore` 作为扩展点、`AgentConfig` 新增配置）；L1 字节级协议不在范围 |
| 第十九条 变更治理 | ✅ | 无宪法违反项 |

**Gate 结果（Phase 0 前）**: 通过，无违规需论证。
**Gate 结果（Phase 1 后复审）**: 通过——设计产物（research/data-model/contracts/quickstart）未引入新的宪法冲突；`SessionStore` 复用、JSON 文件后端新增、AgentConfig 增量接入决策均有 spec 用户决策与 research 记录支撑。契约文档明确对齐 Python `StorageBase`（L2/L3），错误模型复用 `SessionError`（宪法第十三条），数据协议零字段变更（宪法第十二条）。

## Project Structure

### Documentation (this feature)

```text
specs/025-agent-state-persistence/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/
│   ├── session-store.md # SessionStore 接口语义契约（自定义后端扩展点，SQLite/MySQL 示例）
│   ├── json-file-format.md  # {session_id}.json 文件格式契约
│   └── agent-config.md  # AgentConfig 接入契约（session_store/session_id/auto_persist）
├── checklists/
│   └── requirements.md  # /speckit-specify 阶段产物
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/
├── agent_scope_state/
│   ├── src/
│   │   ├── lib.rs                   # 新增 json_file_store 模块 + re-export JsonFileSessionStore
│   │   └── json_file_store.rs       # 新增: JsonFileSessionStore 实现（原子写、损坏检测、元数据）
│   └── tests/
│       └── json_file_store_tests.rs # 新增: 往返/原子/损坏/并发测试
├── agent_scope_agent/
│   ├── src/
│   │   ├── config.rs                # AgentConfig + builder 新增 session_store/session_id/auto_persist
│   │   ├── react_agent.rs           # 构建时按 session_id 加载；reply 后保存；保存失败上报
│   │   ├── react_loop.rs            # 回复结束保存点（batch 路径）
│   │   └── streaming_reactor.rs     # 回复结束保存点（streaming 路径）
│   └── tests/
│       └── agent_persistence_tests.rs # 新增: 恢复/自动落盘/关闭后恢复集成测试
examples/
└── pi-rust/                         # 示例接入（可选，展示恢复用法）
docs/
├── zh/modules/agent.md              # 持久化章节
└── en/modules/agent.md              # 同上
```

**Structure Decision**: 单 workspace 多 crate 布局（既有结构）。存储后端实现置 `agent_scope_state`（存储抽象与数据模型的所有权所在，宪法第十一条）；ReActAgent 接入增量式（`AgentConfig` 新增可选字段），不动现有字段与既有行为。

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

无违规项，本表不适用。
