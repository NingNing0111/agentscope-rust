---
title: "压缩上下文"
description: "理解 ReAct 的 token 预算、自动压缩与手动裁剪"
---

<Note>
**Rust 实现状态**：ReAct 自动压缩已接入批处理与流式推理循环。当前摘要是占位文本，不是模型生成的语义摘要；`compression_prompt` 和 `tool_result_limit` 已公开配置，但当前自动压缩路径尚未使用这两个字段。
</Note>

模型的上下文窗口同时容纳 system prompt、工具定义、历史消息、摘要、运行时注入以及模型输出空间。上下文压缩的目标不是让历史无限增长，而是在调用模型前主动回收旧消息所占预算。

## Token 预算

每轮推理开始时，ReAct 会计算：

```text
input_tokens = count_tokens(历史消息 + system prompt, tool schemas)
trigger_tokens = context_size × trigger_ratio
reserve_tokens = context_size × reserve_ratio
```

当 `enable == true` 且 `input_tokens > trigger_tokens` 时触发自动压缩。这里的 `count_tokens` 和 `context_size` 均来自当前 `ChatModel` 实现，因此结果取决于模型后端的计数精度。

`reserve_ratio` 在当前算法中是压缩目标计算的基准：实现根据 `input_tokens - reserve_tokens` 估算要从最旧端移除多少 token。它不是“最终保留消息比例”，也不保证压缩后精确等于该 token 数；当前算法按平均每条消息的 token 数估算边界。

## 配置与默认行为

`ContextConfig::default()` 的值如下：

| 字段 | 默认值 | 当前行为 |
|------|--------|----------|
| `enable` | `false` | 默认不自动压缩 |
| `trigger_ratio` | `0.8` | 超过模型窗口的 80% 时触发 |
| `reserve_ratio` | `0.1` | 用于计算目标回收量 |
| `compression_prompt` | `"<STD_CP_PROMPT>"` | 已公开，但当前占位摘要路径未使用 |
| `tool_result_limit` | `4096` | 已公开，但当前自动压缩路径未执行工具结果字符截断 |

```rust
use agent_scope_agent::ContextConfig;

let context_config = ContextConfig {
    enable: true,
    trigger_ratio: 0.8,
    reserve_ratio: 0.1,
    ..Default::default()
};
```

把该配置作为 `ReActAgent::new(...)` 或 `ReActAgent::build(...)` 的第三个参数传入，才能启用 ReAct 自动压缩。只构造 `ContextConfig` 不会修改任何状态。

## 构造期约束

`ContextConfig` 必须满足：

- `trigger_ratio ∈ (0, 1)`；
- `reserve_ratio ∈ [0, trigger_ratio)`。

同时，`AgentConfig` 中的注入配置必须满足：

- `InjectionConfig::context_buffer_ratio < ContextConfig::trigger_ratio`。

前两项由 `ContextConfig::validate()` 检查；跨配置约束由 `ReActAgent::new()` / `ReActAgent::build()` 检查。违反约束会在智能体构造阶段返回 `AgentError::InvalidConfig`。

```rust
use agent_scope_agent::ContextConfig;

let invalid = ContextConfig {
    enable: true,
    trigger_ratio: 0.8,
    reserve_ratio: 0.8,
    ..Default::default()
};

assert!(invalid.validate().is_err());
```

## 自动执行顺序

每个 ReAct 推理迭代依次执行：

1. 克隆当前 `AgentState::context`；
2. 加入 system prompt，并连同 tool schemas 计算 `input_tokens`；
3. 若超过阈值，压缩持久状态中的旧消息；
4. 基于压缩后的状态重新判断运行时注入；
5. 再读取更新后的上下文，执行中间件并调用模型。

压缩在运行时注入之前发生。因此，旧的时间 Hint 或任务感知痕迹若被移除，同一轮可以重新注入；最终 Hint 也会占用本轮实际模型输入。详见[感知环境](environment-awareness)。

批处理和流式路径遵循同一压缩语义。

## 当前压缩策略

超过阈值后，当前实现会：

1. 根据平均每条消息的 token 数估算从最旧端移除的消息数；
2. 调整切分点，避免留下失去对应调用的 tool result；
3. 从 `AgentState::context` 头部移除这些消息；
4. 把 `AgentState::summary` 设置为类似 `[Compressed 12 messages, ~2400 tokens]` 的占位文本；
5. 在剩余上下文开头插入携带该占位文本的消息，让模型知道发生了压缩。

::: warning
当前自动摘要只记录“移除了多少消息/估算 token”，不会保留事实、决策、实体或任务细节。不要把它描述为语义总结，也不要依赖它恢复被移除的原文。
:::

### Tool call / tool result 原子边界

模型 API 通常要求工具调用与对应结果保持合法顺序。自动压缩会扩大头部移除边界，把已被移除的 tool call 所对应的连续 tool result 一并移除，避免留下孤立结果。

手动 `trim_context` 则双向保护相邻的 tool-call / tool-result 消息对：只要一侧被保留，另一侧也会被保留。这种保护可能使最终保留消息数高于 `keep_recent`，并使结果无法简单等同于配置的消息或 token 上限。

## 失败时的 fallback

如果自动 `compress_context` 返回错误，ReAct 不会立即中止回复，而是记录 warning 并执行消息数 fallback：

```text
max_messages = max(当前消息数 / 2, 10)
```

fallback 从最旧端移除超出 `max_messages` 的消息，并把 `AgentState::summary` 更新为 `[Truncated N oldest messages]` 占位摘要。当前 fallback **不会**把该摘要插回 `AgentState::context`，因此当轮模型不会自动看到这段 `[Truncated ...]` 文本；调用方若需要展示或发送它，必须自行组装模型输入。它是保底截断，不执行语义总结，也不调用 Workspace offload。

当前 `compress_context` 的常规路径主要进行内存操作，错误分支较少；文档保留 fallback 说明，是为了准确描述 ReAct 对压缩错误的处理契约。

## 手动裁剪 `trim_context`

不使用 ReAct 自动压缩时，可以直接调用 `agent_scope_state::trim_context`。该 API 支持消息数和 token 两类阈值，并返回裁剪前后的统计信息。

```rust
use agent_scope_state::{AgentState, TrimStrategy, trim_context};

let mut state = AgentState::new();

let strategy = TrimStrategy {
    max_messages: Some(30),
    max_tokens: None,
    keep_recent: 20,
    keep_system_messages: true,
};

if let Some(result) = trim_context(&mut state, &strategy, None) {
    println!(
        "context messages: {} -> {}",
        result.messages_before,
        result.messages_after,
    );
}
```

若要按 token 触发，需要提供与应用一致的计数函数：

```rust
use agent_scope_message::Msg;
use agent_scope_state::{AgentState, TrimStrategy, trim_context};

let mut state = AgentState::new();
let count_tokens = |messages: &[Msg]| messages.len() * 100; // 应用提供的估算器

let strategy = TrimStrategy {
    max_messages: None,
    max_tokens: Some(2_000),
    keep_recent: 15,
    keep_system_messages: true,
};

let result = trim_context(&mut state, &strategy, Some(&count_tokens));
```

手动裁剪的保证包括：

- 不拆分相邻的 tool-call / tool-result 消息对；
- 可保留开头连续的 system 消息；
- 至少保留 `keep_recent` 条消息；
- 把被裁剪内容的文本表示累积到 `AgentState::summary`，并限制累计摘要体积；
- 无需裁剪或无法在保留约束下移除消息时返回 `None`。

手动 API 只修改你传入的 `AgentState`。它不会自动接入 ReAct 循环，也不会自动把 `state.summary` 拼入模型输入；调用方需要决定何时裁剪、持久化和重新组装消息。

## 与卸载的边界

如果必须保留被移除内容的原文，应在裁剪前显式调用 Workspace offload，再把可恢复路径放入你自己的摘要或消息。当前 ReAct 自动压缩不会自动完成这一步，参见[卸载上下文](offload-context)。
