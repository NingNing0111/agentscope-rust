# Quickstart: Rig LLM Provider Integration 验证指南

**Feature**: 034-rig-llm-integration | **Date**: 2026-08-17

本文档定义端到端验证场景，证明特性按 spec 工作。类型映射与流式顺序契约见 `contracts/rig-mapping.md`；公开适配契约见 `contracts/provider-adapter.md`；设计决策见 `research.md`、`data-model.md`。

## 前置条件

```bash
cd /Users/pgthinker/StudyCode/GithubProject/agentscope-rust
cargo --version   # stable toolchain
```

**无需真实 LLM key**——核心场景用确定性 mock/recorded 响应（宪法第六条），真实 key 仅场景 5 可选冒烟。

## 场景 1：映射层单元测试（对应 FR-002/004 / SC-002）

`agent_scope_rig` 的映射契约测试（消息往返、工具 schema、ToolChoice、流式顺序、错误分类、结构化输出）全部为纯函数/确定性组件，无网络：

```bash
rtk cargo test -p agent_scope_rig
```

**预期**（逐项对照 `contracts/rig-mapping.md` §8 测试矩阵）：
- `message_mapping_tests.rs`：各 role 往返、Thinking、ToolCall、ToolResult 展开、Hint 不发送、Unknown→FormatError
- `tools_mapping_tests.rs`：schema→ToolDefinition、ToolChoice 四模式 + 子集过滤 + required 降级
- `streaming_tests.rs`：Reasoning→Thinking、Text 增量、ToolCall/Delta 拼接、`is_last`/`usage`/`tool_call_id_map`
- `error_mapping_tests.rs`：rig 错误 → `ModelErrorKind` 六分类全映射
- `structured_output_tests.rs`：output_schema 原生 + 工具 bypass 回退 + JSON repair + 空消息
- `openai/anthropic/deepseek_tests.rs`：构造 + mock HTTP 冒烟

## 场景 2：公开构造入口冒烟（对应 US2 / FR-003 / SC-003）

不经网络验证构造器 ergonomics 与默认值（对比 `contracts/provider-adapter.md` §1 等价表）：

```bash
rtk cargo test -p agent_scope_rig construction
```

**预期**：
- `RigChatModel::openai(key, "gpt-4.1").with_stream(true)` 构造成功，`model_name()=="gpt-4.1"`、`stream_enabled()==true`
- `RigChatModel::anthropic`/`deepseek` 同样构造成功；`context_size` 取 provider 默认（OpenAI 131072）
- `RigEmbeddingModel::openai(key, "text-embedding-3-small")` 构造成功，`model_card().dimensions` 与 rig `ndims` 一致
- 空 `api_key` 构造 → 不 panic，构造后调用返回 `ModelError::ValidationError`（或构造即校验，视实现）

## 场景 3：删除 dashscope + 示例迁移编译（对应 US1 / FR-001/008 / SC-001）

迁移完成后全 workspace 编译验证，并断言无 `agent_scope_dashscope` 残留：

```bash
rtk cargo build --workspace
rtk grep -r "agent_scope_dashscope" --include="*.rs" --include="Cargo.toml" .
```

**预期**：
- `cargo build --workspace` 成功；`crates/agent_scope_dashscope/` 目录不存在；workspace members/root manifest/示例 manifest 无该 crate 引用
- 7 个示例（agent/chat/human-in-the-loop/plan-react-agent/quickstart/rag/subagent）均用 `RigChatModel::openai`/`RigEmbeddingModel::openai` 构造，各自 `cargo build -p <example>` 通过
- `grep` 对 dashscope 零命中（代码 + manifests）

## 场景 4：确定性端到端 agent 循环（对应 US2/US4 / FR-004/010 / SC-002）

用 mock HTTP server（回放固定 OpenAI-compatible 响应）驱动 rig-backed 模型跑 ReAct 循环，验证流式事件顺序与工具生命周期（对照 `contracts/rig-mapping.md` §5 顺序契约）：

```bash
rtk cargo test -p agent_scope_agent rig_e2e
```

**预期**：
- 消息 delta → 工具调用 → 工具结果 → 结束的事件顺序与迁移前基线一致（无工具时纯文本回复亦正常）
- `tool_call_id_map` 在流末正确填充（`tc_{idx}` → provider id），工具结果回填成功
- 流式 `is_last` 只出现在末个 chunk；`finished_reason`/`usage` 在流末正确
- thinking（reasoning delta）→ `ThinkingBlock` 增量，不进消息流外的结构变化
- 错误路径：mock 返回 429/500/401 → 重试语义（429/500 重试 3 次后 `RetryExhausted` 或按 `retryable_errors`；401 不重试直接 `Authentication`）

## 场景 5：真实 key 冒烟（可选，对应 SC-003）

有真实 OpenAI key 时运行示例，目视检查：

```bash
OPENAI_API_KEY=xxx rtk cargo run -p quickstart -- --prompt "用一句话介绍 AgentScope"
OPENAI_API_KEY=xxx rtk cargo run -p chat              # 流式交互，观察 Thinking/Text 增量
```

**预期**：
- quickstart 5 分钟内可跑通（FR-003/SC-003）
- 流式场景看到逐 token/逐 block 增量展示，工具调用（若触发）正常执行
- 无 key / key 无效 → 清晰 `Authentication` 错误，不泄露 key（`contracts/rig-mapping.md` §4）

## 场景 6：兼容性登记（对应 FR-009/011 / SC-004）

```bash
python3 -c "
import json
m = json.load(open('specs/001-compatibility-baseline/capability-matrix.json'))
for e in m['entries']:
    cid = e.get('capability_id','')
    if cid.startswith('provider-'):
        print(cid, '| notes:', e.get('notes',''))
"
```

**预期**：
- 无 `provider-dashscope-*` 残留条目；`provider-openai-*` / `provider-anthropic-*` / `provider-deepseek-*` 条目登记能力覆盖与已知限制（仅 OpenAI 提供 embedding；`enable_search` 等 DashScope 特有能力登记为不迁移的已知限制；thinking 互斥按 provider 定值）
- 文档更新完成：README、`docs/rust/zh`、`agentscope-guide` skill 无 dashscope 构造示例（`rtk grep -r "DashScopeChatModel" docs/ README.md` 零命中或仅 specs 历史提及）
- 依赖治理（FR-011）：`cargo tree -i rig` 确认 rig 仅经 `agent_scope_rig` 引入；`cargo deny`（若配置）license 检查通过

## 完整验收命令（宪法第十七条 of done）

```bash
rtk cargo test --workspace
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo fmt --check
```

**预期**: 全 workspace 测试通过（含新增 `agent_scope_rig` 测试；既有 ~950+ 测试不回归）、clippy 零警告、fmt 通过；无未登记 `UnsupportedFeature`（场景 6 登记覆盖）。

## 回归基线

```bash
rtk cargo test --workspace   # 既有测试全部通过，dashscope 相关契约迁移到 agent_scope_rig
```

完成定义参照宪法第十七条 checklist：单元测试、无静默降级、文档更新、示例可编译、clippy/fmt 通过、兼容矩阵已更新、无未登记 UnsupportedFeature。
