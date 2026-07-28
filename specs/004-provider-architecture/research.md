# Research: Provider Architecture & DashScope (Feature 004)

**Date**: 2026-07-28 | **Spec**: [spec.md](./spec.md)

## 1. OpenAI Crate 提取策略

**Decision**: 文件级迁移 + 路径重写。将 `agent_scope_model/src/openai/` 三个文件（model.rs/formatter.rs/parameters.rs）直接移动到 `agent_scope_openai/src/`，修改 crate 内部引用路径。

**Rationale**:
- `openai/` 子模块与 `agent_scope_model` 核心之间耦合清晰——仅通过 `use crate::*` 引用核心类型（ChatResponse、ModelError 等）
- 提取后只需将 `use crate::response::ChatResponse` 改为 `use agent_scope_model::ChatResponse` 等
- `_models/` 目录跟随移动，保持 `list_models()` 默认路径行为不变
- 测试代码跟随移动，确保测试覆盖不丢失

**Alternatives considered**:
- 重写而非迁移：浪费已有可工作的代码，风险不必要的回归
- 保持 `openai` 在 model crate 中作为 optional feature：仍违反宪法第十一条，`reqwest` 始终在依赖树中

## 2. 核心 Crate 依赖清理

**Decision**: 从 `agent_scope_model/Cargo.toml` 移除 `reqwest`、`tokio-stream`、`tokio-util`、`futures`、`serde_yaml`，保留 `serde`、`serde_json`、`base64`、`schemars`、`uuid`、`chrono`、`async-trait`、`thiserror`。

**Rationale**:
- `agent_scope_model` 核心能力（trait 定义、数据结构、StreamAccumulator、ModelCard）不需要 HTTP 调用
- `wav_header.rs` 仅做字节构建，不依赖外部库
- `card.rs` 使用 `serde_yaml`——需要从核心 crate 移除。`list_models()` 改为接收 `serde_yaml` 解析结果而非直接读 YAML（Provider 侧负责解析）
- `StreamAccumulator` 使用 `base64` 做音频数据编解码——保留此依赖

**Impact**: `ModelCard::from_yaml()` 的 `serde_yaml` 调用需重构为接收 `HashMap` + `serde_json::Value` 而非 `Path`。或者将 `card.rs`/`list_models()` 移到 Provider 层。

**Decision**: `ModelCard` 定义保留在 `agent_scope_model`，但 `from_yaml()` 改为 `from_raw(raw: &HashMap, base_schema: &JsonValue)`——Provider crate 负责 YAML 解析和文件扫描。

## 3. DashScope API 兼容模式

**Decision**: 使用 DashScope 的 OpenAI 兼容端点 `https://dashscope.aliyuncs.com/compatible-mode/v1`。

**Rationale**:
- DashScope 提供 `/compatible-mode/v1/chat/completions` 端点，请求/响应格式与 OpenAI Chat Completions API 一致
- 流式 SSE 格式与 OpenAI 相同（`data: {json}\n\n`，`data: [DONE]` 结束）
- 工具调用（Function Calling）在兼容模式下行为与 OpenAI 对齐
- 可直接复用 OpenAI 的 SSE 解析逻辑

**Known differences from OpenAI**:
| 特性 | OpenAI | DashScope 兼容模式 |
|------|--------|-------------------|
| `enable_search` | 无 | 有（联网搜索增强） |
| `repetition_penalty` | 无 | 有 |
| `structured_output` 原生 | 部分模型支持 | 不支持 |
| tool_choice `"required"` | 支持 | 部分模型不支持 |
| token 统计在流式中 | `stream_options.include_usage` | 同机制 |
| 独立 tokenizer API | tiktoken | 暂不提供独立端点 |

**Implementation note**: DashScope 兼容模式与 OpenAI API 的差异集中在参数扩展字段，主要解析逻辑（SSE parser、ChatResponse builder）可直接复用。

## 4. Mock HTTP 测试方案

**Decision**: 使用 `wiremock` crate 作为 Provider 测试的 mock HTTP server。

**Rationale**:
- `wiremock` 是 Rust 生态最成熟的 HTTP mock 库，支持匹配请求特征（URL、body、headers）和返回预设响应
- 支持异步测试（`#[tokio::test]`），与 Provider 的 async 风格一致
- 每个 Provider crate 独立使用 `wiremock` 作为 `[dev-dependencies]`，无需公共测试 crate（FR-020 降级为 SHOULD，避免过度抽象）
- SSE 流式响应可通过 `wiremock::ResponseTemplate` 的 `set_body_raw()` 返回原始 SSE 字节流

**Alternatives considered**:
- `httpmock`：功能类似，但 `wiremock` 社区更活跃
- 公共 `agent_scope_test_utils` crate：在 P1 阶段过度抽象，待至少 3 个 Provider 存在后提取更有依据

## 5. DashScope Formatter 实现

**Decision**: DashScope 兼容模式下消息格式与 OpenAI 相同（role + content），直接复用 `OpenAIChatFormatter` 的逻辑，但作为独立实现（不继承或依赖 OpenAI crate）。

**Rationale**:
- 消息格式在兼容模式下与 OpenAI 一致（`{"role": "user", "content": "..."}` 或 `[{"type": "text", "text": "..."}, {"type": "image_url", ...}]`）
- 每个 Provider 应自包含——`agent_scope_dashscope` 不依赖 `agent_scope_openai`
- DashScope 特有参数（`enable_search`）通过 `DashScopeParameters` 注入请求体

## 6. GitHub 是否支持 Mermaid 图表

**Decision**: 本 plan 不使用 Mermaid 图表。结构描述以文本树 + 表格呈现，足够清晰。

**Rationale**: 项目宪章、Feature 001-003 的 specs 均未使用 Mermaid；文本格式在终端和 Web 界面兼容性最好。

---

## 7. 依赖使用分析（实际引用）

**Decision**: 基于代码扫描确认：

- **`reqwest`**: 仅在 `openai/model.rs` 中使用（`OpenAIChatModel.client: reqwest::Client` + SSE 流解析的 `reqwest::Response`），迁移到 `agent_scope_openai` 后核心 crate 不再需要
- **`tokio-stream` / `tokio-util`**: 在 `agent_scope_model/src/` 中完全没有被引用（仅在 `Cargo.toml` 声明），是无用依赖，直接移除
- **`futures`**: 在两个位置使用：`model_trait.rs`（`Pin<Box<dyn Stream>>`）和 `openai/model.rs`（`StreamExt` + SSE 流处理）。核心 crate 保留此依赖（trait 定义需要），Provider crate 另行依赖
- **`serde_yaml`**: 仅在 `card.rs` 的 `ModelCard::from_yaml()` 中使用，见 Topic 2
- **`base64`**: 在 `response.rs`（`append_data_block`）、`accumulator.rs`（音频数据编解码）、`openai/model.rs`（解析响应中的音频）三处使用。核心 crate 保留此依赖
- **`thiserror`**: 在 release profile 下，`#[derive(Error)]` 通过 feature flag 编译。当前 `model_error.rs` 使用手动 impl `Display + Error` 格式，`thiserror` 可能为后续保留——需确认是否可移除

**Rationale**: 确保移除决策基于实际代码引用而非假定。

---

## 8. 测试文件迁移计划

**Decision**: `tests/formatter_integration.rs` 从 `agent_scope_model/tests/` 迁移到 `agent_scope_openai/tests/`。

**Rationale**: 该测试直接引用 `OpenAIChatFormatter`，属于 OpenAI-specific 测试。`chat_response_integration.rs` 和 `cross_crate_tests.rs` 不引用 OpenAI 类型，保留在核心 crate。

---

## 9. DashScope 特有参数补充（Web Research 发现）

基于 DashScope 官方文档的深入调研，发现以下额外参数和约束：

**新增参数**:
- **`enable_thinking`** (`bool`): 启用 Qwen 模型的思考模式（reasoning/thinking），返回 `reasoning_content` 字段
- **`thinking_budget`** (`u32`): 思考 token 预算（仅在 `enable_thinking: true` 时有效）
- **`search_options`** (`JsonValue`): 联网搜索的配置选项（如自定义搜索源），配合 `enable_search` 使用

**兼容性约束**:
- **`tool_choice="required"` + `enable_thinking=true`**: 互斥——思考模式下不能强制 tool call
- **`repetition_penalty`**: 有效范围 `(0, +∞)`（中高置信度），需在请求时校验 `> 0`
- **`stream_options.include_usage`**: 仅在 `stream=true` 时发送
- **空 `choices: []`**: 在 SSE 流中，仅含 `usage` 的最终 chunk 的 `choices` 为空数组，需正确处理
- **错误响应格式**: DashScope 同时支持 OpenAI 兼容的嵌套格式 `{"error": {"message": "..."}}` 和百炼自身的扁平格式，解析器需兼容两者

**参考 Rust 实现**:
- `async-dashscope`: DashScope 原生 API 的 Rust 封装（非兼容模式）
- `rig-bailian`: Rig 框架的百炼 Provider
- `anyllm_providers`: 多 Provider 抽象中包含 DashScope 实现

**Rationale**: 这些发现来源于阿里云官方文档和现有 Rust 实现的交叉验证，补充了计划阶段的参数完整性。`enable_thinking` 和 `thinking_budget` 应加入 `DashScopeParameters`；`search_options` 可先作为 `extra_body` 透传。

---

## Summary of Key Decisions
