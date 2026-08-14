# `human-in-the-loop` 示例

人机协同确认（Human-in-the-Loop）示例：一个**循环对话**模式，agent 平时自由回答，
仅在调用写入工具 `write_note` 时**暂停**并向宿主申请权限，宿主批准后以事件**恢复同一
agent** 继续执行。

## 演示内容

1. **循环对话**：进入 REPL 后可持续输入消息与 agent 对话（`/exit`、`/quit` 退出）。
2. **写入需授权**：`write_note` 配置了 `PermissionRule::ask`，模型调用它时引擎发出
   `RequireUserConfirmEvent` 并**暂停**（当前 reply_stream 结束，不喂 denied、无 ReplyEnd）。
3. **y/n/a 批准（暂停-确认-恢复）**：
   - `y`：仅本次批准，宿主注入 `UserConfirmResultEvent{confirmed:true}`，同一 agent
     恢复执行工具（写入 `notes.txt`）。
   - `n`：拒绝，宿主注入 `confirmed:false`，引擎生成 `DENIED` 结果，模型自行调整。
   - `a`：总是允许，宿主注入 `confirmed:true + rules:[allow(write_note)]`，规则采纳进
     引擎，此后调用 `write_note` 不再询问。

## 运行

```bash
cargo run -p human-in-the-loop -- --help
```

```bash
# 直接进入交互循环（需要真实模型调用）
cargo run -p human-in-the-loop

# 或带首条消息
cargo run -p human-in-the-loop -- --prompt "请把「记得买牛奶」写入笔记"
```

## 凭据

真实模型调用需要环境变量：

```bash
export DASHSCOPE_API_KEY="sk-your-key"
```

缺失或为空时程序会给出明确错误提示（不会静默失败或 panic）。

## 预期行为

- 普通对话（如「介绍一下你自己」）不触发确认，agent 直接流式回复。
- 让 agent 记录内容时触发确认；输入 `y` 后写入成功、`notes.txt` 追加一行；
  输入 `n` 模型不写入而调整回复；输入 `a` 则后续写入不再询问。
- **不截断历史、不重建 agent**：确认以 `reply_stream_event` 恢复同一实例，与 Python
  的"暂停 → 事件恢复"语义一致。
- 无凭据时：输出明确的缺凭据错误。

## 关键 API

- `PermissionRule::ask("write_note")`：为该工具设置「需确认」规则。
- `AgentEvent::RequireUserConfirm`：引擎发出的暂停信号（含待确认工具列表）。
- `agent.reply_stream_event(EventInput::Confirm(event))`：宿主以确认结果恢复暂停的回复。
- `ConfirmResult.rules`：可携带 allow 规则，恢复时被采纳进引擎（对应 `a`=总是允许）。

> 说明：Rust 引擎采用 Python 式「暂停-确认-恢复」状态机。`RequireUserConfirmEvent` 后
> reply_stream 结束，宿主读取待确认 tool_call（含 `state="asking"` 与 `suggested_rules`），
> 再以 `UserConfirmResultEvent` 恢复同一 agent，按 tool_call_id 精确匹配放行/拒绝。
