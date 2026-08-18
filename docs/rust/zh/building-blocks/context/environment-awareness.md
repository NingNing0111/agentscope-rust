---
title: "感知环境"
description: "用运行时 Hint 让智能体感知时间、任务与上下文用量"
---

<Note>
**Rust 实现状态**：运行时状态注入已实现，由 `AgentConfig::injection_config` 中的 `InjectionConfig` 控制。应用通常只配置公开字段，由 ReAct 循环决定注入时机；不要依赖内部注入函数或私有执行细节作为调用接口。
</Note>

运行时状态注入会在满足条件时，把时间、未完成任务和上下文用量组合成一个 `HintBlock`，追加到持久对话上下文。模型在随后一次调用中看到该 Hint，从而获得普通对话消息未必包含的环境信息。

## 配置 `InjectionConfig`

默认配置会启用运行时注入：

| 字段 | 默认值 | 作用 |
|------|--------|------|
| `inject_runtime_state` | `true` | 总开关 |
| `timezone` | `"UTC"` | 当前时间使用的 IANA 时区 |
| `time_format` | `"%Y-%m-%dT%H:%M:%S"` | `chrono` 风格的时间格式 |
| `time_interval` | `0.5` | 时间提示的最小间隔，单位为小时 |
| `context_buffer_ratio` | `0.2` | 在压缩阈值前预留的上下文感知缓冲 |
| `template` | 内置模板 | 组合各维度提示的模板 |
| `injection_source` | 内置来源名 | 写入 `HintBlock` 的来源标识 |
| `task_tool_names` | 内置任务工具名 | 用于识别任务操作和抑制重复提醒 |
| `extra_fields` | 空 | 注入触发时附加的自定义字段 |
| `emit_hint_event` | `true` | 是否同时返回 `HintBlockEvent` |

下面的配置使用公开 API：

```rust
use std::collections::HashMap;

use agent_scope_agent::{AgentConfig, InjectionConfig};

let injection_config = InjectionConfig {
    timezone: "Asia/Shanghai".into(),
    time_format: "%Y-%m-%d %H:%M:%S".into(),
    time_interval: 1.0,
    context_buffer_ratio: 0.15,
    extra_fields: HashMap::from([
        ("project".into(), "agentscope-rust".into()),
    ]),
    emit_hint_event: true,
    ..Default::default()
};

let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .injection_config(injection_config)
    .build()?;
```

`AgentConfigBuilder::build()`（即 builder 链末尾的 `.build()`）会验证时间格式、时间间隔、比例、模板和额外字段名等配置。无法解析的 `timezone` 不会导致构造失败；运行时会记录 warning 并回退到 UTC。构造 `ReActAgent` 时还会检查：

```text
context_buffer_ratio < ContextConfig::trigger_ratio
```

无效配置返回 `AgentError::InvalidConfig`。这能确保上下文用量 Hint 在自动压缩阈值之前仍有触发空间。

如果不需要任何运行时 Hint，设置：

```rust
use agent_scope_agent::InjectionConfig;

let injection_config = InjectionConfig {
    inject_runtime_state: false,
    ..Default::default()
};
```

## 注入维度

### 当前时间

系统按 `timezone` 和 `time_format` 生成时间文本。已有时间 Hint 距当前时间超过 `time_interval` 后，可以再次注入；如果上下文中已经找不到可解析的旧时间 Hint，例如它刚被压缩移除，也可以重新注入。

时区采用 IANA 名称，例如 `UTC` 或 `Asia/Shanghai`。无法解析的名称会在运行时回退到 UTC；为避免 Hint 标注的名称与实际时区不一致，应用仍应在部署前验证该字段。

### 未完成任务

系统检查 Agent state 中的未完成任务，并生成任务状态提醒。为避免循环提示，检测到以下任一情况时会抑制该维度：

- 上下文中已有同类任务 Hint；
- 最近消息包含 `task_tool_names` 所列工具的调用。

如果应用注册了不同名称的任务工具，应同步修改 `task_tool_names`。该字段只影响检测，不会注册工具。

### 上下文用量

在每次对外回复的第一个推理迭代，系统比较输入 token 与以下阈值：

```text
context_size × (trigger_ratio - context_buffer_ratio)
```

超过该阈值时，Hint 会提示模型上下文正在接近自动压缩点。后续工具迭代不会重复计算这一维度，以减少同一轮的重复提醒。

这里的 `trigger_ratio` 来自 `ContextConfig`。关闭自动压缩并不等同于关闭运行时注入；若不需要上下文用量提示，应关闭注入总开关或按应用需要调整配置。

### 额外字段

`extra_fields` 用于把应用信息附加到同一个 Hint。字段值会转义后再放入模板，避免破坏注入结构。

额外字段本身不会触发注入。只有时间、任务或上下文用量中的至少一个维度满足条件时，它们才会随 Hint 一起出现。

## 与压缩的执行关系

ReAct 在每次模型调用前按以下顺序处理状态：

1. 读取当前上下文并计算压缩预算；
2. 必要时先压缩旧消息；
3. 重新读取压缩后的 state，重新评估各注入条件；
4. 将生成的单个 `HintBlock` 追加到 `AgentState::context`；
5. 再读取最新上下文并调用模型。

这个顺序带来两个重要结果：

- 压缩移除旧 Hint 后，时间或任务维度可能在同一轮重新注入；
- 新 Hint 会进入持久上下文，参与当轮以及后续轮次的 token 预算，直到被裁剪或压缩。

自动压缩的占位摘要也会参与重新评估。详情参见[压缩上下文](compress-context)。

## 事件与持久上下文

`emit_hint_event` 只控制是否向事件流发布 `HintBlockEvent`：

- `true`：Hint 写入上下文，并发布事件；
- `false`：不发布事件，但 Hint 仍会写入上下文并发送给模型。

因此，关闭事件不能用于关闭注入。需要停用注入时，应设置 `inject_runtime_state: false`。

## 重复注入与排查

看到重复 Hint 时，按以下顺序检查：

1. **时间间隔**：`time_interval` 是否过小，或时区/格式是否变化；
2. **压缩边界**：旧 Hint 是否刚被自动压缩移除；
3. **任务工具名**：自定义任务工具是否加入 `task_tool_names`；
4. **持久状态**：应用是否在轮次之间替换或清空了 `AgentState::context`；
5. **多次构造**：是否为同一会话创建了多个互不共享 state 的 Agent；
6. **事件误判**：前端是否把 `HintBlockEvent` 与上下文中的 Hint 分别渲染成两条记录。

若完全没有 Hint，则检查 `inject_runtime_state`、时区和模板是否有效，并确认至少一个触发维度成立。仅配置 `extra_fields` 不足以触发注入。

## 相关内容

- Hint 的消息结构：[消息与事件](../message-and-event)
- 自动压缩与预算：[压缩上下文](compress-context)
- 任务工具：[计划模式](../plan)
