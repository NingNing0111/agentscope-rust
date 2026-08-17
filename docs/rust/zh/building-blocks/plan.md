---
title: "计划模式"
description: "为智能体提供结构化任务清单，用于规划、追踪并协调复杂工作"
---

<Note>
**Rust 实现状态**: 已实现（兼容等级 L3）——形态为**内置任务规划工具**（Feature 024），替代早期独立的 Planner。兼容基线为 AgentScope Python v2.0.5。
</Note>

计划（Planning）是智能体把复杂请求拆分成离散、有序、可追踪步骤的方式。AgentScope Rust 通过一小组内置工具，让智能体用工具调用来维护一份**显式、结构化的任务清单**——任务的创建、查询与更新都走工具调用。

## 内置任务工具

| 工具 | 操作 | 只读 |
|------|------|------|
| `TaskCreate` | 向任务清单追加新任务（`subject` + `description`，可附 `metadata`） | 否 |
| `TaskList` | 列出所有任务及其状态、owner、阻塞关系 | 是 |
| `TaskGet` | 按 ID 获取单个任务的完整信息（描述、状态、依赖边、元数据） | 是 |
| `TaskUpdate` | 更新任务的状态、字段或依赖边，亦可删除任务 | 否 |

四个工具的 `description` 已内置详细的使用指引（何时调用、何时跳过、如何解读输出），无需额外系统提示工程。

## 自动装配

四个工具在 `ReActAgent` 构造时自动注册（`task_tools_enabled` 默认 `true`）：

```rust
use agent_scope_agent::{AgentConfig, ReActAgent};

let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .build()?;  // task_tools_enabled 默认 true

let agent = ReActAgent::new(
    config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![],
)?;
// 注册后，agent 可通过 TaskCreate/TaskList/TaskGet/TaskUpdate 维护任务清单。
```

需要关闭时调用 `.task_tools_enabled(false)`：

```rust
let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .task_tools_enabled(false)  // 不注册内置任务工具
    .build()?;
```

装配细节：

- 工具名是**保留名**：若自定义工具与 `TaskCreate` / `TaskGet` / `TaskList` / `TaskUpdate` 重名，构造会失败并给出明确错误，提示改名或关闭任务工具。
- 权限：内置任务工具只读写智能体自身的任务状态，因此会**绕过限制性权限模式的默认拦截**；但显式规则仍然生效（尤其是显式 `deny` 优先级最高）。

## 任务状态注入

任务清单以智能体为作用域（`AgentState::tasks_context`），并随智能体状态持久化。此外，运行时状态注入会在上下文长度/未完成任务维度提醒智能体（见 [环境感知](context/environment-awareness)）：

- 存在未完成任务且智能体尚未感知时，注入 `<tasks>…</tasks>` 提醒；
- `task_tools_enabled = false` 会同时关闭该任务维度的注入。

## 任务生命周期

任务 ID 是稳定且单调递增的数字串（`"1"`、`"2"`……，由 `TaskCreate` 分配）。典型规划循环：

1. **登记工作** — 收到新指令时，对每个离散步骤分别调用一次 `TaskCreate`；
2. **查看队列** — `TaskList` 返回紧凑摘要，挑选下一个可做的任务（通常是最小 ID 且无未解 `blocked_by` 的 `pending` 任务）；
3. **认领并开始** — `TaskUpdate` 把状态置为 `in_progress`；
4. **获取完整上下文** — 描述较长时先用 `TaskGet` 拉取完整信息；
5. **完成或重新规划** — 完成后 `TaskUpdate` 置为 `completed`；发现新工作则回到 `TaskCreate`；不再需要的任务置为 `deleted`（硬删除，同时清理所有引用它的依赖边）。

状态流转刻意保持线性：

```
pending → in_progress → completed
                          (或)
                      ↘ deleted（任意状态均可，硬删除）
```

以下是智能体在一次回复中维护清单的完整工具调用序列（JSON 为工具输入/输出，实际由模型与运行时产生）：

```text
── 第 1 轮：登记 ──────────────────────────────────────────────
[agent] TaskCreate
  input:  {"subject": "收集项目需求", "description": "阅读 README 与 CONTRIBUTING，整理需求清单"}
  output: Task (id=1) created successfully: 收集项目需求

[agent] TaskCreate
  input:  {"subject": "起草实现计划", "description": "基于需求清单产出分步实现计划"}
  output: Task (id=2) created successfully: 起草实现计划

── 第 2 轮：建立依赖、认领并开始 ─────────────────────────────
[agent] TaskUpdate
  input:  {"task_id": "2", "add_blocked_by": ["1"]}          # 任务 2 依赖任务 1
  output: Update task (id=2) add_blocked_by.

[agent] TaskUpdate
  input:  {"task_id": "1", "status": "in_progress"}
  output: Update task (id=1) status.

── 第 3 轮：执行任务 1，完成后进入任务 2 ──────────────────────
[agent] TaskUpdate
  input:  {"task_id": "1", "status": "completed"}
  output: Update task (id=1) status.

         Task completed. Call TaskList now to find your next
         available task or see if your work unblocked others.

[agent] TaskList
  input:  {}
  output: 1 [completed] 收集项目需求
          2 [pending] 起草实现计划 [blocked by 1]
```

`TaskList` 的输出格式为每任务一行的紧凑摘要：`<id> [<status>] <subject>(<owner>)[blocked by <ids>]`。`TaskGet` 则返回完整详情（状态、描述、owner、`Blocks:` / `Blocked by:` 依赖边、`Metadata:`）。

## 表达依赖

任务暴露两条对称的依赖边：

- `blocks` — 在本任务完成前不能开始的任务 ID 列表；
- `blocked_by` — 必须在本任务开始前完成的任务 ID 列表。

`TaskUpdate` 接受 `add_blocks` 与 `add_blocked_by` 参数，每次调用都会**自动修改两端**，保持数据一致：

```text
// 创建好任务 "1" 与 "2" 后，让 "2" 依赖 "1"：
{"task_id": "2", "add_blocked_by": ["1"]}
// 此时：task "2".blocked_by == ["1"] 且 task "1".blocks == ["2"]
```

任务被删除时，其 ID 会从其他所有任务的 `blocks` 与 `blocked_by` 中移除，保证依赖图始终有效。

<Note>
`TaskList` 会标注每个仍有未解 `blocked_by` 的任务，`TaskGet` 则返回完整的依赖边列表。智能体据此优先选择无阻塞的工作，但**执行层面是仅建议性的**——运行时不会阻止模型去做一个被阻塞的任务。
</Note>

## 完整示例

`examples/agent` 示例演示了包含任务工具在内的完整编排（权限规则、中断、流式回复）。其运行输出会先确认任务工具已注册：

```bash
cargo run -p agent -- --prompt "请先规划再执行：1) 读取 examples/agent/Cargo.toml；2) 汇报其中的依赖。"
```

一个可直接运行的完整骨架（需要 `DASHSCOPE_API_KEY`，真实模型调用）：

```rust
use std::sync::Arc;

use agent_scope_agent::{Agent, AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_dashscope::DashScopeChatModel;
use agent_scope_event::AgentEvent;
use agent_scope_message::factory::user_msg;
use futures::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    let api_key = std::env::var("DASHSCOPE_API_KEY")
        .map_err(|_| anyhow::anyhow!("error: 缺少环境变量 DASHSCOPE_API_KEY。请设置后重试。"))?;

    let model = Arc::new(DashScopeChatModel::new(&api_key, "qwen-plus").with_stream(true));

    // 内置任务工具（TaskCreate/TaskList/TaskGet/TaskUpdate）默认自动注册。
    let config = AgentConfig::builder()
        .name("assistant")
        .system_prompt(
            "你是一个任务规划助手。面对多步工作时，先用 TaskCreate 拆分任务，\
             执行时用 TaskUpdate 标记 in_progress，完成后标记 completed。",
        )
        .model(model)
        .build()?;

    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )?;

    // 任务清单可通过智能体状态读取（如检查注入的任务工具是否就绪）。
    let state = agent.try_state();
    println!("session_id = {}", state.session_id);
    drop(state);

    let msg = user_msg(
        "user",
        "请规划并执行：1) 总结这个仓库的用途；2) 列出其中两个 crate 的名字。",
    )?;

    let mut stream = agent.reply_stream(Some(vec![msg])).await?;
    while let Some(event) = stream.next().await {
        match &event {
            AgentEvent::ToolCallStart(s) => {
                println!("\n[tool] {} ->", s.tool_call_name);
            }
            AgentEvent::ToolResultTextDelta(d) => print!("{d}"),
            AgentEvent::TextBlockDelta(d) => print!("{d}"),
            AgentEvent::ReplyEnd(e) => println!("\n[end] {:?}", e.finished_reason),
            _ => {}
        }
    }
    Ok(())
}
```

运行后，智能体应当先调用 `TaskCreate` 建立清单，再逐项执行并在完成后用 `TaskUpdate` 翻转状态；回复结束时可通过 `agent.try_state().tasks_context.tasks` 读取最终清单，确认每个任务均已 `completed`。

## 延伸阅读

- [运行智能体](agent/run-agent) — `reply_stream` 与事件消费
- [环境感知](context/environment-awareness) — 未完成任务维度的运行时注入
- [消息与事件](message-and-event) — `ToolCallStart` / `ToolResultTextDelta` 等事件语义
