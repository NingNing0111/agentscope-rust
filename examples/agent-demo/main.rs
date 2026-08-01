use std::collections::HashSet;
use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_scope_agent::{
    Agent, AgentConfig, AgentError, ContextConfig, MemoryMiddleware, Middleware, PermissionContext,
    PermissionRule, ReActAgent, ReActConfig,
};
use agent_scope_dashscope::{DashScopeChatModel, DashScopeEmbeddingModel};
use agent_scope_embedding::EmbeddingModelCard;
use agent_scope_event::AgentEvent;
use agent_scope_memory::{FileMemory, Memory, MemoryConfig};
use agent_scope_message::factory::{assistant_msg, user_msg};
use agent_scope_rag::chunker::{ApproxTokenChunker, Chunk, Chunker};
use agent_scope_rag::knowledge_base::KnowledgeBase;
use agent_scope_rag::parser::{Parser, TextParser};
use agent_scope_rag::rag_middleware::{RAGMiddleware, RAGMode};
use agent_scope_rag::turbovec_store::TurbovecVectorStore;
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, Skill, WorkspaceBase};
use clap::Parser as ClapParser;
use futures::StreamExt;

mod render;
mod tools;

use render::{RenderOptions, Renderer, mask_text};
use tools::{
    MemorySnapshot, RagSnapshot, ToolState, WorkspaceSnapshot, WorkspaceToolSummary, build_toolkit,
};

#[derive(Debug, ClapParser)]
#[command(
    name = "agent_demo",
    about = "Interactive AgentScope Rust assistant backed by real DashScope APIs"
)]
struct Cli {
    /// DashScope API key. Usually loaded from repository-root .env as API_KEY.
    #[arg(long, env = "API_KEY", hide_env_values = true)]
    api_key: Option<String>,

    /// DashScope chat model name.
    #[arg(long, default_value = "qwen-plus")]
    model: String,

    /// DashScope embedding model name used by RAG.
    #[arg(long, default_value = "text-embedding-v3")]
    embedding_model: String,

    /// Maximum ReAct reasoning/acting iterations per reply.
    #[arg(long, default_value_t = 20)]
    max_iters: u32,

    /// Show model/reply/tool lifecycle events in the terminal.
    #[arg(long)]
    show_events: bool,

    /// Print redacted AgentEvent JSON lines while streaming.
    #[arg(long)]
    show_json_events: bool,

    /// Send one prompt and exit instead of starting the REPL.
    #[arg(long)]
    prompt: Option<String>,

    /// Disable tools and run as a pure chat agent.
    #[arg(long)]
    no_tools: bool,

    /// Directory used for memory and workspace runtime data.
    #[arg(long, default_value = ".agent-demo")]
    workdir: String,

    /// Disable MemoryMiddleware and memory tools.
    #[arg(long)]
    no_memory: bool,

    /// Disable LocalWorkspace initialization, workspace tools, and workspace skills.
    #[arg(long)]
    no_workspace: bool,

    /// Disable RAGMiddleware.
    #[arg(long)]
    no_rag: bool,

    /// Import a real skill directory into the LocalWorkspace. May be repeated.
    #[arg(long = "skill-path")]
    skill_paths: Vec<PathBuf>,

    /// RAG source document path (.txt, .md, .markdown, .text). May be repeated.
    #[arg(long = "rag-doc")]
    rag_docs: Vec<PathBuf>,

    /// RAG source directory containing text/markdown documents. May be repeated.
    #[arg(long = "rag-dir")]
    rag_dirs: Vec<PathBuf>,

    /// Recursively scan --rag-dir directories.
    #[arg(long)]
    rag_recursive: bool,

    /// Number of RAG chunks to retrieve per query.
    #[arg(long, default_value_t = 3)]
    rag_top_k: usize,

    /// Optional RAG score threshold.
    #[arg(long)]
    rag_threshold: Option<f32>,

    /// Approximate token chunk size for RAG documents.
    #[arg(long, default_value_t = 500)]
    rag_chunk_size: usize,

    /// Approximate token overlap for adjacent RAG chunks.
    #[arg(long, default_value_t = 80)]
    rag_overlap: usize,

    /// Turbovec collection name for this run's RAG documents.
    #[arg(long, default_value = "agent_demo_docs")]
    rag_collection: String,
}

#[derive(Debug)]
enum DemoError {
    MissingApiKey,
    InvalidConfig(String),
    Agent(AgentError),
    Io(io::Error),
}

impl fmt::Display for DemoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey => write!(
                f,
                "Missing API_KEY. Create a repository-root .env file with:\n\n  API_KEY=sk-your-real-key\n\nOr run with --api-key / API_KEY in the environment."
            ),
            Self::InvalidConfig(message) => write!(f, "Invalid configuration: {message}"),
            Self::Agent(err) => write!(f, "Agent error: {err}"),
            Self::Io(err) => write!(f, "I/O error: {err}"),
        }
    }
}

impl std::error::Error for DemoError {}

impl From<AgentError> for DemoError {
    fn from(value: AgentError) -> Self {
        Self::Agent(value)
    }
}

impl From<io::Error> for DemoError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone)]
struct RuntimeConfig {
    api_key: String,
    model: String,
    embedding_model: String,
    max_iters: u32,
    show_events: bool,
    show_json_events: bool,
    prompt: Option<String>,
    no_tools: bool,
    workdir: String,
    memory_enabled: bool,
    workspace_enabled: bool,
    rag_enabled: bool,
    skill_paths: Vec<PathBuf>,
    rag_docs: Vec<PathBuf>,
    rag_dirs: Vec<PathBuf>,
    rag_recursive: bool,
    rag_top_k: usize,
    rag_threshold: Option<f32>,
    rag_chunk_size: usize,
    rag_overlap: usize,
    rag_collection: String,
}

impl RuntimeConfig {
    fn from_cli(cli: Cli) -> Result<Self, DemoError> {
        let api_key = cli
            .api_key
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or(DemoError::MissingApiKey)?;

        if cli.max_iters == 0 {
            return Err(DemoError::InvalidConfig(
                "--max-iters must be greater than 0".to_string(),
            ));
        }
        if cli.rag_top_k == 0 {
            return Err(DemoError::InvalidConfig(
                "--rag-top-k must be greater than 0".to_string(),
            ));
        }
        if cli.rag_chunk_size <= cli.rag_overlap {
            return Err(DemoError::InvalidConfig(
                "--rag-chunk-size must be greater than --rag-overlap".to_string(),
            ));
        }

        let workdir = cli.workdir.trim().to_string();
        if workdir.is_empty() {
            return Err(DemoError::InvalidConfig(
                "--workdir must not be empty".to_string(),
            ));
        }
        let rag_collection = cli.rag_collection.trim().to_string();
        if rag_collection.is_empty() {
            return Err(DemoError::InvalidConfig(
                "--rag-collection must not be empty".to_string(),
            ));
        }

        Ok(Self {
            api_key,
            model: cli.model,
            embedding_model: cli.embedding_model,
            max_iters: cli.max_iters,
            show_events: cli.show_events,
            show_json_events: cli.show_json_events,
            prompt: cli.prompt,
            no_tools: cli.no_tools,
            workdir,
            memory_enabled: !cli.no_memory,
            workspace_enabled: !cli.no_workspace,
            rag_enabled: !cli.no_rag,
            skill_paths: cli.skill_paths,
            rag_docs: cli.rag_docs,
            rag_dirs: cli.rag_dirs,
            rag_recursive: cli.rag_recursive,
            rag_top_k: cli.rag_top_k,
            rag_threshold: cli.rag_threshold,
            rag_chunk_size: cli.rag_chunk_size,
            rag_overlap: cli.rag_overlap,
            rag_collection,
        })
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let cli = Cli::parse();
    let exit_code = match RuntimeConfig::from_cli(cli) {
        Ok(config) => match run(config).await {
            Ok(()) => 0,
            Err(err) => {
                eprintln!("{}", mask_text(&err.to_string(), &[]));
                1
            }
        },
        Err(err) => {
            eprintln!("{}", err);
            2
        }
    };

    std::process::exit(exit_code);
}

async fn run(config: RuntimeConfig) -> Result<(), DemoError> {
    print_banner(&config);

    let agent_context = build_agent(&config).await?;
    print_runtime_summary(&agent_context);

    let render_options = RenderOptions {
        show_events: config.show_events,
        show_json_events: config.show_json_events,
        secrets: vec![config.api_key.clone()],
    };

    if let Some(prompt) = config.prompt.clone() {
        run_turn(&agent_context, &prompt, &render_options).await?;
        return Ok(());
    }

    run_repl(agent_context, config, render_options).await
}

struct AgentContext {
    agent: ReActAgent,
    workspace: WorkspaceBuildResult,
    memory: MemoryBuildResult,
    rag: RagBuildResult,
    session_id: String,
}

fn print_banner(config: &RuntimeConfig) {
    println!("AgentScope Rust Interactive Agent");
    println!("Chat model: {}", config.model);
    println!("Embedding model: {}", config.embedding_model);
    println!(
        "API key: {}",
        mask_text(&config.api_key, std::slice::from_ref(&config.api_key))
    );
    println!("Workdir: {}", config.workdir);
    println!("Type /help for commands, /exit to quit. This program calls real DashScope APIs.\n");
}

fn print_runtime_summary(context: &AgentContext) {
    println!(
        "Memory: {}",
        if context.memory.snapshot.enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!(
        "Memory dir: {}/{}",
        context.memory.snapshot.workdir, context.memory.snapshot.memory_dir
    );
    match &context.workspace.snapshot {
        Some(workspace) => {
            println!("Workspace: enabled ({})", workspace.workdir);
            if context.workspace.skills.is_empty() {
                println!("Workspace skills: none discovered");
            } else {
                let names = context
                    .workspace
                    .skills
                    .iter()
                    .map(|skill| skill.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("Workspace skills: {names}");
            }
        }
        None => println!("Workspace: disabled"),
    }
    if context.rag.snapshot.enabled {
        println!(
            "RAG: enabled ({} mode, {} source file(s), {} chunk(s), collection {}, embedding {})",
            context.rag.snapshot.mode,
            context.rag.snapshot.sources.len(),
            context.rag.snapshot.chunk_count,
            context.rag.snapshot.collection,
            context.rag.snapshot.embedding_model
        );
    } else {
        println!("RAG: disabled");
    }
    println!();
}

async fn build_agent(config: &RuntimeConfig) -> Result<AgentContext, DemoError> {
    let model = Arc::new(DashScopeChatModel::new(&config.api_key, &config.model).with_stream(true));
    let mut middlewares: Vec<Arc<dyn Middleware>> = Vec::new();

    let workspace = build_workspace_snapshot(config).await?;
    let session_id = format!("agent-demo-{}", uuid::Uuid::new_v4().as_simple());
    let memory = build_memory_middleware(config).await?;
    if let Some(middleware) = memory.middleware.clone() {
        middlewares.push(middleware);
    }
    let rag = build_rag_middleware(config).await?;
    if let Some(middleware) = rag.middleware.clone() {
        middlewares.push(middleware);
    }

    let tool_state = ToolState {
        workspace: workspace.snapshot.clone(),
        workspace_exec: workspace.workspace.clone(),
        memory_store: memory.memory.clone(),
    };

    let mut builder = AgentConfig::builder().name("agent_demo").model(model);

    if !config.no_tools {
        let toolkit = build_toolkit(tool_state, workspace.skills.clone());
        let skill_instructions = toolkit.get_skill_instructions(None);
        builder = builder
            .system_prompt(system_prompt(
                config,
                &workspace,
                &memory.snapshot,
                &rag.snapshot,
                Some(&skill_instructions),
            ))
            .toolkit(toolkit)
            .permission_context(build_permission_context(
                config,
                !workspace.skills.is_empty(),
                memory.memory.is_some(),
                workspace.snapshot.is_some(),
            ));
    } else {
        builder = builder.system_prompt(system_prompt(
            config,
            &workspace,
            &memory.snapshot,
            &rag.snapshot,
            None,
        ));
    }

    let agent_config = builder.build()?;
    let react_config = ReActConfig {
        max_iters: config.max_iters,
        ..ReActConfig::default()
    };

    let agent = ReActAgent::new(
        agent_config,
        react_config,
        ContextConfig::default(),
        middlewares,
    )?;

    Ok(AgentContext {
        agent,
        workspace,
        memory,
        rag,
        session_id,
    })
}

#[derive(Clone)]
struct WorkspaceBuildResult {
    snapshot: Option<WorkspaceSnapshot>,
    workspace: Option<Arc<LocalWorkspace>>,
    instructions: Option<String>,
    skills: Vec<Skill>,
}

#[derive(Clone)]
struct MemoryBuildResult {
    middleware: Option<Arc<dyn Middleware>>,
    memory: Option<Arc<dyn Memory>>,
    snapshot: MemorySnapshot,
}

#[derive(Clone)]
struct RagBuildResult {
    middleware: Option<Arc<dyn Middleware>>,
    snapshot: RagSnapshot,
}

async fn build_workspace_snapshot(
    config: &RuntimeConfig,
) -> Result<WorkspaceBuildResult, DemoError> {
    if !config.workspace_enabled {
        return Ok(WorkspaceBuildResult {
            snapshot: None,
            workspace: None,
            instructions: None,
            skills: Vec::new(),
        });
    }

    let skill_paths = config
        .skill_paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let mut workspace = LocalWorkspace::new(LocalWorkspaceConfig {
        workdir: format!("{}/workspace", config.workdir),
        workspace_id: Some("agent-demo-workspace".to_string()),
        default_mcps: vec![],
        skill_paths,
        instructions: None,
    });
    workspace.initialize().await.map_err(|err| {
        DemoError::InvalidConfig(format!("failed to initialize workspace: {err}"))
    })?;

    let tools = workspace
        .list_tools()
        .await
        .map_err(|err| DemoError::InvalidConfig(format!("failed to list workspace tools: {err}")))?
        .into_iter()
        .map(|tool| WorkspaceToolSummary {
            name: tool.name,
            description: tool.description,
        })
        .collect::<Vec<_>>();
    let skills = workspace.list_skills().await.map_err(|err| {
        DemoError::InvalidConfig(format!("failed to list workspace skills: {err}"))
    })?;
    let instructions = workspace.get_instructions().await;
    let instructions_summary = first_line_or_chars(&instructions, 120);
    let snapshot = WorkspaceSnapshot {
        workspace_id: workspace.workspace_id().to_string(),
        workdir: workspace.workdir().to_string(),
        is_alive: workspace.is_alive(),
        instructions_summary,
        tools,
    };

    Ok(WorkspaceBuildResult {
        snapshot: Some(snapshot),
        workspace: Some(Arc::new(workspace)),
        instructions: Some(instructions),
        skills,
    })
}

async fn build_memory_middleware(config: &RuntimeConfig) -> Result<MemoryBuildResult, DemoError> {
    let memory_dir = "Memory".to_string();
    let snapshot = MemorySnapshot {
        enabled: config.memory_enabled,
        workdir: config.workdir.clone(),
        memory_dir: memory_dir.clone(),
    };

    if !config.memory_enabled {
        return Ok(MemoryBuildResult {
            middleware: None,
            memory: None,
            snapshot,
        });
    }

    let memory_config = MemoryConfig {
        memory_dir,
        retrieval_max_files: 5,
        ..MemoryConfig::default()
    };
    let memory = Arc::new(FileMemory::new(
        &config.workdir,
        memory_config.clone(),
        None,
    ));
    let memory: Arc<dyn Memory> = memory;
    Ok(MemoryBuildResult {
        middleware: Some(Arc::new(MemoryMiddleware::new(
            Arc::clone(&memory),
            memory_config,
        ))),
        memory: Some(memory),
        snapshot,
    })
}

async fn build_rag_middleware(config: &RuntimeConfig) -> Result<RagBuildResult, DemoError> {
    let disabled_snapshot = RagSnapshot {
        enabled: false,
        mode: "static".to_string(),
        sources: Vec::new(),
        chunk_count: 0,
        collection: config.rag_collection.clone(),
        embedding_model: config.embedding_model.clone(),
    };

    if !config.rag_enabled {
        return Ok(RagBuildResult {
            middleware: None,
            snapshot: disabled_snapshot,
        });
    }

    let paths = collect_rag_paths(config)?;
    if paths.is_empty() {
        return Ok(RagBuildResult {
            middleware: None,
            snapshot: disabled_snapshot,
        });
    }

    let chunks = build_rag_chunks(&paths, config.rag_chunk_size, config.rag_overlap)?;
    if chunks.is_empty() {
        return Err(DemoError::InvalidConfig(
            "RAG sources produced no chunks".to_string(),
        ));
    }

    let sources = paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let chunk_count = chunks.len();
    let embedding = Arc::new(DashScopeEmbeddingModel::new(
        config.api_key.clone(),
        EmbeddingModelCard::new(&config.embedding_model, 1536, false),
    ));
    let store = Arc::new(TurbovecVectorStore::new(4).map_err(|err| {
        DemoError::InvalidConfig(format!("failed to create Turbovec vector store: {err}"))
    })?);
    let kb = Arc::new(KnowledgeBase::new(
        "agent_demo_docs".to_string(),
        "User-provided documents indexed for this AgentScope Rust run.".to_string(),
        embedding,
        store,
        config.rag_collection.clone(),
        None,
    ));
    kb.insert_document(chunks, Some("agent-demo-input-docs".to_string()), None)
        .await
        .map_err(|err| DemoError::InvalidConfig(format!("failed to index RAG documents: {err}")))?;

    let snapshot = RagSnapshot {
        enabled: true,
        mode: "static".to_string(),
        sources,
        chunk_count,
        collection: config.rag_collection.clone(),
        embedding_model: config.embedding_model.clone(),
    };

    Ok(RagBuildResult {
        middleware: Some(Arc::new(RAGMiddleware::new(
            vec![kb],
            RAGMode::Static,
            config.rag_top_k,
            config.rag_threshold,
        ))),
        snapshot,
    })
}

fn collect_rag_paths(config: &RuntimeConfig) -> Result<Vec<PathBuf>, DemoError> {
    let mut paths = Vec::new();
    for doc in &config.rag_docs {
        if !doc.exists() {
            return Err(DemoError::InvalidConfig(format!(
                "RAG document does not exist: {}",
                doc.display()
            )));
        }
        if !doc.is_file() {
            return Err(DemoError::InvalidConfig(format!(
                "RAG document is not a file: {}",
                doc.display()
            )));
        }
        if !is_supported_rag_file(doc) {
            return Err(DemoError::InvalidConfig(format!(
                "unsupported RAG document format: {}",
                doc.display()
            )));
        }
        paths.push(doc.clone());
    }

    for dir in &config.rag_dirs {
        if !dir.exists() {
            return Err(DemoError::InvalidConfig(format!(
                "RAG directory does not exist: {}",
                dir.display()
            )));
        }
        if !dir.is_dir() {
            return Err(DemoError::InvalidConfig(format!(
                "RAG directory is not a directory: {}",
                dir.display()
            )));
        }
        collect_rag_dir(dir, config.rag_recursive, &mut paths)?;
    }

    paths.sort();
    let mut seen = HashSet::new();
    paths.retain(|path| seen.insert(path.clone()));

    if (!config.rag_docs.is_empty() || !config.rag_dirs.is_empty()) && paths.is_empty() {
        return Err(DemoError::InvalidConfig(
            "RAG sources did not contain any supported .txt/.md/.markdown/.text files".to_string(),
        ));
    }

    Ok(paths)
}

fn collect_rag_dir(dir: &Path, recursive: bool, paths: &mut Vec<PathBuf>) -> Result<(), DemoError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && recursive {
            collect_rag_dir(&path, recursive, paths)?;
        } else if path.is_file() && is_supported_rag_file(&path) {
            paths.push(path);
        }
    }
    Ok(())
}

fn is_supported_rag_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "txt" | "md" | "markdown" | "text"
            )
        })
        .unwrap_or(false)
}

fn build_rag_chunks(
    paths: &[PathBuf],
    chunk_size: usize,
    overlap: usize,
) -> Result<Vec<Chunk>, DemoError> {
    let parser = TextParser;
    let chunker = ApproxTokenChunker::new(chunk_size, overlap);
    let mut all_chunks = Vec::new();

    for path in paths {
        let bytes = std::fs::read(path)?;
        let source = path.to_string_lossy().to_string();
        let sections = parser
            .parse(bytes, &source)
            .map_err(|err| DemoError::InvalidConfig(format!("failed to parse {source}: {err}")))?;
        let mut chunks = chunker
            .chunk(sections)
            .map_err(|err| DemoError::InvalidConfig(format!("failed to chunk {source}: {err}")))?;
        for chunk in &mut chunks {
            chunk.metadata.insert("path".to_string(), source.clone());
        }
        all_chunks.extend(chunks);
    }

    Ok(all_chunks)
}

fn first_line_or_chars(text: &str, max_chars: usize) -> String {
    let first = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    if first.chars().count() <= max_chars {
        return first.to_string();
    }
    let mut truncated = first.chars().take(max_chars).collect::<String>();
    truncated.push('…');
    truncated
}

fn build_permission_context(
    _config: &RuntimeConfig,
    has_skills: bool,
    has_memory: bool,
    has_workspace: bool,
) -> PermissionContext {
    let mut context = PermissionContext::default();
    context.add_rule(PermissionRule::allow("calculator"));
    context.add_rule(PermissionRule::allow("safe_time"));
    if has_workspace {
        context.add_rule(PermissionRule::allow("workspace_info"));
        context.add_rule(PermissionRule::allow("workspace_list_tools"));
        context.add_rule(PermissionRule::allow("workspace_write_file"));
        context.add_rule(PermissionRule::allow("Bash"));
    }
    if has_memory {
        context.add_rule(PermissionRule::allow("memory_write"));
        context.add_rule(PermissionRule::allow("memory_search"));
        context.add_rule(PermissionRule::allow("memory_read"));
        context.add_rule(PermissionRule::allow("memory_list"));
    }
    if has_skills {
        context.add_rule(PermissionRule::allow("Skill"));
    }
    context
}

fn system_prompt(
    config: &RuntimeConfig,
    workspace: &WorkspaceBuildResult,
    memory: &MemorySnapshot,
    rag: &RagSnapshot,
    skill_instructions: Option<&str>,
) -> String {
    let mut prompt = String::from(
        "You are an AgentScope Rust interactive assistant backed by real DashScope APIs.\n\nNever reveal, repeat, or infer API keys or secrets.",
    );

    if config.no_tools {
        prompt.push_str("\nAnswer clearly and concisely. Tool calling is disabled for this run.");
    } else {
        prompt.push_str(
            "\n\nUse available tools when they improve accuracy:\n- Use calculator for arithmetic instead of doing math silently.\n- Use safe_time when the user asks for current time.\n- Never fabricate tool, skill, memory, workspace, or retrieval results.\n- Do not claim something was saved, written, read, or retrieved unless the corresponding tool or middleware result supports it.",
        );
        if memory.enabled {
            prompt.push_str("\n- Use memory_write when the user asks you to remember stable non-secret information. Use memory_search before answering questions that depend on saved memories; use memory_read for exact memory names and memory_list to discover available memories.");
        }
        if workspace.snapshot.is_some() {
            prompt.push_str("\n- Use workspace_info and workspace_list_tools for workspace questions. Use Bash when shell command output is needed. Low-risk diagnostics run directly; potentially risky Bash commands will ask the terminal user for confirmation. Use workspace_write_file when the user asks to create or write a UTF-8 text file.");
        }
        if let Some(instructions) = skill_instructions
            && !instructions.trim().is_empty()
        {
            prompt.push_str("\n- Use Skill only for skills listed in <agent-skills>; if no skills are listed, do not claim skills are available.");
            prompt.push_str("\n\n");
            prompt.push_str(instructions);
        }
    }

    prompt.push_str("\n\nEnabled runtime capabilities:");
    prompt.push_str(if memory.enabled {
        "\n- MemoryMiddleware: FileMemory is available with the current MEMORY.md index and may inject relevant memories as hints. No memories are pre-seeded by this program."
    } else {
        "\n- MemoryMiddleware: disabled for this run."
    });
    if let Some(snapshot) = &workspace.snapshot {
        prompt.push_str("\n- LocalWorkspace: initialized at ");
        prompt.push_str(&snapshot.workdir);
        if workspace.skills.is_empty() {
            prompt.push_str("; no workspace skills were discovered.");
        } else {
            prompt
                .push_str("; workspace skills were discovered and exposed through the Skill tool.");
        }
    } else {
        prompt.push_str("\n- LocalWorkspace: disabled for this run.");
    }
    if rag.enabled {
        prompt.push_str("\n- Static RAGMiddleware: user-provided documents were indexed with DashScope embeddings and Turbovec. Use injected RAG context when it appears relevant and avoid inventing sources.");
    } else {
        prompt.push_str(
            "\n- Static RAGMiddleware: disabled for this run or no RAG documents were provided.",
        );
    }

    if let Some(instructions) = workspace.instructions.as_deref() {
        prompt.push_str("\n\nWorkspace instructions:\n");
        prompt.push_str(instructions);
    }

    prompt.push_str("\n\nKeep replies concise and grounded in the available tools and context.");
    prompt
}

async fn run_repl(
    agent_context: AgentContext,
    config: RuntimeConfig,
    mut render_options: RenderOptions,
) -> Result<(), DemoError> {
    loop {
        print!("you> ");
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            println!();
            return Ok(());
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        if handle_command(input, &config, &mut render_options)? {
            continue;
        }

        run_turn(&agent_context, input, &render_options).await?;
    }
}

fn handle_command(
    input: &str,
    config: &RuntimeConfig,
    render_options: &mut RenderOptions,
) -> Result<bool, DemoError> {
    match input {
        "/exit" | "/quit" => std::process::exit(0),
        "/help" => {
            println!("Commands:");
            println!("  /help          Show this help");
            println!("  /model         Show current model");
            println!("  /tools         Show configured tool categories");
            println!("  /events on|off Toggle lifecycle event rendering");
            println!("  /json on|off   Toggle redacted AgentEvent JSON output");
            println!("  /exit, /quit   Quit");
            println!("\nConfiguration:");
            println!("  memory:    {}", enabled_label(config.memory_enabled));
            println!("  workspace: {}", enabled_label(config.workspace_enabled));
            println!("  rag:       {}", enabled_label(config.rag_enabled));
            println!("  workdir:   {}", config.workdir);
            println!("\nUseful prompts:");
            println!("  请用 calculator 计算 23 * (17 + 5)");
            println!("  现在的 UTC 时间是什么？");
            println!("  请记住：我偏好先给结论，再列步骤。");
            println!("  你记得我的回答偏好吗？请先查询记忆再回答。");
            println!("  请介绍 workspace 当前有哪些工具。");
            println!("  请调用 Bash 执行 pwd，并告诉我返回了什么。");
            println!("  请创建 hello.txt，写入 Hello World!");
            println!("  如果启用了 RAG，请基于已加载文档回答我的问题。");
            Ok(true)
        }
        "/model" => {
            println!("Chat model: {}", config.model);
            println!("Embedding model: {}", config.embedding_model);
            Ok(true)
        }
        "/tools" => {
            if config.no_tools {
                println!("Tools are disabled for this run.");
            } else {
                println!("Always available tools:");
                println!("  calculator               Safe arithmetic expression evaluator");
                println!("  safe_time                Current local/UTC timestamp");
                if config.workspace_enabled {
                    println!("\nWorkspace tools:");
                    println!(
                        "  Bash                    Restricted read-only shell diagnostics inside the workspace"
                    );
                    println!("  workspace_info          Active LocalWorkspace status");
                    println!("  workspace_list_tools     LocalWorkspace tool inventory");
                    println!(
                        "  workspace_write_file     Confirmed UTF-8 file writes inside the workspace"
                    );
                    println!(
                        "  Skill                    Available only when workspace skills are discovered"
                    );
                }
                if config.memory_enabled {
                    println!("\nMemory tools:");
                    println!("  memory_write             Save or update durable memories");
                    println!("  memory_search            Search durable memories by keyword");
                    println!("  memory_read              Read a durable memory by exact name");
                    println!("  memory_list              List durable memory entries");
                }
                println!(
                    "\nRAG is middleware-based and is enabled only when real documents are provided with --rag-doc or --rag-dir."
                );
            }
            Ok(true)
        }
        "/events on" => {
            render_options.show_events = true;
            println!("Lifecycle events: on");
            Ok(true)
        }
        "/events off" => {
            render_options.show_events = false;
            println!("Lifecycle events: off");
            Ok(true)
        }
        "/json on" => {
            render_options.show_json_events = true;
            println!("Redacted JSON events: on");
            Ok(true)
        }
        "/json off" => {
            render_options.show_json_events = false;
            println!("Redacted JSON events: off");
            Ok(true)
        }
        command if command.starts_with('/') => {
            println!("Unknown command: {command}. Type /help for available commands.");
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn enabled_label(enabled: bool) -> &'static str {
    if enabled { "enabled" } else { "disabled" }
}

async fn run_turn(
    context: &AgentContext,
    input: &str,
    render_options: &RenderOptions,
) -> Result<(), DemoError> {
    let msg =
        user_msg("user", input).map_err(|err| DemoError::InvalidConfig(format!("{err:?}")))?;
    if let Some(workspace) = &context.workspace.workspace {
        workspace
            .offload_context(&context.session_id, std::slice::from_ref(&msg))
            .await
            .map_err(|err| {
                DemoError::InvalidConfig(format!("failed to offload user context: {err}"))
            })?;
    }

    let mut stream = context.agent.reply_stream(Some(vec![msg])).await?;
    let mut renderer = Renderer::new(render_options.clone());
    let mut assistant_text = String::new();

    println!("assistant>");
    while let Some(event) = stream.next().await {
        if let AgentEvent::TextBlockDelta(delta) = &event {
            assistant_text.push_str(&delta.delta);
        }
        renderer.render(&event)?;
    }
    renderer.finish()?;
    println!();

    if let Some(workspace) = &context.workspace.workspace {
        let assistant = assistant_msg("agent_demo", &assistant_text);
        workspace
            .offload_context(&context.session_id, &[assistant])
            .await
            .map_err(|err| {
                DemoError::InvalidConfig(format!("failed to offload assistant context: {err}"))
            })?;
    }

    Ok(())
}
