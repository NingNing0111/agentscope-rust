# Feature Specification: human-in-the-loop 示例改造为循环对话 + 写入工具权限批准

**Feature Branch**: `031-human-in-the-loop-repl`

**Created**: 2026-08-13

**Status**: Draft

**Input**: User description: "优化下 examples/human-in-the-loop。我希望是一个循环对话模式，然后有一个写入工具，需要写入时，申请权限批准。而不是一个流程。"

## 背景与动机

现有 `examples/human-in-the-loop` 是一个**一次性两阶段流程**：

1. 以 `ask` 规则运行一次 → 触发 `RequireUserConfirmEvent` → 宿主询问 y/n。
2. 批准后宿主**重建 allow agent 重跑同一个 prompt**，处理完一个请求即退出。

这与真实 agent 权限体验不符：用户希望像在一个终端会话里那样**持续对话**，agent 平时可自由回答，只有**需要写入时才申请权限**，宿主批准后放行写入，对话继续。

## 设计决策（经头脑风暴确认）

| 决策点 | 结论 |
|--------|------|
| 批准选项粒度 | **y / n / a**：`y`=仅本次批准；`n`=拒绝；`a`=总是允许（此后不再询问） |
| 写入工具形态 | 保留现有 **`write_note`**：把一段内容追加写入 `notes.txt` |
| 对话历史持久化 | **内存循环即可**（退出即结束，不持久化、不可恢复） |
| 引擎改动 | **不动引擎**。引擎不内置暂停/恢复状态机，批准放行由宿主在示例层驱动 |

## 用户故事与验收 (mandatory)

### User Story 1 - 循环对话下写入需授权 (Priority: P1)

用户在 REPL 里与 agent 连续对话，agent 仅在需要调用 `write_note` 时暂停并申请权限，宿主批准后工具执行、对话继续。

**Acceptance Scenarios**:

1. **Given** 用户输入非写入类请求（如「介绍一下你自己」），**When** 正常对话，**Then** 不触发任何确认，agent 直接流式回复。
2. **Given** 用户让 agent 记录内容，**When** agent 调用 `write_note`，**Then** 引擎发出 `RequireUserConfirmEvent`，宿主展示待确认工具名与参数并询问 y/n/a。
3. **Given** 宿主批准（`y`），**When** 重放该轮，**Then** `write_note` 执行成功，`notes.txt` 追加一行，对话继续。

### User Story 2 - 拒绝与"总是允许" (Priority: P2)

用户可拒绝单次写入，或授权此后所有写入不再询问。

**Acceptance Scenarios**:

1. **Given** 宿主拒绝（`n`），**When** 当前流继续消费，**Then** 模型已收到被拒结果并自行调整（如直接回答、不借助工具），不执行写入。
2. **Given** 宿主选择「总是允许」（`a`），**When** 此后 agent 再调用 `write_note`，**Then** 不再触发确认，直接执行写入。
3. **Given** 用户输入 `/exit` 或 `/quit`，**When** REPL 收到命令，**Then** 程序正常退出。

---

## 架构与实现方案

### 方案：宿主维护权威历史 + 批准时截断重放（方案 A）

核心约束（来自引擎源码）：

- `PermissionBehavior::Ask` 时引擎发出 `RequireUserConfirmEvent`，并**同时把 denied result 喂回模型**，模型自行调整；引擎**不内置暂停/恢复状态机**。
- 权限优先级为 **deny > ask > allow**，因此"批准放行"**不能**叠加 allow 规则，必须**移除 ask 规则**（重建 agent）后重放。
- `PermissionContext` 固化在 `AgentConfig` 中，agent 构建后不可运行中修改。
- `reply_stream(input)` 会把 input append 到 `state.context`，引擎回复时自动 append assistant 消息；宿主可读 `agent.state().context` 获取权威历史。

因此示例层做法：**宿主每轮新建 agent**，批准时把权威历史**截断回退到本轮用户消息**，重建 allow-only agent 重放该轮。

### 模块划分

```
examples/human-in-the-loop/src/main.rs
├── 输入类型 & 工具函数
│   ├── WriteNoteInput { content }
│   └── write_note(input) -> String            // 追加写入 notes.txt
├── 权限上下文构建
│   ├── ask_context()    -> PermissionContext  // ask("write_note")
│   └── allow_context()  -> PermissionContext  // allow("write_note")
├── build_agent(model, perm) -> ReActAgent
├── ask_user(tool_call) -> Decision            // y/n/a 从 stdin 读取
└── main: REPL 主循环
```

### 主循环（REPL）

```
打印 banner
loop:
  readline 用户输入
  | "/exit" / "/quit" → break
  | 空行 → continue
  push 用户消息到 history
  start_len = history.len()
  构建 agent（always_allow ? allow_context : ask_context）
  stream = reply_stream(history)
  事件循环:
    | TextBlockDelta      → 打印 + 累积
    | ToolCallStart / ToolResult 事件 → 打印执行过程
    | RequireUserConfirm  → ask_user(tool_call)：
    |     y → truncate(history, start_len); always_allow 不变; 重建 allow agent; 重放
    |     a → always_allow = true; truncate; 重建 allow agent; 重放
    |     n → continue（引擎已把 denied 喂回模型）
    | ReplyEnd → 本轮结束
  本轮结束: history = agent.state().context.clone()  // 同步权威历史
```

### 批准选项语义

| 用户输入 | 行为 |
|----------|------|
| `y` | 本次批准：截断历史回退到本轮用户消息，重建 `allow("write_note")` agent（无 ask 规则）重放 → 工具执行 |
| `n` | 拒绝：不重放，模型已收到 denied result，自行调整 |
| `a` | 总是允许：宿主 `always_allow=true`，重建 allow-only agent，截断重放，此后不再询问 |

### 确认展示

`RequireUserConfirmEvent` 携带 `tool_calls: Vec<ToolCallBlock>`，可读取工具名与 JSON 参数，询问时展示：

```
🔐 write_note 需要授权：{"content":"记得买牛奶"}
   将追加写入 notes.txt。批准？[y/n/a] (a=总是允许)
```

### 错误处理

- 缺 `DASHSCOPE_API_KEY` → 明确报错退出（沿用现有行为，不静默、不 panic）。
- agent reply 出错 → 打印错误，回到主循环（不崩溃）。
- stdin 读取失败 → 报错退出。
- 本轮重放后仍收到确认（allow 规则下异常）→ 打印告警并 continue（防御）。

### 不做的事（YAGNI）

- 不加会话持久化/恢复。
- 不改引擎、不加通用 `write_file`。
- 不加多工具权限矩阵。

## 测试策略

本示例依赖真实模型调用（`DASHSCOPE_API_KEY`），无法在 CI 无凭据下跑通交互。验证方式：

1. `cargo build -p human-in-the-loop` 编译通过、clippy 干净。
2. 无凭据时运行，输出明确缺凭据错误。
3. 有凭据时人工按场景走查：普通对话不确认 / 写入触发确认 / y 批准写入成功 / n 拒绝不写入 / a 之后不再询问 / `/exit` 退出。
4. `examples/human-in-the-loop/README.md` 同步更新为新交互模式说明。

## 参考

- `crates/agent_scope_agent/src/permission.rs`：`PermissionBehavior::Ask` 语义、deny>ask>allow 优先级、`suggested_rules`。
- `crates/agent_scope_event/src/control_events.rs`：`RequireUserConfirmEvent` 结构。
- 现有 `examples/human-in-the-loop/src/main.rs`：当前一次性流程实现。
- `examples/chat/src/main.rs`：事件流消费示例（`reply_stream` 事件分发模式）。
