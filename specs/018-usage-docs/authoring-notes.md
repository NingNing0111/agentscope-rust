# Authoring Notes: Feature 018 文档撰写事实库

**用途**: 内部撰写参考（非交付物，无需双语）。所有文档中的 API、配置、兼容性信息必须以此处核实事实或源码直接核实为准（契约 E-2）。

---

## T002: docs/superpowers/ 既有内容基线（FR-001 对照依据）

记录时间: 2026-08-01

```text
docs/superpowers/specs/2026-07-31-examples-terminal-chat-design.md
```

共 1 个文件。交付验收时（T031）`docs/superpowers/` 必须与此清单一致，无增删改。

---

## T003: 示例锚点清单（examples/，契约 E-1）

### examples/chat.rs — 终端流式对话 Agent（thinking 模式）

| 行区间 | 内容 |
|--------|------|
| L35-L50 | CLI 定义：`api_key` 经 `-k`/`--api-key` 传入（**无 env 属性**，必须显式传参）；`--model` 默认 `qwen-plus`；`--no-thinking` 开关 |
| L56-L78 | `BlockTracker`：按 block 追踪状态并累积 End 事件内容 |
| L86 | `render_event()`：覆盖全部 AgentEvent Block 类型的终端渲染 |
| L387-L388 | `main()` 入口；L388 `dotenv::dotenv().ok();` |

### examples/common.rs — 示例共享工具库

| 行区间 | 内容 |
|--------|------|
| L34 | `create_model(api_key, model_name) -> Arc<DashScopeChatModel>` |
| L43 | `create_model_with_thinking(...)`（thinking 模式变体） |
| L268-L286 | calculator 表达式解析与求值（`parse`/`evaluate`） |
| L288 | `calc_handler(input: CalcInput) -> String` 工具处理函数 |
| L311 | `create_calculator_tool() -> FunctionTool` |
| L327 | `build_agent(...)` 组装 ReActAgent |
| L368 | `create_memory_agent(...)` 带记忆的 Agent |
| L414 | `create_session_store() -> Arc<InMemorySessionStore>` |
| L425-L433 | Session 包装（`new`/`create_session`） |
| L487-L494 | 本地 embedding 辅助 + `cosine_sim` |
| L666 | `create_rag_agent(...)` RAG 链路组装 |
| L735-L776 | 测试报告辅助（`print_result`/`print_test_header`/`print_summary`/`print_banner`） |

### examples/memory_test.rs — 记忆能力验证

L35-L45 CLI（`env = "API_KEY"`，L37）；L54 `collect_reply()`；L77 `main()`；L214 `run_write_memory()`；L251 `run_search_memory()`；L289 `run_memory_reasoning()`

### examples/session_test.rs — 会话持久化验证（离线，无需真实模型）

L30-L36 CLI（`env = "API_KEY"` 且 `default_value = ""`，L32）；L44 `make_msg()`；L53 `main()`；L157 `run_save_load_roundtrip()`；L185 `run_context_consistency()`；L224 `run_close_cleanup()`

### examples/rag_test.rs — RAG 检索增强验证

L35-L49 CLI（`env = "API_KEY"` L37；`--embedding-model` 默认 `EMBEDDING_MODEL` L45；`--embedding-dims` 默认 `EMBEDDING_DIMS` L49）；L57 `collect_reply_text()`；L77 `main()`；L196 `run_ingest_test()`；L236 `run_grounded_query()`；L250 `run_empty_kb_query()`

### examples/streaming_tool_test.rs — 流式工具调用验证

L108-L114 CLI（`env = "API_KEY"` L110）；L123 `main()`；L229 `run_single_tool_call()`；L244 `run_multi_tool_call()`

### examples/verify_agent.rs — ReActAgent 六项集成验证

L35-L41 CLI（`env = "API_KEY"` L37）；L113-L277 calculator 解析器；L279 `calc_handler`；L293 `create_calculator_tool()`；L307 `build_agent()`；L345 `test_simple_chat`；L369 `test_calculator_tool`；L397 `test_multiturn`；L434 `test_streaming`；L473 `test_complex_calc`；L504 `test_observe_reply`；L535 `main()`

---

## T004: 已核实配置事实（research.md D7，2026-08-01 对照源码）

1. **API key 传入方式（示例间存在差异，文档必须按所引示例准确描述）**:
   - `chat.rs`：**仅** `-k`/`--api-key` 显式传参，`#[arg(short = 'k', long)]` **无 env 属性**（chat.rs:40）
   - `memory_test.rs:37` / `streaming_tool_test.rs:110` / `verify_agent.rs:37` / `rag_test.rs:37`：`#[arg(short = 'k', long, env = "API_KEY")]`
   - `session_test.rs:32`：`env = "API_KEY"` 且 `default_value = ""`（可离线运行）
2. **`.env` 加载**: `dotenv::dotenv().ok();`（chat.rs:388，其余示例同模式）；`.env` 位于仓库根，含 `API_KEY=sk-...`，已被 `.gitignore` 的 `.env*` 忽略
3. **默认模型名**: `qwen-plus`（各示例 CLI 默认值）
4. **Chat 模型构造**: `DashScopeChatModel::new(api_key: impl Into<String>, model_name: impl Into<String>)`（`crates/agent_scope_dashscope/src/model.rs:65`）；链式配置 `with_base_url()`（model.rs:82）、`with_stream(bool)`（model.rs:88）
5. **Embedding 模型构造**: `DashScopeEmbeddingModel::new(api_key: String, model_card: EmbeddingModelCard)`（`crates/agent_scope_dashscope/src/embedding.rs:92`）；`with_cache()`（embedding.rs:103）、`with_base_url()`（embedding.rs:109）。⚠️ 类型名是 `DashScopeEmbeddingModel`（research.md D7 中的 `DashScopeEmbedding` 系笔误，以此为准）
6. **凭据缺失行为**: API key 为空时不 panic，调用时返回错误（测试锚点 `test_dashscope_missing_api_key`，embedding.rs:257）
7. **crate 不显式读取环境变量**：凭据由调用方传入，符合分层设计

---

## T005: 上游版本锁定与兼容性矩阵现状（research.md D4）

**上游锁定**（`specs/001-compatibility-baseline/version-lock.json`）:

| 字段 | 值 |
|------|-----|
| 仓库 | https://github.com/agentscope-ai/agentscope |
| Release tag | v2.0.5 |
| Commit hash | 27b6a0d2a2afedf53462c9a2add33932d54b2d20 |
| Python 版本 | >=3.11 |
| 锁定日期 | 2026-07-28 |

**兼容性矩阵现状**（`capability-matrix.json`，2026-08-01 实测）:
- 280 条目，字段 `capability_id`/`category`/`upstream_symbol`/`target_level`/`status`/`notes` 等
- 全部条目 `status = "NOT_ANALYZED"`——矩阵未随 Feature 001-017 完成而回填（与宪法第一条"每发布更新矩阵"存在落差，回填列为后续任务，不属于本特性）
- 文档侧应对：兼容性章节以 `target_level` + 各 feature spec 声明的目标等级 + 代码实际实现状态交叉核实；迁移参考中如实注明矩阵 status 陈旧状态，不编造实现状态
