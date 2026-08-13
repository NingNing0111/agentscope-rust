# human-in-the-loop REPL 改造 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `examples/human-in-the-loop` 从一次性两阶段流程改造为循环对话 REPL：agent 平时自由回答，仅在调用写入工具 `write_note` 时申请权限，宿主以 y/n/a 批准/拒绝/总是允许。

**Architecture:** 宿主每轮构建 `ReActAgent`（`always_allow` 时用 allow-only 权限上下文，否则用 ask 上下文），传入内存 `history` 调用 `reply_stream`。消费事件流时遇 `RequireUserConfirm` 暂停并询问用户；批准（y/a）时把历史截断回退到本轮用户消息、重建 allow agent 重放；拒绝（n）时继续消费（引擎已把 denied 结果喂回模型）。每轮结束从 `agent.state().context` 同步权威历史。

**Tech Stack:** Rust / tokio / futures / clap / agent_scope_agent（ReActAgent / PermissionRule）/ agent_scope_dashscope / agent_scope_event（AgentEvent）/ agent_scope_message（factory::user_msg）/ agent_scope_tool（FunctionTool / ToolKit）

## Global Constraints

- 不改动任何 `crates/` 源码，全部改动限定在 `examples/human-in-the-loop/`。
- 沿用现有错误处理风格：缺 `DASHSCOPE_API_KEY` 时明确报错退出，不静默、不 panic。
- 交互文案用中文（与现有示例一致）。
- 沿用项目命令约定：所有 git/cargo 命令以 `rtk` 前缀。
- 不引入新依赖（现有 Cargo.toml 依赖足够）。

---

### Task 1: 重写 main.rs 为循环对话 REPL + y/n/a 权限批准

**Files:**
- Modify: `examples/human-in-the-loop/src/main.rs`（整体重写）

**Interfaces:**
- Produces:
  - `struct WriteNoteInput { content: String }` — `write_note` 工具输入（`Deserialize + JsonSchema`）
  - `async fn write_note(input: WriteNoteInput) -> String` — 追加写入 `notes.txt`
  - `fn ask_context() -> PermissionContext` — `ask("write_note")`
  - `fn allow_context() -> PermissionContext` — `allow("write_note")`
  - `fn build_agent(model: Arc<DashScopeChatModel>, perm: PermissionContext) -> anyhow::Result<ReActAgent>`
  - `enum Approval { Approved, Rejected, AlwaysAllow }`
  - `fn ask_user(tool_name: &str, input: &str) -> io::Result<Approval>`

- [ ] **Step 1: 确认当前文件基线**

先读当前 `examples/human-in-the-loop/src/main.rs`，确认已包含：`WriteNoteInput`、`write_note`、`ask_context`、`build_agent`、`ask_user` 等函数的现有形态，重写时复用其中不变的部分（工具函数与权限上下文构造）。

- [ ] **Step 2: 写新的完整 main.rs**

整体替换文件内容为下方代码。要点：

- **REPL 主循环**：`loop` 内 `read_line`，`/exit`/`/quit` 退出、空行跳过，否则 push 到 `history`，记 `start_len`，进入 `run_turn`。
- **`run_turn`**：内部循环——按 `always_allow` 选权限上下文构建 agent，`reply_stream(Some(history.clone()))`，消费事件；`RequireUserConfirm` 时 `ask_user`，y/a 则 `history.truncate(start_len)`、必要时置 `always_allow=true` 并继续外层循环（重放）；n 则继续消费；`ReplyEnd` 后从 `agent.state().context` 同步 `history` 并返回。
- **事件分发**：`ThinkingBlockDelta` 暗淡打印、`TextBlockDelta` 正常打印、`ToolCallStart`/`ToolResultTextDelta` 打印执行过程、`ReplyEnd` 打印结束原因，其余忽略。

```rust
//! Human-in-the-loop example: a loop-mode conversational agent whose `write_note`
//! tool is guarded by a permission `ask` rule. The host (this program) drives
//! approval / rejection interactively, y/n/a.
//!
//! Flow:
//!   1. REPL 循环：用户在 stdin 输入消息，agent 流式回复。
//!   2. agent 平时可自由回答；调用 `write_note` 时，`ask` 规则使引擎发出
//!      `RequireUserConfirmEvent`，并把被拒结果喂回模型。
//!   3. 宿主询问 y/n/a：
//!        y = 本次批准：截断历史回退到本轮用户消息，重建 allow agent 重放 → 写入成功
//!        n = 拒绝：不重放，模型已收到被拒结果，自行调整
//!        a = 总是允许：宿主置 always_allow，此后重建 allow agent，不再询问
//!
//! Requires `DASHSCOPE_API_KEY` for real model calls.

use std::io::{self, Write};
use std::sync::Arc;

use agent_scope_agent::{
    Agent, AgentConfig, ContextConfig, PermissionContext, PermissionMode, PermissionRule,
    ReActAgent, ReActConfig,
};
use agent_scope_dashscope::DashScopeChatModel;
use agent_scope_event::AgentEvent;
use agent_scope_message::factory::user_msg;
use agent_scope_tool::{FunctionTool, ToolKit};
use clap::Parser;
use futures::StreamExt;
use schemars::JsonSchema;
use serde::Deserialize;

/// Input for the `write_note` tool (the one guarded by the `ask` rule).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct WriteNoteInput {
    content: String,
}

/// Append a line to `notes.txt` in the current directory.
async fn write_note(input: WriteNoteInput) -> String {
    use tokio::io::AsyncWriteExt;

    match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("notes.txt")
        .await
    {
        Ok(mut file) => match file
            .write_all(format!("- {}\n", input.content).as_bytes())
            .await
        {
            Ok(_) => format!("已把「{}」写入 notes.txt", input.content),
            Err(e) => format!("写入笔记失败: {e}"),
        },
        Err(e) => format!("打开笔记文件失败: {e}"),
    }
}

/// Permission context that requires confirmation for `write_note`.
fn ask_context() -> PermissionContext {
    let mut perm = PermissionContext::new(PermissionMode::Default);
    perm.add_rule(PermissionRule::ask("write_note"));
    perm
}

/// Permission context with `write_note` allowed outright (no ask rule).
fn allow_context() -> PermissionContext {
    let mut perm = PermissionContext::new(PermissionMode::Default);
    perm.add_rule(PermissionRule::allow("write_note"));
    perm
}

fn build_agent(
    model: Arc<DashScopeChatModel>,
    perm: PermissionContext,
) -> anyhow::Result<ReActAgent> {
    let mut toolkit = ToolKit::new();
    toolkit.register(FunctionTool::new(
        "write_note",
        "把一段内容追加写入当前目录的 notes.txt。",
        write_note,
    ));

    let agent_config = AgentConfig::builder()
        .name("assistant")
        .system_prompt(
            "你是一个乐于助人的助手。当用户说要把内容记下来时，\
             请使用 write_note 工具写入笔记。",
        )
        .model(model)
        .toolkit(toolkit)
        .permission_context(perm)
        .build()?;

    Ok(ReActAgent::new(
        agent_config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )?)
}

/// Host decision for a pending tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Approval {
    /// 仅本次批准（y）。
    Approved,
    /// 拒绝（n），模型基于被拒结果自行调整。
    Rejected,
    /// 总是允许（a），此后不再询问。
    AlwaysAllow,
}

/// Ask the user to approve / reject / always-allow a pending tool call.
fn ask_user(tool_name: &str, input: &str) -> io::Result<Approval> {
    print!(
        "\n🔐 {tool_name} 需要授权：{input}\n   批准该调用？[y/n/a] (a=总是允许) "
    );
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(match line.trim().to_lowercase().as_str() {
        "a" | "always" => Approval::AlwaysAllow,
        "n" | "no" => Approval::Rejected,
        _ => Approval::Approved,
    })
}

/// Run one user turn against `history`. Returns `Ok(())` when the turn ends.
///
/// On approval (`y`/`a`), truncates `history` back to `start_len`, rebuilds an
/// allow-context agent and replays — the write executes and the turn continues.
async fn run_turn(
    model: &Arc<DashScopeChatModel>,
    history: &mut Vec<agent_scope_message::Msg>,
    always_allow: &mut bool,
    start_len: usize,
) -> anyhow::Result<()> {
    loop {
        // Rebuild per replay: permission context is fixed at construction time.
        let perm = if *always_allow {
            allow_context()
        } else {
            ask_context()
        };
        let agent = build_agent(Arc::clone(model), perm)?;

        let stream = agent.reply_stream(Some(history.clone())).await?;
        let mut replay = false;
        let mut stream = stream;
        while let Some(event) = stream.next().await {
            match &event {
                AgentEvent::ThinkingBlockDelta(d) => print!("\x1b[2m{}\x1b[0m", d.delta),
                AgentEvent::TextBlockDelta(d) => print!("{}", d.delta),
                AgentEvent::ToolCallStart(s) => {
                    println!("\n[tool start] {}", s.tool_call_name)
                }
                AgentEvent::ToolResultTextDelta(d) => println!("\n[tool result] {}", d.delta),
                AgentEvent::RequireUserConfirm(c) => {
                    for tc in &c.tool_calls {
                        match ask_user(&tc.name, &tc.input)? {
                            Approval::Approved => {
                                println!("[human-in-the-loop] 本次批准 {tc.name}，重放本轮…");
                            }
                            Approval::AlwaysAllow => {
                                println!("[human-in-the-loop] 总是允许 {tc.name}，重放本轮…");
                                *always_allow = true;
                            }
                            Approval::Rejected => {
                                println!("[human-in-the-loop] 已拒绝 {tc.name}，模型自行调整…");
                                continue;
                            }
                        }
                        history.truncate(start_len);
                        replay = true;
                        break;
                    }
                }
                AgentEvent::ReplyEnd(e) => {
                    println!("\n[reply end] {:?}", e.finished_reason)
                }
                _ => {}
            }
            if replay {
                break;
            }
        }
        // 流已消费完（或被 replay 提前断开），agent 仍持有最新 state。
        drop(stream);

        if replay {
            continue; // 重新构建 allow agent 重放本轮
        }

        // 同步权威历史：reply_stream 会把 user 消息 append 到 context，assistant
        // 回复与工具结果也自动记录。以 agent 的 context 为准回写 history。
        let context = agent.state().context.clone();
        history.clear();
        history.extend(context);
        return Ok(());
    }
}

#[derive(Parser)]
struct Cli {
    /// 首条用户消息；留空则直接进入交互循环。
    #[arg(short, long)]
    prompt: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    dotenv::dotenv().ok();

    let api_key = std::env::var("DASHSCOPE_API_KEY")
        .map_err(|_| anyhow::anyhow!("error: 缺少环境变量 DASHSCOPE_API_KEY。请设置后重试。"))?;
    let model = Arc::new(DashScopeChatModel::new(&api_key, "qwen-plus").with_stream(true));

    let mut history: Vec<agent_scope_message::Msg> = Vec::new();
    let mut always_allow = false;

    println!("=== human-in-the-loop REPL ===");
    println!("输入消息与 agent 对话；输入 /exit 或 /quit 退出。");
    println!("写入工具 write_note 需授权时程序会询问 [y/n/a]。");

    let mut pending_first = cli.prompt;
    loop {
        // `--prompt` 只在首轮使用，随后退化为交互输入。
        let trimmed: String = if let Some(p) = pending_first.take() {
            p
        } else {
            print!("\n你> ");
            io::stdout().flush()?;
            let mut line = String::new();
            if io::stdin().read_line(&mut line)? == 0 {
                break; // EOF 静默退出
            }
            line.trim().to_string()
        };

        if trimmed.is_empty() {
            continue;
        }
        if matches!(trimmed.as_str(), "/exit" | "/quit") {
            break;
        }

        let msg = user_msg("user", &trimmed).map_err(|e| anyhow::anyhow!("{e:?}"))?;
        history.push(msg);
        let start_len = history.len();
        run_turn(&model, &mut history, &mut always_allow, start_len).await?;
    }

    Ok(())
}
```

> 注：`run_turn` 内消费 `RequireUserConfirm` 时，`for tc in &c.tool_calls` 循环里 `Approved`/`AlwaysAllow` 分支设置 `replay = true` 后 `break`，`Rejected` 分支 `continue` 到下一个待确认工具。事件循环外层用 `if replay { break; }` 提前结束本轮事件流，随后 `drop(agent)`、`continue` 外层循环完成重放。每轮正常结束（`replay == false`）时用 `agent2.state().context.clone()` 同步权威历史。

- [ ] **Step 3: 编译验证**

```bash
rtk cargo build -p human-in-the-loop
```
Expected: 编译通过（0 error）。

- [ ] **Step 4: clippy 验证**

```bash
rtk cargo clippy -p human-in-the-loop --all-targets
```
Expected: 无警告（或仅无害提示）。若 clippy 对未使用 `pending_first` 等报死代码警告，按提示清理。

- [ ] **Step 5: 无凭据错误路径验证**

```bash
DASHSCOPE_API_KEY= rtk cargo run -p human-in-the-loop -- --prompt hi 2>&1 | head
```
Expected: 输出明确缺 `DASHSCOPE_API_KEY` 错误并退出（非 panic）。

- [ ] **Step 6: fmt**

```bash
rtk cargo fmt -p human-in-the-loop -- --check
```
Expected: 通过（不通过则运行 `rtk cargo fmt -p human-in-the-loop` 后重跑 check）。

- [ ] **Step 7: Commit**

```bash
rtk git add examples/human-in-the-loop/src/main.rs
rtk git commit -m "feat(example): human-in-the-loop 改造为循环对话 REPL + y/n/a 写入授权"
```

---

### Task 2: 更新 README 反映新交互模式

**Files:**
- Modify: `examples/human-in-the-loop/README.md`

**Interfaces:**
- Consumes: Task 1 的行为语义（REPL、y/n/a、`--prompt` 可选首条消息）。

- [ ] **Step 1: 重写 README 正文**

将 README 替换为描述新交互模式的版本（沿用现有标题结构与风格，保留「关键 API」与「凭据」小节）：

```markdown
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

缺失凭据时程序会给出明确错误提示（不会静默失败或 panic）。

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
```

- [ ] **Step 2: 检查引用一致性**

在 README 中 grep 是否还有描述旧两阶段流程的措辞（如「重建 agent 并重跑同一 prompt」），若有则改为新语义。

- [ ] **Step 3: Commit**

```bash
rtk git add examples/human-in-the-loop/README.md
rtk git commit -m "docs(example): 更新 human-in-the-loop README 为循环对话 + y/n/a 授权模式"
```

---

### Task 3: 全仓一致性验证

**Files:**
- 无新增/修改文件。

**Interfaces:**
- Consumes: Task 1、Task 2 的产物。

- [ ] **Step 1: 全仓编译**

```bash
rtk cargo check --workspace
```
Expected: 通过（确认未误伤其他 crate）。

- [ ] **Step 2: 全仓 clippy**

```bash
rtk cargo clippy --workspace --all-targets 2>&1 | tail -20
```
Expected: 无 error；示例相关无新增 warning。

- [ ] **Step 3: 变更范围核对**

```bash
rtk git status --short
```
Expected: 仅 `examples/human-in-the-loop/` 下两个文件有变更（外加之前已提交的 spec 目录）。

- [ ] **Step 4: 记录收尾状态**

向记忆写入一条收尾记录（名称 `feature-031-hitl-repl-complete`）：Feature 031 实现完成，
human-in-the-loop 已从一次性流程改为循环对话 REPL + y/n/a 写入授权，未改 crates 源码。
