use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use agent_scope_agent::{
    Agent, AgentConfig, ContextConfig, MemoryMiddleware, Middleware, PermissionContext,
    PermissionRule, ReActAgent, ReActConfig,
};
use agent_scope_dashscope::{DashScopeChatModel, DashScopeEmbeddingModel};
use agent_scope_embedding::EmbeddingModelCard;
use agent_scope_memory::{FileMemory, Memory, MemoryConfig};
use agent_scope_message::factory::{assistant_msg, user_msg};
use agent_scope_rag::{KnowledgeBase, RAGMiddleware, RAGMode, TurbovecVectorStore};
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, Skill, WorkspaceBase};

use crate::config::{RunMode, RuntimeConfig};
use crate::error::{PiError, PiResult};
use crate::session::{SessionRecord, SessionStore};
use crate::tools::{ToolState, build_toolkit};

pub struct AgentRuntime {
    pub agent: ReActAgent,
    pub config: RuntimeConfig,
    pub session: SessionRecord,
    pub store: SessionStore,
    pub skills: Vec<Skill>,
    pub skill_instructions: String,
    /// Host-approved operation fingerprints, shared with the tool closures.
    /// The REPL inserts fingerprints here after the user approves a denied
    /// destructive operation, which makes the tool skip its confirmation gate
    /// on retry.
    pub approvals: Arc<Mutex<HashSet<String>>>,
}

impl AgentRuntime {
    pub async fn build(config: RuntimeConfig) -> PiResult<Self> {
        std::fs::create_dir_all(&config.workdir)
            .map_err(|err| PiError::io("create workdir", err))?;
        let store = SessionStore::new(config.workdir.join("sessions"));
        let session = if let Some(resume) = config.resume.as_deref() {
            if resume == "__latest__" {
                store
                    .load_latest()?
                    .unwrap_or_else(|| SessionRecord::new(&config))
            } else {
                store.load(resume)?
            }
        } else {
            SessionRecord::new(&config)
        };

        let approvals = Arc::new(Mutex::new(HashSet::new()));
        let mut probe_state = ToolState::from_config(&config);
        probe_state.approvals = Arc::clone(&approvals);
        let skills = load_workspace_skills(&config).await?;
        let skill_probe = build_toolkit(probe_state, skills.clone());
        let skill_instructions = skill_probe.get_skill_instructions(None);
        let agent = build_react_agent(
            &config,
            skills.clone(),
            &skill_instructions,
            Arc::clone(&approvals),
        )?;
        for turn in &session.turns {
            let user = user_msg("user", &turn.user_input)?;
            let assistant = assistant_msg("assistant", &turn.assistant_text);
            agent.observe(Some(vec![user, assistant])).await?;
        }

        Ok(Self {
            agent,
            config,
            session,
            store,
            skills,
            skill_instructions,
            approvals,
        })
    }
}

async fn load_workspace_skills(config: &RuntimeConfig) -> PiResult<Vec<Skill>> {
    let workspace_dir = config.workdir.join("workspace");
    std::fs::create_dir_all(&workspace_dir).map_err(|err| PiError::io("create workspace", err))?;
    let skill_paths = config
        .skill_paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    let mut workspace = LocalWorkspace::new(LocalWorkspaceConfig {
        workdir: workspace_dir.to_string_lossy().to_string(),
        workspace_id: Some("pi-rust-workspace".to_string()),
        default_mcps: Vec::new(),
        skill_paths,
        instructions: None,
    });
    workspace
        .initialize()
        .await
        .map_err(|err| PiError::internal(format!("workspace initialization failed: {err}")))?;
    workspace
        .list_skills()
        .await
        .map_err(|err| PiError::internal(format!("workspace skill loading failed: {err}")))
}

fn build_react_agent(
    config: &RuntimeConfig,
    skills: Vec<Skill>,
    skill_instructions: &str,
    approvals: Arc<Mutex<HashSet<String>>>,
) -> PiResult<ReActAgent> {
    let has_skills = !skills.is_empty();
    let model = Arc::new(DashScopeChatModel::new(&config.api_key, &config.model).with_stream(true));

    // One shared memory store is used by both the Memory tool (writes) and the
    // library's MemoryMiddleware (index injection + retrieval), so a fact the
    // model saves in one turn is immediately visible to the next turn and to
    // any later session started from the same workdir.
    let (memory, memory_config): (Option<Arc<dyn Memory>>, Option<MemoryConfig>) = if config
        .no_memory
    {
        (None, None)
    } else {
        let workdir = config.workdir.to_string_lossy().to_string();
        let memory_dir = config.workdir.join("Memory").to_string_lossy().to_string();
        let memory_config = MemoryConfig {
            memory_dir,
            ..MemoryConfig::default()
        };
        (
            Some(
                Arc::new(FileMemory::new(&workdir, memory_config.clone(), None)) as Arc<dyn Memory>,
            ),
            Some(memory_config),
        )
    };

    let mut builder = AgentConfig::builder()
        .name("pi_rust")
        .model(model.clone())
        .system_prompt(system_prompt(config, skill_instructions))
        // pi-rust persists conversations via its own SessionRecord store
        // (workdir/sessions); disable the library's default auto-persist so no
        // extra session files are written (Feature 025).
        .auto_persist(false);

    if !config.no_tools {
        let mut state = ToolState::from_config(config);
        state.approvals = approvals;
        state.memory = memory.clone();
        builder = builder
            .toolkit(build_toolkit(state, skills))
            .permission_context(permission_context(has_skills));
    }

    let mut middlewares: Vec<Arc<dyn Middleware>> = Vec::new();
    if let (Some(memory), Some(config)) = (&memory, &memory_config) {
        middlewares.push(Arc::new(MemoryMiddleware::new(
            Arc::clone(memory),
            config.clone(),
        )));
    }
    if !config.no_rag {
        let embedding = Arc::new(DashScopeEmbeddingModel::new(
            config.api_key.clone(),
            EmbeddingModelCard::new("text-embedding-v3", 1024, false),
        ));
        let vector_store = Arc::new(TurbovecVectorStore::new(4)?);
        let kb = Arc::new(KnowledgeBase::new(
            "project".to_string(),
            "Project documents indexed for pi-rust retrieval".to_string(),
            embedding,
            vector_store,
            "project".to_string(),
            None,
        ));
        middlewares.push(Arc::new(RAGMiddleware::new(
            vec![kb],
            RAGMode::Static,
            5,
            None,
        )));
    }

    let agent_config = builder.build()?;
    let react_config = ReActConfig {
        max_iters: config.max_iters,
        ..Default::default()
    };
    let context_config = ContextConfig {
        enable: true,
        ..Default::default()
    };
    Ok(ReActAgent::new(
        agent_config,
        react_config,
        context_config,
        middlewares,
    )?)
}

fn permission_context(has_skills: bool) -> PermissionContext {
    let mut context = PermissionContext::default();
    context.add_rule(PermissionRule::allow("Read"));
    context.add_rule(PermissionRule::allow("Write"));
    context.add_rule(PermissionRule::allow("Edit"));
    context.add_rule(PermissionRule::allow("Bash"));
    context.add_rule(PermissionRule::allow("Grep"));
    context.add_rule(PermissionRule::allow("Glob"));
    context.add_rule(PermissionRule::allow("ListDir"));
    context.add_rule(PermissionRule::allow("Memory"));
    if has_skills {
        context.add_rule(PermissionRule::allow("Skill"));
    }
    context
}

fn system_prompt(config: &RuntimeConfig, skill_instructions: &str) -> String {
    let mode_guidance = match config.mode {
        RunMode::React => "",
        RunMode::Coding => {
            r#"
Coding workflow:
1. Understand: inspect relevant files before editing.
2. Plan: identify the minimal change before modifying files.
3. Change: prefer Edit for existing files and Write for new files. For very large
   content, write a placeholder file first, then append with Edit in chunks.
4. Verify: run the narrowest relevant check/test with Bash.
5. Iterate: if verification fails, inspect the error, make a targeted fix, and rerun verification within the iteration budget.
6. Report: final answer must include Summary, Files changed, Verification, and Remaining risks.
"#
        }
    };
    let skills_guidance = if skill_instructions.trim().is_empty() {
        "No workspace skills are loaded; do not claim skills are available.\n".to_string()
    } else {
        format!(
            "Use Skill only for skills listed in <agent-skills>. When a user task matches a listed skill, call Skill first to read the full instructions.\n\n{skill_instructions}\n"
        )
    };
    let task_tools_guidance = r#"
Task tools (TaskCreate / TaskList / TaskGet / TaskUpdate) are available.
Use them only for genuinely multi-step work (3+ steps). The correct lifecycle is:
  TaskCreate -> do the actual work with Read/Write/Edit/Bash -> TaskUpdate(status=completed).
NEVER mark a task completed before its work has actually succeeded.
If only one step remains, just do it directly without creating a task.
"#;
    let failure_recovery_guidance = r#"
If a tool reports an argument/JSON error or "was NOT executed", the tool call was
NOT performed. Re-issue it with a corrected, complete JSON argument instead of
stopping or pretending it succeeded. Repeated failures on large arguments mean
the argument is too big: prefer incremental writes.
"#;
    format!(
        r#"You are pi-rust, a coding Agent implemented in Rust on agentscope-rust.

Capabilities:
- Use Read before explaining or editing files.
- Use Grep/Glob/ListDir to locate and explore files before editing.
- Use Write to create files and Edit for exact replacements.
- Use Bash to execute command-line tasks directly: run the commands the user asks for (curl/git/build/test/verify — anything that can be done in the shell). Prefer executing the command yourself over handing the user a script or manual steps.
- Destructive shell commands and risky overwrites are gated by the host: the REPL asks the user y/n, and approved operations are retried automatically. Just call the tool normally; approval is the host's job. Do not claim an operation was approved unless the tool actually succeeded.
- When the user asks you to remember something (their name, a preference, a fact), call the Memory tool to save it. Never claim you remembered something you did not persist.
- Keep answers concise, structured, and actionable.
- Never reveal API keys or secrets.
{mode_guidance}
{skills_guidance}
{task_tools_guidance}
{failure_recovery_guidance}
Current provider: {provider}
Current model: {model}
Run mode: {mode}
Project working directory: {cwd}
Tools enabled: {tools}
Memory enabled: {memory}
RAG enabled: {rag}
"#,
        provider = config.provider.name(),
        model = config.model,
        mode = config.mode.as_str(),
        cwd = config.cwd.display(),
        tools = !config.no_tools,
        memory = !config.no_memory,
        rag = !config.no_rag,
    )
}

#[allow(dead_code)]
pub fn unsupported_provider(provider: &str) -> PiError {
    PiError::unsupported(format!(
        "provider '{provider}' is not implemented; pi-rust currently supports DashScope"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ProviderConfig, RunMode, RuntimeConfig};
    use std::path::PathBuf;

    fn config(mode: RunMode) -> RuntimeConfig {
        RuntimeConfig {
            api_key: "sk-test".into(),
            masked_api_key: "****".into(),
            model: "qwen-plus".into(),
            provider: ProviderConfig::DashScope,
            workdir: PathBuf::from(".pi-rust"),
            cwd: PathBuf::from("."),
            mode,
            skill_paths: vec![],
            prompt: None,
            resume: None,
            list_sessions: false,
            no_tools: false,
            no_memory: false,
            no_rag: false,
            max_iters: 20,
            command_timeout_secs: 30,
            show_events: false,
            show_json_events: false,
            no_tui: false,
        }
    }

    #[test]
    fn coding_system_prompt_includes_task_tool_workflow() {
        let prompt = system_prompt(&config(RunMode::Coding), "");
        for needle in [
            "TaskCreate",
            "TaskUpdate(status=completed)",
            "NEVER mark a task completed",
            "was NOT executed",
            "incremental writes",
            "placeholder file",
        ] {
            assert!(prompt.contains(needle), "missing {needle:?} in prompt");
        }
    }

    #[test]
    fn react_system_prompt_includes_recovery_guidance() {
        let prompt = system_prompt(&config(RunMode::React), "");
        assert!(
            prompt.contains("TaskCreate"),
            "task tools missing in react prompt"
        );
        assert!(
            prompt.contains("was NOT executed"),
            "recovery missing in react prompt"
        );
    }

    #[test]
    fn coding_mode_mentions_chunked_writes() {
        let prompt = system_prompt(&config(RunMode::Coding), "");
        assert!(prompt.contains("append with Edit in chunks"));
    }
}
