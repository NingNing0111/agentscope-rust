# `human-in-the-loop` 示例

人机协同确认（Human-in-the-Loop）示例：一个**循环对话**模式，agent 平时自由回答，
仅在调用写入工具 `write_note` 时暂停并向宿主申请权限，宿主批准后写入、对话继续。

## 演示内容

1. **循环对话**：进入 REPL 后可持续输入消息与 agent 对话（`/exit`、`/quit` 退出）。
2. **写入需授权**：`write_note` 配置了 `PermissionRule::ask`，模型调用它时引擎发出
   `RequireUserConfirmEvent` 并把被拒结果喂回模型。
3. **y/n/a 批准**：
   - `y`：仅本次批准，宿主重建 allow agent 重放该轮，工具执行（写入 `notes.txt`）。
   - `n`：拒绝，模型已收到被拒结果，自行调整（如不借助工具直接回答）。
   - `a`：总是允许，宿主记住授权，此后调用 `write_note` 不再询问。

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
- 无凭据时：输出明确的缺凭据错误。

## 关键 API

- `PermissionRule::ask("write_note")`：为该工具设置「需确认」规则。
- `AgentEvent::RequireUserConfirm`：引擎发出确认事件（含待确认工具列表）。
- `agent.state().context`：权威对话历史，宿主每轮结束后据此同步。
- `history.truncate(start_len)`：批准后把历史回退到本轮用户消息，重建 allow agent 重放。

> 说明：Rust 引擎本身不内置「暂停/恢复」状态机（见 `PermissionResult::RequireConfirm`
> 文档）。确认闭环由宿主驱动——批准时截断历史回退并重建带 allow 规则的 agent 重放，
> 拒绝时模型基于被拒结果自行调整。
