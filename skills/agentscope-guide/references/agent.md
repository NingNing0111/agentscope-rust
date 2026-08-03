# 参考:Agent 系统(`agent_scope_agent`)

> 详细 API 参考:`Agent` trait、`ReActAgent`、`AgentConfig`/`ReActConfig`/`ContextConfig`、`Middleware`、权限系统、`Planner`、`SubAgent`。

## 1. `Agent` trait

所有 Agent 类型的共同接口(`async_trait`,支持 `Arc<dyn Agent>`):

| 方法 | 说明 |
|------|------|
| `reply(input)` | 非流式调用入口;返回最终 assistant `Msg` |
| `reply_stream(input)` | 流式调用入口;返回 `Stream<Item = AgentEvent>` |
| `observe(input)` | 只追加消息到上下文,不触发模型回复 |
| `name()` | Agent 配置名 |
| `state()` | trait 层状态访问;`ReActAgent` 请用 `try_state()` |

`reply(None)` 语义:基于已有上下文继续回复;上下文为空返回 `AgentError::NoContentToReply`。

## 2. `ReActAgent` 内部循环

```text
用户输入 / 已有上下文
→ middleware.pre_reply
→ middleware.on_system_prompt
→ loop(max_iters):
   → middleware.pre_reasoning
   → model.call(messages, tool_schemas, tool_choice)
   → middleware.post_reasoning
   → 若模型返回文本:累积为最终回复
   → 若模型返回 ToolCallBlock:权限检查 → pre_acting → ToolKit.call_tool → post_acting
   → 工具结果追加回上下文,进入下一轮
→ middleware.post_reply
→ 返回最终 Msg 或事件流收尾
```

要点:
- 支持非流式 `Complete(ChatResponse)` 与流式 `Stream(...)`;非流式 `reply()` 路径用 `StreamAccumulator` 累积。
- **同一 Agent 同时只允许一个回复活跃**,并发启动第二个 → `AgentError::AlreadyStreaming`。
- `interrupt()` 可中断进行中的回复;发出 `UserInterrupt`,以 `ReplyEnd(finished_reason: interrupted)` 收尾。
- **`ReActAgent::state()` 会 panic**,读取状态用 `try_state()`。

## 3. `AgentConfig`(builder)

| builder | 说明 |
|---------|------|
| `name(...)` | 必填;用于消息和事件中的 `name` |
| `system_prompt(...)` | 系统提示词,可为空 |
| `model(...)` | `Arc<dyn ChatModel>`,必填 |
| `toolkit(...)` | 可选工具注册表 |
| `permission_context(...)` | 工具执行权限上下文 |
| `permission_mode(...)` | 权限模式 |
| `with_stream_channel_capacity(...)` | 流式通道容量;`None` 无界,`Some(n)` 需 `n > 0` |

最小构造需 `name` + `model`;缺任一必填项 `build()` 返回 `AgentError::InvalidConfig`。

## 4. `ReActConfig`

| 字段 | 默认 | 说明 |
|------|------|------|
| `max_iters` | `20` | 单次回复最多 reasoning/acting 迭代数,必须 > 0 |
| `stop_on_reject` | `false` | 工具权限拒绝时是否停止 |
| `interruption_message` | `"The execution was interrupted."` | 中断时的 assistant 文本 |
| `structured_output_grace_iters` | `3` | 结构化输出解析失败的容错迭代数 |

## 5. `ContextConfig`(上下文压缩)

| 字段 | 默认 | 说明 |
|------|------|------|
| `enable` | `false` | 是否启用压缩 |
| `trigger_ratio` | `0.8` | token 超过 `context_size * trigger_ratio` 时触发 |
| `reserve_ratio` | `0.1` | 为模型回复保留的上下文比例 |
| `compression_prompt` | `"<STD_CP_PROMPT>"` | 压缩模型调用的系统提示 |
| `tool_result_limit` | `4096` | 工具结果内容截断限制 |

## 6. `Middleware`(9 个 hook)

全部默认 no-op,按注册顺序 FIFO 调用:

| Hook | 时机 | 常见用途 |
|------|------|----------|
| `pre_reply` | 回复开始前 | 修改输入、启动检索、捕获模型引用 |
| `post_reply` | 回复结束后 | 记录日志、持久化 |
| `on_system_prompt` | 首次模型调用前 | 追加记忆、策略、动态说明 |
| `pre_reasoning` | 每轮模型调用前 | 修改上下文消息或工具 schema |
| `post_reasoning` | 模型返回后 | 记录响应、统计用量 |
| `pre_acting` | 工具执行前 | 修改或拒绝工具调用 |
| `post_acting` | 工具执行后 | 记录工具结果、触发副作用 |
| `pre_observe` | `observe()` 调用时 | 规范化被观察消息 |
| `pre_print` | 输出渲染前 | 修改展示内容 |

自定义 middleware 示例:

```rust
use agent_scope_agent::{AgentError, Middleware};
use agent_scope_message::Msg;
use agent_scope_model::ChatModel;
use std::sync::Arc;

struct AuditMiddleware;

#[async_trait::async_trait]
impl Middleware for AuditMiddleware {
    async fn pre_reply(
        &self,
        agent_name: &str,
        input: &mut Option<Vec<Msg>>,
        _model: &Arc<dyn ChatModel>,
    ) -> Result<(), AgentError> {
        tracing::info!(agent = agent_name, has_input = input.is_some(), "reply started");
        Ok(())
    }
}
```

## 7. 权限系统

权限模式:

| 模式 | 默认行为 |
|------|----------|
| `Default` | 无匹配规则时允许 |
| `AcceptEdits` | 无匹配规则时允许 |
| `Explore` | 只读规划模式;无 allow 规则时拒绝未分类工具 |
| `Bypass` | 无匹配规则时允许 |
| `DontAsk` | ask 决策转 deny;无匹配规则时允许 |

规则优先级:`deny` → `ask` → `allow` → 模式默认值。支持精确匹配、`*` 全匹配、`prefix*` 前缀匹配;可用 `rule_content` 对序列化后的工具输入做子串匹配。

```rust
use agent_scope_agent::{PermissionContext, PermissionMode, PermissionRule};

let mut permission = PermissionContext::new(PermissionMode::Explore);
permission.add_rule(PermissionRule::allow("calculator"));
permission.add_rule(PermissionRule::deny("shell*"));

let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .toolkit(toolkit)
    .permission_context(permission)
    .build()?;
```

## 8. `Planner`(多步骤任务)

增量式 `Planner` 在任意 `Agent` 之上执行确定性的多步骤任务:

```rust
use std::sync::Arc;
use agent_scope_agent::{Planner, PlannerConfig};

let planner = Planner::new(
    Arc::new(agent),          // 可执行 ReAct 的 Agent
    Arc::new(planner_model),  // 输出 {"objective":"...","steps":[...]} 的模型
    PlannerConfig::default(),
)?;
let result = planner.run("准备发布摘要").await?;
```

- 规划模型输出 JSON plan(`objective` + `steps`),`Planner` 逐步骤驱动执行 Agent。
- 可恢复的步骤失败触发显式 replanning,直到 `PlannerConfig::max_replans`。
- 终态:`Completed` / `PartiallyCompleted` / `Cancelled` / `Failed` / `Unsupported`。
- `run_stream` 将生命周期转成 `AgentEvent::Custom`(`name = "planner.lifecycle"`)。

## 9. `SubAgent`(进程内委派)

父 Agent 通过 `SubAgentRegistry` 注册具名 `SubAgent` 或 `SubAgentTemplate`,用显式 `DelegationRequest` 调用 `delegate_once()` / `delegate_many()`:

```rust
use agent_scope_agent::{
    SubAgent, SubAgentRegistry, DelegationRequest, delegate_once,
};

let mut registry = SubAgentRegistry::default();
registry.register(Arc::new(SubAgent::new("researcher", researcher_agent)));

let result = delegate_once(
    &registry,
    DelegationRequest { participant: "researcher".into(), task: "...".into(), ..Default::default() },
    &agent,
).await?;
```

- 成功结果以 `CollaborationResult` 返回,状态 `CollaborationStatus::Succeeded`,结果 `Msg.name` 保留 SubAgent 说话者身份。
- `ContextSharingPolicy` 默认最小权限;`CapabilityScope` 控制工具/记忆/会话/workspace/sandbox/模型访问与副作用权限。
- Python app-service/message-bus/分布式等能力返回 `SubAgentErrorCategory::UnsupportedFeature`,不会静默伪装成功。

## 10. 错误

| 错误 | 常见原因 | 处理建议 |
|------|----------|----------|
| `InvalidConfig` | 缺 name/model 或配置非法 | 构造阶段 fail-fast |
| `NoContentToReply` | `reply(None)` 且上下文为空 | 先传消息或 `observe()` |
| `AlreadyStreaming` | 已有活跃回复 | 消费完或 drop stream 再启动 |
| `ModelError` | Provider 调用失败 | 按认证/限流/网络分类处理 |
| `ToolError` | 工具不存在、输入非法、执行失败 | 检查注册与模型生成的 JSON |
| `PermissionDenied` | 权限规则拒绝 | 调整 `PermissionContext` |
| `MaxItersExceeded` | ReAct 循环超过 `max_iters` | 增大上限或改进提示词 |
| `CancellationError` | 回复被取消 | 通常作为正常控制流处理 |
| `ContextCompressionFailed` | 压缩模型调用失败 | 关闭压缩或检查模型 |

## 11. 常见坑

1. **`chat.rs` 不自动读环境变量作为 API key**:只接受显式传参。
2. **不要并发启动同一 Agent 的两个回复** → `AlreadyStreaming`。
3. **`reply(None)` 需要已有上下文**。
4. **流式消费必须读到结束或主动 drop stream**。
5. **`ReActAgent::state()` 不适合直接调用**,用 `try_state()`。
6. **权限默认不是 sandbox**:`Default` 模式无匹配规则时允许工具调用;只读场景显式用 `PermissionMode::Explore`。
