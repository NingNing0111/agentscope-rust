# Research: Provider 剥离与 DashScope (Feature 005)

**Date**: 2026-07-29 | **Spec**: [spec.md](./spec.md)

## 1. OpenAI 代码移除策略

**Decision**: 直接删除 `agent_scope_model/src/openai/` 子模块，不迁移到独立 crate。代码保留在 Git 历史中，日后如需恢复可从 `git log -- agent_scope_model/src/openai/` 找回。

**Rationale**:
- 用户明确要求"已实现的 openai 移除掉"——不创建 `agent_scope_openai` crate
- Feature 004 的提取方案创建了 `agent_scope_openai` crate，但 Feature 005 范围更收窄：只做清理 + DashScope
- Git 历史天然提供代码归档，无需维护一个空的/不用的 crate
- 核心目标：让 `agent_scope_model` 成为纯抽象层

**Alternatives considered**:
- 提取为独立 `agent_scope_openai` crate：增加维护负担，用户当前不要求
- 保留为 optional feature（`#[cfg(feature = "openai")]`）：仍违反宪法第十一条，`reqwest` 仍在可选依赖中

## 2. 核心 Crate 依赖清理

**Decision**: 移除 `reqwest`、`tokio-stream`、`tokio-util`、`serde_yaml`、`thiserror`；保留 `futures`。

**Rationale (per dependency)**:
- `reqwest`：仅在 `openai/model.rs`（Client 构造 + Response 流解析），随 openai/ 删除移除
- `tokio-stream`：代码库零引用（`grep -rn tokio_stream src/` 无结果），纯死依赖
- `tokio-util`：代码库零引用（`grep -rn tokio_util src/` 无结果），纯死依赖
- `serde_yaml`：仅在 `card.rs:83` 的 `ModelCard::from_yaml()` 中使用。解决方案：将 `from_yaml()` 重构为 `from_raw(raw_data: &str)` ——调用侧（未来 Provider）负责 YAML 解析，核心只接受已解析的结构化数据
- `thiserror`：`model_error.rs` 当前使用手动 `impl Display + Error`，未使用 `#[derive(Error)]`。移除无影响
- `futures`：在 `model_trait.rs:7` 中使用 `use futures::Stream`（`Pin<Box<dyn Stream<Item = ...>>>`），这是 `ChatModel` trait 定义的一部分——**无法移除**

**Impact**: `ModelCard::from_yaml()` API 变更——从接受 `Path` 改为接受已解析的数据。`list_models()` 的 YAML 读取逻辑移出核心 crate。

## 3. ModelCard API 重构方案

**Decision**: `ModelCard::from_yaml()` → `ModelCard::from_raw(yaml_str: &str)`，返回 `Result<Vec<ModelCard>, ModelError>`。调用侧负责文件扫描和读取。

**Rationale**:
- 核心 crate 不应关心 YAML 文件从哪里来（文件系统？嵌入资源？网络？）
- 移除 `serde_yaml` 后，核心 crate 的依赖树彻底干净
- DashScope Provider 在 `list_models()` 中自行处理 `_models/*.yaml` 文件扫描和 YAML 解析

**Alternatives considered**:
- 整个 `ModelCard` 移到 Provider 层：过度拆分——`ModelCard` 是通用模型元数据概念，应由核心定义
- 用 `serde_json` 替代 `serde_yaml`：YAML 是上游约定格式，不应要求 Provider 预先转换

## 4. DashScope API 兼容模式

**Decision**: 使用 DashScope 的 OpenAI 兼容端点 `https://dashscope.aliyuncs.com/compatible-mode/v1`。

**Rationale**:
- DashScope 提供 `/compatible-mode/v1/chat/completions` 端点，请求/响应格式与 OpenAI Chat Completions API 一致
- 流式 SSE 格式与 OpenAI 相同（`data: {json}\n\n`，`data: [DONE]` 结束）
- 工具调用在兼容模式下行为与 OpenAI 对齐
- 消息格式与 OpenAI 一致

**Known differences from OpenAI**:
| 特性 | OpenAI | DashScope 兼容模式 |
|------|--------|-------------------|
| `enable_search` | 无 | 有（联网搜索增强） |
| `repetition_penalty` | 无 | 有 |
| `enable_thinking` | 无 | 有（Qwen 思考模式） |
| `structured_output` 原生 | 部分模型支持 | 不支持 |
| `tool_choice: "required"` | 支持 | 部分模型不支持 |
| SSE 最终 chunk `choices` | 非空 | 可能为空数组 `[]` |
| 错误响应格式 | 嵌套 `{"error": ...}` | 嵌套 + 扁平两种 |

## 5. Mock HTTP 测试方案

**Decision**: 使用 `wiremock` 0.6 crate 作为 DashScope Provider 测试的 mock HTTP server。

**Rationale**:
- `wiremock` 是 Rust 生态最成熟的 HTTP mock 库
- 支持匹配请求特征（URL、body、headers）并返回预设响应
- 支持异步测试（`#[tokio::test]`）
- SSE 流式响应通过 `ResponseTemplate::set_body_raw()` 返回原始字节流
- 仅 1 个 Provider 时不需要公共 test_utils crate（FR-018 为 SHOULD，延后到有 2+ Provider 时提取）

## 6. DashScope Formatter 实现

**Decision**: 独立实现 `DashScopeFormatter`，不依赖 OpenAI 代码。在兼容模式下消息格式与 OpenAI 相同，因此格式化逻辑可参考已被移除的 `OpenAIChatFormatter`，但作为全新实现。

**Rationale**:
- 没有 `agent_scope_openai` crate 可供依赖
- 每个 Provider 应自包含
- 消息格式在兼容模式下已知（role + content 的 JSON 结构），无需反向工程

## 7. SSE 流解析策略

**Decision**: 手动实现 SSE 行解析器（逐行读取字节流，识别 `data:` 前缀，累积 JSON 行直到空行分隔）。

**Rationale**:
- DashScope 兼容模式下的 SSE 格式为标准 `data: <json>\n\n`
- 无需引入第三方 SSE 解析库（`reqwest::Response::bytes_stream()` + 行分割足够）
- 需处理的边界情况：空 `choices: []`、`data: [DONE]`、chunk 中包含 `usage` 的最终帧

## 8. 测试文件处理

**Decision**: `tests/formatter_integration.rs` 随 `openai/` 子模块删除（直接引用 `OpenAIChatFormatter`）。`chat_response_integration.rs` 和 `cross_crate_tests.rs` 保留在核心 crate。

**Rationale**:
- `formatter_integration.rs` 中 4 个测试全部引用 `OpenAIChatFormatter`，删除 OpenAI 模块后不可编译
- 其余 2 个测试文件不引用任何 OpenAI 类型

---

## Summary of Key Decisions

| 决策 | 选择 |
|------|------|
| OpenAI 代码 | 直接删除，不创建独立 crate |
| 移除的依赖 | reqwest, tokio-stream, tokio-util, serde_yaml, thiserror |
| 保留的依赖 | futures（trait 定义需要） |
| ModelCard 重构 | `from_yaml(Path)` → `from_raw(&str)` |
| DashScope API | OpenAI 兼容端点 `/compatible-mode/v1` |
| Mock 测试 | `wiremock` 0.6 per-crate dev-dependency |
| Formatter | 独立实现（自包含，不依赖 OpenAI） |
| SSE 解析 | 手动逐行解析 |
