//! Human-in-the-loop example: a loop-mode conversational agent whose `write_note`
//! tool is guarded by a permission `ask` rule. The host (this program) drives
//! approval / rejection interactively, y/n/a.
//!
//! Flow:
//!   1. REPL 循环：用户在 stdin 输入消息，agent 流式回复。
//!   2. agent 平时可自由回答；调用 `write_note` 时，`ask` 规则使引擎发出
//!      `RequireUserConfirmEvent`，并把被拒结果喂回模型。
//!   3. 宿主询问 y/n/a：
//!   - y = 本次批准：截断历史回退到本轮用户消息，重建 allow agent 重放 → 写入成功
//!   - n = 拒绝：不重放，模型已收到被拒结果，自行调整
//!   - a = 总是允许：宿主置 always_allow，此后重建 allow agent，不再询问
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
use agent_scope_message::Msg;
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

/// Pick the permission context for the current attempt.
///
/// `allow` is true after the host approves (`y`/`a`) so the replay executes the
/// write instead of re-triggering confirmation. Returns an `ask` context by
/// default and an `allow` context once approved.
fn permission_context_for(allow: bool) -> PermissionContext {
    if allow {
        allow_context()
    } else {
        ask_context()
    }
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
    print!("\n🔐 {tool_name} 需要授权：{input}\n   批准该调用？[y/n/a] (a=总是允许) ");
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
    history: &mut Vec<Msg>,
    always_allow: &mut bool,
    start_len: usize,
) -> anyhow::Result<()> {
    // 本次 turn 的放行状态：初始跟随全局 always_allow；批准（y/a）后置为
    // 放行，用于本次重放。`y` 只影响本次 turn（`always_allow` 不变，下一轮
    // 恢复询问），`a` 同时持久化到 always_allow。
    let mut current_allow = *always_allow;
    loop {
        // Rebuild per replay: permission context is fixed at construction time.
        let perm = permission_context_for(current_allow);
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
                                println!("[human-in-the-loop] 本次批准 {}，重放本轮…", tc.name);
                                // 仅本次放行：重放用 allow 上下文，写入成功。
                                current_allow = true;
                            }
                            Approval::AlwaysAllow => {
                                println!("[human-in-the-loop] 总是允许 {}，重放本轮…", tc.name);
                                *always_allow = true;
                                current_allow = true;
                            }
                            Approval::Rejected => {
                                println!("[human-in-the-loop] 已拒绝 {}，模型自行调整…", tc.name);
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
            continue; // 重新构建（本次放行时用 allow 上下文）重放本轮
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
        .ok()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("error: 缺少环境变量 DASHSCOPE_API_KEY。请设置后重试。"))?;
    let model = Arc::new(DashScopeChatModel::new(&api_key, "qwen-plus").with_stream(true));

    let mut history: Vec<Msg> = Vec::new();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 批准（y/a）后的重放必须用 allow 上下文（含 allow 规则、不含 ask 规则），
    /// 否则重放会再次触发确认，导致无限循环。
    #[test]
    fn approved_replay_uses_allow_context() {
        let ctx = permission_context_for(true);
        assert!(
            ctx.allow_rules.contains_key("write_note"),
            "批准后应使用 allow 上下文放行 write_note"
        );
        assert!(
            !ctx.ask_rules.contains_key("write_note"),
            "批准后不应残留 ask 规则，否则重放再次触发确认"
        );
    }

    /// 未批准时保留 ask 上下文（首次尝试需确认）。
    #[test]
    fn default_uses_ask_context() {
        let ctx = permission_context_for(false);
        assert!(
            ctx.ask_rules.contains_key("write_note"),
            "未批准时应使用 ask 上下文要求确认"
        );
        assert!(!ctx.allow_rules.contains_key("write_note"));
    }
}
