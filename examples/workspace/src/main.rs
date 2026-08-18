//! Workspace example: bind a `LocalWorkspace` to a `ReActAgent` so built-in
//! workspace tools are injected and used by the normal agent/tool pipeline.
//!
//! A workspace scopes filesystem and command operations to a controlled working
//! directory. When it is attached to an agent, the agent automatically receives
//! built-in tools such as Bash, Read, Write, Edit, Grep, Glob, ResetTools, and
//! Skill. This demo installs a read-only permission context so only
//! Read/Glob/Grep can run without confirmation. It creates a temporary project
//! file and asks the agent to read it through the injected `Read` tool.

use std::sync::Arc;

use agent_scope_agent::{
    Agent, AgentConfig, ContextConfig, PermissionContext, PermissionMode, PermissionRule,
    ReActAgent, ReActConfig,
};
use agent_scope_event::AgentEvent;
use agent_scope_message::factory::user_msg;
use agent_scope_rig::RigChatModel;
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};
use clap::Parser;
use futures::StreamExt;

#[derive(Parser)]
struct Cli {
    /// User prompt to send to the workspace-enabled agent.
    #[arg(short, long)]
    prompt: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    dotenv::dotenv().ok();

    let api_key = std::env::var("DEFAULT_API_KEY")
        .map_err(|_| anyhow::anyhow!("error: 缺少环境变量 DEFAULT_API_KEY。请设置后重试。"))?;
    let model_name =
        std::env::var("DEFAULT_CHAT_MODEL").unwrap_or_else(|_| "qwen3.7-plus".to_string());
    let mut model = RigChatModel::openai(&api_key, &model_name)?;
    if let Ok(base_url) = std::env::var("DEFAULT_URL") {
        model = model.with_base_url(base_url);
    }
    let model = Arc::new(model.with_stream(true));

    let dir = tempfile::tempdir()?;
    let workdir = dir.path().to_str().unwrap().to_string();
    let note_path = dir.path().join("project-note.txt");
    tokio::fs::write(
        &note_path,
        "workspace note: ReActAgent can read this file through the injected Read tool.\n",
    )
    .await?;

    // 1. Create and initialize the workspace.
    let mut ws = LocalWorkspace::new(LocalWorkspaceConfig {
        workdir: workdir.clone(),
        workspace_id: Some("demo-ws".into()),
        default_mcps: vec![],
        skill_paths: vec![],
        instructions: Some("Project documents live in the workspace root.".into()),
    });
    ws.initialize().await?;
    println!("workspace initialized at {workdir}");
    println!("instructions: {}", ws.get_instructions().await);

    let tools = ws.list_tools().await?;
    println!(
        "workspace exposes backend tools: {}",
        tools
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    // 2. Bind the workspace to an agent. ReActAgent construction injects the
    // workspace built-ins into the agent's ToolKit. Explore mode denies every
    // unclassified tool call, so arbitrary prompts cannot run Bash/Write/Edit;
    // this demo only allows read-only workspace tools.
    let ws: Arc<dyn WorkspaceBase> = Arc::new(ws);
    let mut perm = PermissionContext::new(PermissionMode::Explore);
    perm.add_rule(PermissionRule::allow("Read"));
    perm.add_rule(PermissionRule::allow("Glob"));
    perm.add_rule(PermissionRule::allow("Grep"));

    let agent_config = AgentConfig::builder()
        .name("workspace-demo")
        .system_prompt(
            "你是一个 workspace 工具演示助手。需要读取工作区文件时必须调用 Read 工具；\
             回答时请说明你读取到了什么。",
        )
        .model(model)
        .workspace(Arc::clone(&ws))
        .permission_context(perm)
        .auto_persist(false)
        .build()?;

    let agent = ReActAgent::new(
        agent_config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![],
    )?;

    let toolkit = agent.toolkit().expect("workspace toolkit present");
    let schemas = toolkit.get_tool_schemas();
    let injected_names: Vec<String> = schemas
        .iter()
        .filter_map(|s| s["function"]["name"].as_str().map(str::to_string))
        .collect();
    println!("injected agent tools: {}", injected_names.join(", "));

    // 3. Ask the agent to use the injected Read tool rather than calling the
    // workspace backend directly.
    let default_prompt = format!(
        "请调用 Read 工具读取这个文件并总结内容：{}",
        note_path.display()
    );
    let prompt = cli.prompt.as_deref().unwrap_or(&default_prompt);
    let msg = user_msg("user", prompt).map_err(|e| anyhow::anyhow!("{e:?}"))?;

    println!("\n--- agent-driven workspace tool call ---");
    let mut stream = agent.reply_stream(Some(vec![msg])).await?;
    while let Some(event) = stream.next().await {
        match &event {
            AgentEvent::TextBlockDelta(d) => print!("{}", d.delta),
            AgentEvent::ToolCallStart(s) => {
                println!("\n[tool start] {} ({})", s.tool_call_name, s.tool_call_id)
            }
            AgentEvent::ToolCallDelta(d) => print!("{}", d.delta),
            AgentEvent::ToolCallEnd(e) => println!(" [tool end] {}", e.tool_call_id),
            AgentEvent::ToolResultStart(r) => {
                println!(
                    "[tool result start] {} ({})",
                    r.tool_call_name, r.tool_call_id
                )
            }
            AgentEvent::ToolResultTextDelta(d) => print!("{}", d.delta),
            AgentEvent::ToolResultEnd(e) => println!("\n[tool result end] {}", e.tool_call_id),
            AgentEvent::RequireUserConfirm(c) => {
                let names: Vec<String> = c.tool_calls.iter().map(|b| b.name.clone()).collect();
                println!("\n[needs confirmation] tools: {names:?}");
            }
            AgentEvent::ReplyEnd(e) => {
                println!("\n[reply end] finished_reason={:?}", e.finished_reason)
            }
            _ => {}
        }
    }

    // The temporary workspace directory is removed automatically when `dir` is dropped.
    Ok(())
}
