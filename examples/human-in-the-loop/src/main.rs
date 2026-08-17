//! Human-in-the-loop example: a loop-mode conversational agent whose `write_note`
//! tool is guarded by a permission `ask` rule. The host (this program) drives
//! approval / rejection interactively, y/n/a.
//!
//! Flow (event-driven pause → confirm → resume, aligned with Python):
//!   1. REPL 循环：用户在 stdin 输入消息，agent 流式回复。
//!   2. agent 调用 `write_note` 时，`ask` 规则使引擎发出
//!      `RequireUserConfirmEvent` 并**暂停**（流结束，无 ReplyEnd）。
//!   3. 宿主询问 y/n/a 后以 `UserConfirmResultEvent` **恢复同一 agent**：
//!   - y = 仅本次批准（confirmed=true，无 rules）→ 工具执行
//!   - n = 拒绝（confirmed=false）→ 生成 DENIED 结果，模型调整
//!   - a = 总是允许（confirmed=true + rules:[allow]）→ 规则采纳进引擎，此后不再询问
//!
//! Requires `DEFAULT_API_KEY` for real model calls.

use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;

use agent_scope_agent::event_input::EventInput;
use agent_scope_agent::{
    Agent, AgentConfig, ContextConfig, PermissionContext, PermissionMode, PermissionRule,
    ReActAgent, ReActConfig,
};
use agent_scope_event::{AgentEvent, ConfirmResult, EventBase, UserConfirmResultEvent};
use agent_scope_message::Msg;
use agent_scope_message::PermissionRule as MsgPermissionRule;
use agent_scope_message::factory::user_msg;
use agent_scope_rig::RigChatModel;
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

/// A message-level `allow` rule for `tool`, carried in `ConfirmResult.rules`.
///
/// On resume the engine decodes this back into a `PermissionRule::allow` and
/// adopts it (clearing the matching ask rule), so later calls of the tool no
/// longer ask.
fn allow_rule(tool: &str) -> MsgPermissionRule {
    let mut extras = HashMap::new();
    extras.insert("tool_name".to_string(), serde_json::json!(tool));
    extras.insert("behavior".to_string(), serde_json::json!("allow"));
    extras.insert("source".to_string(), serde_json::json!("runtime"));
    MsgPermissionRule { extras }
}

fn build_agent(model: Arc<RigChatModel>) -> anyhow::Result<ReActAgent> {
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
        .permission_context(ask_context())
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
    /// 总是允许（a），规则采纳进引擎，此后不再询问。
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

/// Run one user turn against the SAME agent instance, driving the
/// pause → confirm → resume loop until the reply ends.
///
/// When the agent pauses awaiting confirmation (`RequireUserConfirmEvent`),
/// the host collects y/n/a for each pending tool call, injects a
/// `UserConfirmResultEvent` via `reply_stream_event`, and continues consuming
/// the resumed stream. The agent is never rebuilt and no history is truncated.
async fn run_turn(
    agent: &ReActAgent,
    always_allow: &mut bool,
    user_input: Msg,
) -> anyhow::Result<()> {
    let mut stream = agent.reply_stream(Some(vec![user_input])).await?;

    loop {
        // 消费当前流，收集事件。`RequireUserConfirm` 后流已暂停（自然结束）；
        // `ReplyEnd` 表示本轮回复完成。
        let mut confirm: Option<agent_scope_event::RequireUserConfirmEvent> = None;
        while let Some(event) = stream.next().await {
            match &event {
                AgentEvent::ThinkingBlockDelta(d) => print!("\x1b[2m{}\x1b[0m", d.delta),
                AgentEvent::TextBlockDelta(d) => print!("{}", d.delta),
                AgentEvent::ToolCallStart(s) => println!("\n[tool start] {}", s.tool_call_name),
                AgentEvent::ToolResultTextDelta(d) => println!("\n[tool result] {}", d.delta),
                AgentEvent::RequireUserConfirm(c) => {
                    confirm = Some(c.clone());
                    println!("\n[human-in-the-loop] 暂停等待确认…");
                }
                AgentEvent::ReplyEnd(e) => println!("\n[reply end] {:?}", e.finished_reason),
                _ => {}
            }
            if confirm.is_some() {
                break;
            }
        }
        drop(stream);

        let Some(confirm) = confirm else {
            // 无暂停：本轮回复结束（ReplyEnd 或正常流终止）。
            break;
        };

        // 逐个询问每个待确认工具，收集确认结果。
        let mut results = Vec::new();
        for tc in &confirm.tool_calls {
            match ask_user(&tc.name, &tc.input)? {
                Approval::Approved => {
                    println!("[human-in-the-loop] 本次批准 {}，继续执行…", tc.name);
                    results.push(ConfirmResult {
                        confirmed: true,
                        tool_call: tc.clone(),
                        rules: None,
                    });
                }
                Approval::AlwaysAllow => {
                    println!("[human-in-the-loop] 总是允许 {}，规则采纳…", tc.name);
                    *always_allow = true;
                    results.push(ConfirmResult {
                        confirmed: true,
                        tool_call: tc.clone(),
                        rules: Some(vec![allow_rule(&tc.name)]),
                    });
                }
                Approval::Rejected => {
                    println!("[human-in-the-loop] 已拒绝 {}，模型自行调整…", tc.name);
                    results.push(ConfirmResult {
                        confirmed: false,
                        tool_call: tc.clone(),
                        rules: None,
                    });
                }
            }
        }

        // 以事件恢复同一 agent，继续消费其流。
        let resume = UserConfirmResultEvent {
            base: EventBase::new(),
            reply_id: confirm.reply_id.clone(),
            confirm_results: results,
        };
        stream = agent
            .reply_stream_event(EventInput::Confirm(resume))
            .await?;
    }

    Ok(())
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

    let api_key = std::env::var("DEFAULT_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("error: 缺少环境变量 DEFAULT_API_KEY。请设置后重试。"))?;
    // 模型名从 DEFAULT_CHAT_MODEL 读取（fallback qwen3.7-plus）；DEFAULT_URL 可选覆盖端点。
    let model_name =
        std::env::var("DEFAULT_CHAT_MODEL").unwrap_or_else(|_| "qwen3.7-plus".to_string());
    let mut model = RigChatModel::openai(&api_key, &model_name)?;
    if let Ok(base_url) = std::env::var("DEFAULT_URL") {
        model = model.with_base_url(base_url);
    }
    let model = Arc::new(model.with_stream(true));

    // 同一 agent 实例贯穿整个会话：暂停-确认-恢复不重建、不截断历史。
    let agent = build_agent(model)?;
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
        run_turn(&agent, &mut always_allow, msg).await?;
    }

    let _ = always_allow;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `a`（总是允许）构造的规则必须可被引擎解码为 allow 规则。
    #[test]
    fn always_allow_rule_carries_allow_behavior() {
        let rule = allow_rule("write_note");
        let decoded: PermissionRule = serde_json::from_value(serde_json::to_value(&rule).unwrap())
            .expect("规则应可反序列化为引擎 PermissionRule");
        assert_eq!(decoded.tool_name, "write_note");
        assert_eq!(
            decoded.behavior,
            agent_scope_agent::permission::PermissionBehavior::Allow
        );
    }

    /// 未批准时保留 ask 上下文（首次尝试需确认）。
    #[test]
    fn default_uses_ask_context() {
        let ctx = ask_context();
        assert!(
            ctx.ask_rules.contains_key("write_note"),
            "未批准时应使用 ask 上下文要求确认"
        );
        assert!(!ctx.allow_rules.contains_key("write_note"));
    }
}
