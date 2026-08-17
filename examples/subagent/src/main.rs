//! SubAgent 示例 — 工具驱动：主 Agent 通过内置工具自主创建并委托子智能体。
//!
//! 本示例把 SubAgent 封装成两个内置工具，注册进主 Agent 的 `ToolKit`：
//! - `SubAgentCreate`：主 Agent 调用它**创建并注册**一个真实 ReActAgent 子智能体；
//! - `SubAgentDelegate`：主 Agent 调用它把任务**委托**给已创建的子智能体并收取结果。
//!
//! 主 Agent 的 ReAct 循环会自主决定何时创建哪些子智能体、如何拆解与派发任务，
//! 最后汇总各子智能体的产出向用户汇报。子智能体本身也是真实 OpenAI 模型调用，
//! 工具与主 Agent 通过共享的 `SubAgentRegistry` 协作。
//!
//! 凭据从项目根目录 `.env` 读取（`DEFAULT_API_KEY`），也支持环境变量。
//!
//! 运行：
//! ```bash
//! cargo run -p subagent                            # 默认模型 qwen3.7-plus（可用 .env 的 DEFAULT_CHAT_MODEL 覆盖）
//! cargo run -p subagent -- --model <model>         # 指定模型
//! cargo run -p subagent -- --task "你的自定义任务"  # 自定义任务
//! ```

use std::io::Write;
use std::sync::Arc;

use agent_scope_agent::{
    Agent, AgentConfig, CollaborationResult, CollaborationStatus, ContextConfig, DelegationRequest,
    ReActAgent, ReActConfig, SubAgent, SubAgentRegistry, delegate_once,
};
use agent_scope_event::AgentEvent;
use agent_scope_message::{ContentBlock, Msg, Role, TextBlock};
use agent_scope_rig::RigChatModel;
use agent_scope_tool::{FunctionTool, ToolKit};
use clap::Parser;
use futures::StreamExt;
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::RwLock as AsyncRwLock;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
struct Cli {
    /// 使用的模型名（也可用 .env 的 DEFAULT_CHAT_MODEL 覆盖）
    #[arg(long, default_value = "qwen3.7-plus")]
    model: String,
    /// 委派给主 Agent 的任务（默认使用内置示例任务）
    #[arg(long)]
    task: Option<String>,
}

// ---------------------------------------------------------------------------
// 工具输入参数（schemars 自动从结构体推导 JSON Schema）
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct SubAgentCreateParams {
    /// 子智能体唯一名称（作为 SubAgentDelegate 的 target）
    name: String,
    /// 一句话职责描述，写入注册表
    description: String,
    /// 角色指令，会成为该子智能体的系统提示词
    instructions: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SubAgentDelegateParams {
    /// 目标子智能体名称（必须先通过 SubAgentCreate 创建）
    target: String,
    /// 要委托的具体任务
    task: String,
}

// ---------------------------------------------------------------------------
// 配置与构建辅助
// ---------------------------------------------------------------------------

/// 组装 `AgentConfig`：`toolkit` 为 `Some` 时注册进该智能体；子智能体无需工具。
fn agent_config(
    name: &str,
    description: &str,
    instructions: &str,
    api_key: &str,
    model_name: &str,
    toolkit: Option<ToolKit>,
) -> Result<AgentConfig, Box<dyn std::error::Error>> {
    let mut model = RigChatModel::openai(api_key, model_name)?;
    if let Ok(base_url) = std::env::var("DEFAULT_URL") {
        model = model.with_base_url(base_url);
    }
    let model = Arc::new(model.with_stream(false));
    let mut builder = AgentConfig::builder()
        .name(name)
        .system_prompt(format!(
            "你是「{name}」，{description}。{instructions}\n请直接回答，简明扼要，不要编造内容。"
        ))
        .model(model);
    if let Some(toolkit) = toolkit {
        builder = builder.toolkit(toolkit);
    }
    Ok(builder.build()?)
}

/// 构建一个真实 ReActAgent 子智能体。
fn build_subagent(
    name: &str,
    description: &str,
    instructions: &str,
    api_key: &str,
    model_name: &str,
) -> Result<ReActAgent, Box<dyn std::error::Error>> {
    Ok(ReActAgent::new(
        agent_config(name, description, instructions, api_key, model_name, None)?,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )?)
}

/// 把一次委托结果转成给主 Agent 看的文本。
fn collab_text(result: &CollaborationResult) -> String {
    match result.status {
        CollaborationStatus::Succeeded => {
            let text = result
                .message
                .as_ref()
                .and_then(|msg| msg.get_text_content(" "))
                .unwrap_or_default();
            format!("子智能体 '{}' 已完成: {}", result.subagent_name, text)
        }
        _ => {
            let detail = result
                .error
                .as_ref()
                .map(|err| format!("[{}] {}", err.code, err.message))
                .unwrap_or_else(|| "无错误详情".to_string());
            format!(
                "子智能体 '{}' 未完成: {:?}（{detail}）",
                result.subagent_name, result.status
            )
        }
    }
}

fn make_msg(name: &str, text: &str, role: Role) -> Msg {
    Msg::new(
        name.to_string(),
        vec![ContentBlock::Text(TextBlock::new(text.to_string()))],
        role,
    )
    .expect("example message is valid")
}

// ---------------------------------------------------------------------------
// 把 SubAgent 封装成工具
// ---------------------------------------------------------------------------

/// 把 SubAgent 封装成两个工具：`SubAgentCreate`（创建+注册）与
/// `SubAgentDelegate`（委托+收结果）。两者通过共享的 `SubAgentRegistry` 协作，
/// 主 Agent 的 ReAct 循环负责决定调用时机。
fn make_subagent_tools(
    registry: Arc<AsyncRwLock<SubAgentRegistry>>,
    api_key: String,
    model_name: String,
) -> (FunctionTool, FunctionTool) {
    // SubAgentCreate：构建并注册一个真实 ReActAgent 子智能体。
    let create_registry = Arc::clone(&registry);
    let create_api_key = api_key.clone();
    let create_model = model_name.clone();
    let create = FunctionTool::new(
        "SubAgentCreate",
        "创建并注册一个子智能体（SubAgent）。子智能体是共享同一模型配置的真实 Agent，能独立完成你委托的任务。\n\n## 何时使用\n- 当你需要把任务拆解给专人完成（如调研、编码、复核）时，先用本工具创建对应的子智能体。\n- 每个子智能体有全局唯一名称，之后用 SubAgentDelegate(target=<name>) 委托任务。\n\n## 参数\n- name: 子智能体唯一名称，例如 researcher / coder / reviewer\n- description: 一句话职责描述\n- instructions: 角色指令，会成为该子智能体的系统提示词\n\n## 注意\n先创建再委托；同一名称只需创建一次。",
        move |params: SubAgentCreateParams| {
            let registry = Arc::clone(&create_registry);
            let api_key = create_api_key.clone();
            let model_name = create_model.clone();
            async move {
                // 幂等：已注册则直接提示，避免重复构建。
                {
                    let reg = registry.read().await;
                    if reg.get(&params.name).is_ok() {
                        return format!("子智能体 '{}' 已存在，无需重复创建。", params.name);
                    }
                }
                let agent = match build_subagent(
                    &params.name,
                    &params.description,
                    &params.instructions,
                    &api_key,
                    &model_name,
                ) {
                    Ok(agent) => agent,
                    Err(e) => return format!("构建子智能体 '{}' 失败：{e}", params.name),
                };
                let subagent = match SubAgent::new(
                    params.name.clone(),
                    params.description.clone(),
                    Arc::new(agent),
                ) {
                    Ok(sa) => sa,
                    Err(e) => return format!("校验子智能体 '{}' 失败：{e}", params.name),
                };
                match registry.write().await.register_subagent(subagent) {
                    Ok(_) => format!(
                        "已创建并注册子智能体 '{}'：{}。现在可以用 SubAgentDelegate 把任务委托给它。",
                        params.name, params.description
                    ),
                    Err(e) => format!("注册子智能体 '{}' 失败：{e}", params.name),
                }
            }
        },
    );

    // SubAgentDelegate：把任务委托给一个已创建的子智能体并返回结果。
    let delegate_registry = Arc::clone(&registry);
    let delegate = FunctionTool::new(
        "SubAgentDelegate",
        "把任务委托给一个已创建的子智能体，并返回它的完成结果。\n\n## 何时使用\n- 目标子智能体已通过 SubAgentCreate 创建后，用本工具把对应子任务交给它。\n- 若返回「未找到」类错误，请先用 SubAgentCreate 创建该子智能体再重试。\n\n## 参数\n- target: 目标子智能体名称（必须与 SubAgentCreate 的 name 一致）\n- task: 要委托的具体任务描述",
        move |params: SubAgentDelegateParams| {
            let registry = Arc::clone(&delegate_registry);
            async move {
                let reg = registry.read().await;
                let result = delegate_once(
                    &reg,
                    DelegationRequest::new("assistant", &params.target, &params.task),
                )
                .await;
                match result {
                    Ok(result) => collab_text(&result),
                    Err(e) => format!("委托失败：{e}"),
                }
            }
        },
    );

    (create, delegate)
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

const DEFAULT_TASK: &str = "\
请组织一组子智能体完成以下三件事：
1) 调研 Rust 所有权机制的核心要点并给出结论；
2) 用 Rust 写一个体现所有权转移的示例函数；
3) 复核前两项产出是否自洽。
完成后向用户做总结。";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    dotenv::dotenv().ok();
    let api_key = std::env::var("DEFAULT_API_KEY").map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("缺少环境变量 DEFAULT_API_KEY。请确认项目根目录 .env 中已配置（{e}）。"),
        )
    })?;

    // 模型名：优先 .env 的 DEFAULT_CHAT_MODEL，fallback CLI --model（默认 qwen3.7-plus）。
    let model_name = std::env::var("DEFAULT_CHAT_MODEL").unwrap_or_else(|_| cli.model.clone());

    println!(
        "=== SubAgent 工具驱动演示（真实 OpenAI 模型: {}）===",
        model_name
    );

    // 1. 共享注册表：两个工具与主 Agent 共用一个 registry。
    let registry = Arc::new(AsyncRwLock::new(SubAgentRegistry::new("assistant")));

    // 2. 把 SubAgent 封装成两个内置工具，注册进主 Agent 的 ToolKit。
    let (create_tool, delegate_tool) =
        make_subagent_tools(registry.clone(), api_key.clone(), model_name.clone());
    let mut toolkit = ToolKit::new();
    toolkit.register(create_tool);
    toolkit.register(delegate_tool);
    println!("  [toolkit] 已注册工具: SubAgentCreate + SubAgentDelegate");

    // 3. 主 Agent：持有工具，在 ReAct 循环里自主创建并委托子智能体。
    let main_agent = ReActAgent::new(
        agent_config(
            "assistant",
            "团队负责人，负责拆解任务、创建并委托子智能体，最后汇总结果",
            "你会通过 SubAgentCreate / SubAgentDelegate 工具指挥一组真实子智能体完成任务。\
             先创建再委托，不要调用未创建的子智能体。",
            &api_key,
            &model_name,
            Some(toolkit),
        )?,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )?;

    // 4. 输入任务并流式运行主循环（主 Agent 自主完成创建→委托→汇总）。
    //    用 `reply_stream` 消费事件，按事件类型打印核心事件。
    let task = cli.task.unwrap_or_else(|| DEFAULT_TASK.to_string());
    println!("\n--- 任务 ---\n{task}\n");
    println!("--- 主 Agent 流式事件（核心事件按类型打印）---");
    let mut stream = main_agent
        .reply_stream(Some(vec![make_msg("user", &task, Role::User)]))
        .await?;
    while let Some(event) = stream.next().await {
        match &event {
            AgentEvent::ReplyStart(e) => println!("[reply start] id={}", e.reply_id),
            AgentEvent::ModelCallStart(m) => println!("\n[model call] model={}", m.model_name),
            // 思考增量用暗色显示，与最终文本区分。
            AgentEvent::ThinkingBlockDelta(d) => print!("\x1b[2m{}\x1b[0m", d.delta),
            // 主 Agent 的最终汇总文本。
            AgentEvent::TextBlockDelta(d) => print!("{}", d.delta),
            AgentEvent::ToolCallStart(s) => {
                // 主 Agent 自主决定调用 SubAgentCreate / SubAgentDelegate。
                println!("\n[tool call] {} ({})", s.tool_call_name, s.tool_call_id)
            }
            AgentEvent::ToolCallDelta(d) => print!("{}", d.delta),
            AgentEvent::ToolCallEnd(e) => println!(" [tool end] {}", e.tool_call_id),
            AgentEvent::ToolResultStart(r) => {
                println!("\n[tool result] {} ({})", r.tool_call_name, r.tool_call_id)
            }
            // 子智能体的完成结果以工具结果增量流回。
            AgentEvent::ToolResultTextDelta(d) => print!("{}", d.delta),
            AgentEvent::ToolResultEnd(e) => println!("\n[tool result end] {}", e.tool_call_id),
            AgentEvent::ExceedMaxIters(_) => println!("\n[exceeded max iterations]"),
            AgentEvent::ReplyEnd(e) => {
                println!("\n[reply end] finished_reason={:?}", e.finished_reason)
            }
            _ => {}
        }
        std::io::stdout().flush().ok();
    }

    // 5. 展示主 Agent 实际创建了哪些子智能体。
    println!("\n--- 已创建的子智能体 ---");
    let reg = registry.read().await;
    for sa in reg.list() {
        println!("  {} — {}", sa.name, sa.description);
    }
    if reg.list().is_empty() {
        println!("  （主 Agent 未创建任何子智能体）");
    }

    Ok(())
}
