---
title: "子智能体"
description: "库级多智能体委托：SubAgent / SubAgentRegistry / delegate_* / MultiAgentConversation"
---

<Note>
**Rust 实现状态**: 已实现（兼容等级 L3）。本文档描述的能力在 AgentScope Rust 中可用，兼容基线为 AgentScope Python v2.0.5。形态为**库级多智能体委托**（`SubAgent` / `SubAgentRegistry` / `delegate_*` / `MultiAgentConversation`）；服务级的完整 team 编排框架尚未实现（见 [智能体团队](/deploy/agent-team)）。
</Note>

子智能体（SubAgent）是 AgentScope Rust 的**库级多智能体委托**能力：一个父智能体可以把职责清晰的任务委托给一组预注册的协作方，收取各自结果并继续推进主会话。与 Python 的服务级 team 不同，Rust 侧由应用层自行组装、委托与协调，运行在同一进程内。

## 核心抽象

| 类型 | 作用 |
|------|------|
| `SubAgentTemplate` | 「创建蓝图」：名称、描述、指令、能力范围、上下文策略、默认预算；`validate()` 校验后可用 `create_subagent(agent)` 派生具体实例 |
| `SubAgent` | 已注册的进程内协作方：包装一个 `Arc<dyn Agent>`，附带名称、描述、状态、能力范围与上下文策略 |
| `SubAgentRegistry` | 协作方登记表：注册模板 / 子智能体、按名称查询、启停、按选择策略挑选目标 |
| `DelegationRequest` | 一次委托请求：父智能体名、目标子智能体名、任务文本、共享上下文与预算 |
| `CollaborationResult` | 委托结果：状态（成功 / 失败 / 超时 / 取消……）、消息、错误信息、`DelegationTrace` 轨迹 |
| `MultiAgentConversation` | 多智能体对话记录：保留参与者角色与发言顺序 |
| `DelegationTrace` | 一次委托的关联事件序列（`DelegationEventType`）与净化记录 |

## 快速上手

把 `SubAgent` 与 `SubAgentRegistry` 组合成一个最小协作方清单，再用 `delegate_once` 委托任务（以下为 [`examples/subagent`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/subagent/) 源码摘录）：

```rust
use std::sync::Arc;

use agent_scope_agent::{
    Agent, ContextConfig, DelegationRequest, ReActAgent, ReActConfig, SubAgent,
    SubAgentRegistry, delegate_once,
};

// 1. 注册协作方（子智能体名必须与其内部 agent 名一致）。
let mut registry = SubAgentRegistry::new("assistant");
registry.register_subagent(SubAgent::new(
    "researcher",
    "负责信息检索与资料整理",
    Arc::new(researcher_agent),  // 任意实现了 Agent 的实例，如 ReActAgent
)?)?;

// 2. 显式委托一个任务，收取结果。
let result = delegate_once(
    &registry,
    DelegationRequest::new("assistant", "researcher", "请调研 AgentScope Rust 的多智能体能力"),
)
.await?;
```

委托内部自动完成：按名称查找并应用目标策略 → 追加任务消息 → 调用子智能体的 `reply` → 把结果包装为 `CollaborationResult` 并追加轨迹事件。

## 委托 API

`agent_scope_agent::delegation` 提供四个入口：

| 函数 | 语义 |
|------|------|
| `delegate_once` | 执行一次「最终结果」委托，返回 `CollaborationResult` |
| `delegate_many` | 批量委托；默认顺序执行，任一请求设置 `budget.allow_concurrent = true` 时改为并发（结果按输入顺序返回） |
| `delegate_stream` | 流式委托：返回事件接收器 + 最终结果，事件与轨迹关联 |
| `observe_result_by_parent` | 成功结果由父智能体 `observe` 吸收，进入其上下文 |

`delegate_once` 还提供带取消传播的变体 `delegate_once_with_cancel`，配合 `CancellationToken` 可在父侧取消时中止等待。

批量委托示例：

```rust
let requests = vec![
    DelegationRequest::new("assistant", "researcher", "列举三条 Rust 编程的最佳实践，每条约一句话"),
    DelegationRequest::new("assistant", "coder", "用 Rust 写一个返回 Vec<i32> 最大值的函数"),
    DelegationRequest::new("assistant", "reviewer", "用一句话说明你在复核环节中的职责"),
];
let results = delegate_many(&registry, requests).await?;
// 默认顺序执行；任一请求设置 budget.allow_concurrent = true 时改为并发。
```

## 上下文共享与预算

委托请求携带 `SharedContext`（消息、摘要、记忆 / 会话 / 工作空间引用）。目标子智能体的 `ContextSharingPolicy` 会**净化**调用方传入的上下文，防止越权：

```rust
let ctx_policy = ContextSharingPolicy {
    message_policy: MessageContextPolicy::SummaryOnly,
    ..ContextSharingPolicy::default()
};
let shared: SharedContext = ctx_policy.build_shared_context(&[], Some("父智能体已完成初步调研".to_string()))?;
// 若子智能体策略更严（如 message_policy = None），delegate 时越权消息会被剥离并记录 redaction_notes。
```

`DelegationBudget` 约束一次委托：最大深度、最大调用次数、超时（毫秒）、最大上下文消息数、是否允许并发。`effective_budget` 取目标默认与请求值的**更严格组合**，调用方无法放宽目标默认。`CapabilityScope` 声明目标能力范围；委托前会拒绝「完全禁用」的模型访问与副作用范围（fail-closed）。

## 选择策略

`SubAgentRegistry` 用 `SelectionPolicy` 决定如何挑选目标：

| 策略 | 语义 |
|------|------|
| `ExplicitOnly`（默认） | 必须显式给出子智能体名 |
| `ResponsibilityMatch` | 按查询词匹配名称或描述；唯一命中返回该子智能体，多命中报 `AmbiguousSubAgent`，零命中报 `MissingSubAgent` |
| `ManualApprovalRequired` | 需先人工批准（`approved = true`）才允许选择 |

## 失败与超时

委托不会以 `Err` 中断整个流程，而是把决策包装进 `CollaborationResult.status`：

- 子智能体执行报错 → `CollaborationStatus::Failed`（`result.error` 携带错误码与信息）；
- 超时（`budget.timeout_ms`）→ `TimedOut`；
- 取消 → `Cancelled`；能力范围拒绝 → `PermissionDenied`；未支持特性 → `UnsupportedFeature`。

## 多智能体对话

`MultiAgentConversation` 在进程内保留一份有序对话记录，保留发言人身份：

```rust
let mut conversation = MultiAgentConversation::new("demo-conversation-1");
conversation.add_participant("assistant", "parent");
conversation.add_participant("researcher", "subagent");
// 通过 push_message(msg) 追加发言；msg 构造细节见示例的 make_msg 辅助函数。
conversation.push_message(...);
```

## 把 SubAgent 封装成工具

除了应用层手动 `delegate_once`，更贴近「主 Agent 自主指挥」的用法是把 SubAgent 封装成工具注册进主 Agent 的 `ToolKit`，由模型的 ReAct 循环决定创建与委托时机：

| 工具 | 作用 |
|------|------|
| `SubAgentCreate(name, description, instructions)` | 创建并注册一个真实 `ReActAgent` 子智能体（幂等：同名已存在则直接提示） |
| `SubAgentDelegate(target, task)` | 把任务委托给已创建的子智能体，把结果作为工具输出回填给主 Agent |

工具与主 Agent 共享同一个 `SubAgentRegistry`（`Arc<tokio::sync::RwLock<SubAgentRegistry>>`），用 `agent_scope_tool::FunctionTool` 封装（输入结构体加 `schemars::JsonSchema` + `serde::Deserialize`，schema 自动推导）。主 Agent 得到这两个工具后，会自主完成「拆解任务 → 创建子智能体 → 逐个委托 → 汇总结果」的完整流程。

## 完整示例

见 [`examples/subagent`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/subagent/)（独立示例 crate，实现位于 [`src/main.rs`](https://github.com/NingNing0111/agentscope-rust/blob/master/examples/subagent/src/main.rs)）：

```bash
# 工具驱动：主 Agent 自主创建并委托子智能体（真实 DashScope 模型调用）
cargo run -p subagent                            # 默认模型 qwen-plus + 内置示例任务
cargo run -p subagent -- --model qwen-max        # 指定模型
cargo run -p subagent -- --task "你的自定义任务"  # 自定义任务
```

示例把 SubAgent 封装成 `SubAgentCreate` / `SubAgentDelegate` 两个工具注册进主 Agent，主 Agent 通过 ReAct 循环自主完成创建与委托；运行结束后从共享注册表列出实际创建的子智能体，验证「由主 Agent 自己创建」。

示例需要真实模型调用，凭据从项目根目录 `.env` 读取（`DASHSCOPE_API_KEY`），也支持环境变量；缺凭据时程序给出明确错误提示。

### 流式查看主 Agent 的决策过程

示例主循环用 `reply_stream` 消费事件流，**按事件类型打印核心事件**，可以直接观察到主 Agent 自主决策的完整过程：拆解任务 → 创建子智能体 → 逐个委托 → 子智能体产出回流 → 最终汇总。事件与输出标记的对应关系如下：

| 事件 | 输出标记 |
|------|----------|
| `ReplyStart` / `ReplyEnd` | `[reply start]` / `[reply end]`（含 `finished_reason`） |
| `ModelCallStart` | `[model call]`（含模型名，每次模型调用一次） |
| `ToolCallStart` / `ToolCallEnd` | `[tool call]` / `[tool end]`（`SubAgentCreate` / `SubAgentDelegate` 的调用参数） |
| `ToolResultStart` / `ToolResultTextDelta` / `ToolResultEnd` | `[tool result]` 与增量文本（子智能体的产出以此流回） |
| `TextBlockDelta` | 主 Agent 汇总文本逐字实时输出 |
| `ThinkingBlockDelta` | 思考增量（示例中以暗色显示，区别于正文） |

事件消费模式（与 [`examples/chat`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/chat) 一致）：

```rust
use agent_scope_event::AgentEvent;
use futures::StreamExt;

let mut stream = main_agent.reply_stream(Some(msgs)).await?;
while let Some(event) = stream.next().await {
    match &event {
        AgentEvent::ModelCallStart(e) => println!("[model call] {}", e.model_name),
        AgentEvent::TextBlockDelta(d) => print!("{}", d.delta),
        AgentEvent::ToolCallStart(s) => println!("[tool call] {}", s.tool_call_name),
        AgentEvent::ToolResultStart(r) => println!("[tool result] {}", r.tool_call_name),
        AgentEvent::ReplyEnd(e) => println!("[reply end] {:?}", e.finished_reason),
        _ => {}
    }
    std::io::stdout().flush().ok();
}
```

若委托的目标子智能体超时（`DelegationBudget.timeout_ms`），主 Agent 会收到 `ToolResultEnd` 返回的失败结果文本，并据其在后续循环里**附带上下文重新委托**——这种「自主纠错」正是流式事件观察 ReAct 闭环的价值所在。

## 兼容性等级

| 模块 | 兼容等级 | 说明 |
|------|----------|------|
| 库级多智能体委托（SubAgent / Registry / delegate_* / MultiAgentConversation） | L3 | 进程内父→子委托，与应用层协调的协作组装 |

## 延伸阅读

- [智能体团队](/deploy/agent-team) — 服务级 team 编排与 Rust 侧缺失边界
- [Agent 概述](/building-blocks/agent/overview) — `Agent` trait 与 `ReActAgent` 主循环
- [运行智能体](/building-blocks/agent/run-agent) — `reply` / `reply_stream` / `observe`
