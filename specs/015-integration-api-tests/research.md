# Research: Integration API Tests (Examples)

**Feature**: 015-integration-api-tests
**Date**: 2026-07-31

## Decision 1: Example Binary Structure

**Decision**: 每个 US 一个独立 example binary，独立 CLI 入口。

**Rationale**:
- FR-002 要求每个场景独立可运行。
- 独立 binary 避免测试间相互干扰（特别是 Memory/Session 的状态污染）。
- 符合现有 `chat.rs` / `verify_agent.rs` 模式。
- Cargo.toml 中每个 example 只需一条 `[[example]]` 配置。

**Alternatives considered**:
- 单一 binary + CLI subcommands：代码更集中但增加了解析复杂度，且不符合"独立可运行"要求。
- 写为 `tests/` 目录下的集成测试：会与 cargo test 混淆，且不便于手动指定 API key。

## Decision 2: Memory Example 数据隔离

**Decision**: 每次运行使用独立 tempdir，运行结束后清理。

**Rationale**:
- FR-011 要求"不需要预存在本地状态"。
- 使用 `tempfile::TempDir` 确保每次运行从零开始。
- 如果用户想保留记忆文件用于调试，可通过 `--keep-dir` flag 选择保留。

**Alternatives considered**:
- 固定目录：会积累测试残留数据，不符合"fresh runs must work"要求。
- 用户指定目录：增加 CLI 复杂度，且不符合自动化场景。

## Decision 3: Session Example 使用 InMemorySessionStore

**Decision**: Session 持久化测试使用 `InMemorySessionStore`，不依赖外部数据库。

**Rationale**:
- Spec assumptions 明确"File-based memory and session backends are sufficient"。
- `InMemorySessionStore` 已经完整实现 save/load/delete/list_ids/list_meta。
- 不需要额外依赖（如 Redis/SQLite），保持 example 零配置。

**Alternatives considered**:
- File-based session store：当前 crate 中没有实现（只有 InMemorySessionStore），需要新增，超出 scope。
- 单次运行内 save/load：虽然无法验证"跨程序重启"场景，但可以验证 AgentState 序列化往返的正确性。

## Decision 4: RAG Example 的 VectorStore

**Decision**: 使用测试中已有的 `MockVectorStore`（in-memory）或创建一个类似的 example-scoped in-memory vector store。

**Rationale**:
- 项目中没有生产级 VectorStore 实现（Qdrant 等），但 `vector_store_mock.rs` 已有完整实现。
- Example 只需验证 pipeline 正确性（embedding → vector store → search → grounded answer），不验证特定 vector DB。
- 可以在 example 中内联一个简化版 MockVectorStore。

**Alternatives considered**:
- 外部 Qdrant/Milvus 服务：安装成本高，不符合"single command"目标。
- Skip RAG example：RAG 是 Feature 011 的核心交付物，需要有集成验证。

## Decision 5: RAG Example 的 Embedding Model

**Decision**: 使用 `DashScopeEmbeddingModel`，从 API key 和 model name 构造。

**Rationale**:
- FR-012 明确要求使用 DashScope embedding API。
- EmbeddingModelCard 硬编码为 `text-embedding-v3`，dimensions=1536。
- DashScope embedding API 端点已验证可用。

## Decision 6: Streaming Tool-Call Example 的事件验证

**Decision**: 消费流事件，统计事件类型和顺序，验证关键不变量。

**Rationale**:
- FR-006 要求验证完整事件生命周期：start → delta(s) → end。
- 可以统计：ToolCallStart 数量、ToolCallEnd 数量、ToolResultStart 数量、ToolResultEnd 数量，验证它们成对出现。
- 验证 event 顺序：ToolCallStart 在 ToolCallEnd 之前、ToolCallDelta 在 Start 和 End 之间。
- 验证最终答案数学正确性。

**Alternatives considered**:
- 不做结构化验证，仅打印事件：太弱，等同于现有 chat.rs。
- 使用 Rust 单元测试风格验证：不符合 example 定位。

## Decision 7: Error Handling 策略

**Decision**: 所有 example 在三种以下错误场景产生有意义输出：(1) API key 无效，(2) 网络不可达，(3) 模型不存在的响应。

**Rationale**:
- FR-010 要求 graceful error handling。
- 不 panic，使用清晰的错误消息 + 非零退出码。
- 借鉴 `chat.rs` 的错误处理模式（检查 "invalid"、"api"、"key" 关键词）。

## Decision 8: Cargo.toml 配置

**Decision**: 在 workspace 根 `Cargo.toml` 中添加 4 个 `[[example]]` 条目。

**Rationale**:
- 与现有 `chat` 和 `verify_agent` example 配置保持一致。
- 每个 example 的 `name` 和 `path` 一一对应。

## Decision 9: 复用 common.rs

**Decision**: 扩展现有 `common.rs` 添加 Memory、Session、RAG、Embedding 的共享工厂函数。

**Rationale**:
- 避免每个 example 重复构造代码。
- 保持一致的配置模式（API key、model name 等）。
- 所有 example 共享 calculator tool 和 agent builder。
