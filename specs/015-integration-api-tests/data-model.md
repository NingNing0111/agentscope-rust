# Data Model: Integration API Tests (Examples)

**Feature**: 015-integration-api-tests
**Date**: 2026-07-31

## Key Entities

### TestResult

每个测试场景的标准化结果。

| 字段 | 类型 | 描述 |
|------|------|------|
| `name` | `&'static str` | 测试场景名称 |
| `passed` | `bool` | 是否通过 |
| `detail` | `String` | 通过时的简述或失败时的诊断信息 |
| `duration_ms` | `u64` | 执行耗时（毫秒） |

**状态转换**: 不可变（创建后不修改）

**关系**: 每个 example binary 产生 1-N 个 TestResult

---

### CliArgs

所有 example 共享的 CLI 参数。

| 字段 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| `api_key` | `String` | env `API_KEY` | DashScope API key |
| `model` | `String` | `"qwen-plus"` | 模型名称 |
| `keep_dir` | `bool` | `false` | 是否保留临时目录（调试用） |

**关系**: 每个 example binary 有一个 CliArgs 实例

---

### MemoryTestState

Memory example 的内部状态（非序列化）。

| 字段 | 类型 | 描述 |
|------|------|------|
| `workdir` | `TempDir` | 临时工作目录 |
| `agent` | `ReActAgent` | 配置了 MemoryMiddleware 的 Agent |
| `memory` | `Arc<FileMemory>` | File-backend Memory 实例 |

**生命周期**: 在 main() 中创建，在 main() 结束时 drop（TempDir 自动清理）

---

### SessionTestState

Session example 的内部状态。

| 字段 | 类型 | 描述 |
|------|------|------|
| `store` | `Arc<InMemorySessionStore>` | 会话存储后端 |
| `session` | `Option<SessionImpl>` | 当前活动会话 |

**状态转换**:
- `[session=None]` → create → `[session=Active]`
- `[session=Active]` → save → `[session persisted in store]`
- `[session persisted]` → load → `[session=Active (restored)]`
- `[session=Active]` → close → `[session=Closed]`

---

### RAGTestState

RAG example 的内部状态。

| 字段 | 类型 | 描述 |
|------|------|------|
| `embedding_model` | `Arc<DashScopeEmbeddingModel>` | DashScope embedding 模型 |
| `vector_store` | Implemented in-example | 内存向量存储 |
| `kb` | `Arc<KnowledgeBase>` | 知识库实例 |
| `rag_middleware` | `RAGMiddleware` | RAG 中间件（Static mode） |
| `agent` | `ReActAgent` | 配置了 RAGMiddleware 的 Agent |

**关系**: KnowledgeBase → EmbeddingModel + VectorStore → RAGMiddleware → ReActAgent

---

### EventTrace

Streaming tool-call example 中使用的事件追踪结构。

| 字段 | 类型 | 描述 |
|------|------|------|
| `tool_call_starts` | `u32` | ToolCallStart 事件计数 |
| `tool_call_deltas` | `u32` | ToolCallDelta 事件计数 |
| `tool_call_ends` | `u32` | ToolCallEnd 事件计数 |
| `tool_result_starts` | `u32` | ToolResultStart 事件计数 |
| `tool_result_deltas` | `u32` | ToolResultTextDelta 事件计数 |
| `tool_result_ends` | `u32` | ToolResultEnd 事件计数 |
| `text_deltas` | `Vec<String>` | 收集的文本块内容 |
| `start_end_pairs_ok` | `bool` | Start/End 是否成对 |

**不变量**: tool_call_starts == tool_call_ends, tool_result_starts == tool_result_ends
