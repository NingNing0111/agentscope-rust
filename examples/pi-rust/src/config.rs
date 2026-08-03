use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::error::{PiError, PiResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RunMode {
    React,
    Coding,
}

impl RunMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::React => "react",
            Self::Coding => "coding",
        }
    }
}

#[derive(Debug, Clone, Parser)]
#[command(
    name = "pi-rust",
    about = "Interactive AgentScope Rust coding assistant"
)]
pub struct Cli {
    #[arg(long, env = "API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,

    #[arg(long, default_value = "qwen-plus")]
    pub model: String,

    #[arg(long, default_value = ".pi-rust")]
    pub workdir: PathBuf,

    #[arg(long, default_value = ".")]
    pub cwd: PathBuf,

    #[arg(long, value_enum, default_value_t = RunMode::React)]
    pub mode: RunMode,

    #[arg(long = "skill-path")]
    pub skill_paths: Vec<PathBuf>,

    #[arg(long)]
    pub prompt: Option<String>,

    #[arg(long, num_args = 0..=1, default_missing_value = "__latest__")]
    pub resume: Option<String>,

    #[arg(long)]
    pub list_sessions: bool,

    #[arg(long)]
    pub no_tools: bool,

    #[arg(long)]
    pub no_memory: bool,

    #[arg(long)]
    pub no_rag: bool,

    #[arg(long, default_value_t = 20)]
    pub max_iters: u32,

    #[arg(long, default_value_t = 30)]
    pub command_timeout_secs: u64,

    #[arg(long)]
    pub show_events: bool,

    #[arg(long)]
    pub show_json_events: bool,
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub api_key: String,
    pub masked_api_key: String,
    pub model: String,
    pub provider: ProviderConfig,
    pub workdir: PathBuf,
    pub cwd: PathBuf,
    pub mode: RunMode,
    pub skill_paths: Vec<PathBuf>,
    pub prompt: Option<String>,
    pub resume: Option<String>,
    pub list_sessions: bool,
    pub no_tools: bool,
    pub no_memory: bool,
    pub no_rag: bool,
    pub max_iters: u32,
    pub command_timeout_secs: u64,
    pub show_events: bool,
    pub show_json_events: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderConfig {
    DashScope,
}

impl ProviderConfig {
    pub fn from_name(name: &str) -> PiResult<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "dashscope" | "" => Ok(Self::DashScope),
            other => Err(PiError::unsupported(format!(
                "provider '{other}' is not implemented; pi-rust currently supports DashScope"
            ))),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::DashScope => "dashscope",
        }
    }
}

impl RuntimeConfig {
    pub fn from_cli(cli: Cli) -> PiResult<Self> {
        let api_key = resolve_api_key(cli.api_key)?;
        let model = validate_non_empty("model", cli.model)?;
        let workdir = validate_non_empty_path("workdir", cli.workdir)?;
        let cwd = validate_cwd(cli.cwd)?;
        let prompt = match cli.prompt {
            Some(prompt) if prompt.trim().is_empty() => {
                return Err(PiError::config("prompt", "--prompt must not be empty"));
            }
            other => other,
        };
        if cli.max_iters == 0 {
            return Err(PiError::config("max_iters", "--max-iters must be > 0"));
        }
        if cli.command_timeout_secs == 0 {
            return Err(PiError::config(
                "command_timeout_secs",
                "--command-timeout-secs must be > 0",
            ));
        }
        if cli.mode == RunMode::Coding && cli.no_tools {
            return Err(PiError::config(
                "mode",
                "--mode coding requires tools; remove --no-tools",
            ));
        }
        let skill_paths = validate_skill_paths(cli.skill_paths)?;

        Ok(Self {
            masked_api_key: mask_secret(&api_key),
            api_key,
            model,
            provider: ProviderConfig::DashScope,
            workdir,
            cwd,
            mode: cli.mode,
            skill_paths,
            prompt,
            resume: cli.resume,
            list_sessions: cli.list_sessions,
            no_tools: cli.no_tools,
            no_memory: cli.no_memory,
            no_rag: cli.no_rag,
            max_iters: cli.max_iters,
            command_timeout_secs: cli.command_timeout_secs,
            show_events: cli.show_events,
            show_json_events: cli.show_json_events,
        })
    }
}

pub fn resolve_api_key(cli_value: Option<String>) -> PiResult<String> {
    let candidate = cli_value
        .or_else(|| std::env::var("API_KEY").ok())
        .or_else(|| std::env::var("DASHSCOPE_API_KEY").ok())
        .unwrap_or_default();
    validate_non_empty("api_key", candidate).map_err(|_| {
        PiError::config(
            "api_key",
            "provide --api-key, API_KEY, or DASHSCOPE_API_KEY",
        )
    })
}

pub fn mask_secret(secret: &str) -> String {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= 8 {
        return "****".to_string();
    }
    let prefix: String = chars.iter().take(4).collect();
    let suffix: String = chars
        .iter()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{prefix}…{suffix}")
}

fn validate_non_empty(field: &'static str, value: String) -> PiResult<String> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        Err(PiError::config(field, format!("{field} must not be empty")))
    } else {
        Ok(trimmed)
    }
}

fn validate_non_empty_path(field: &'static str, value: PathBuf) -> PiResult<PathBuf> {
    if value.as_os_str().is_empty() {
        Err(PiError::config(field, format!("{field} must not be empty")))
    } else {
        Ok(value)
    }
}

fn validate_cwd(value: PathBuf) -> PiResult<PathBuf> {
    let cwd = validate_non_empty_path("cwd", value)?;
    if !cwd.exists() {
        return Err(PiError::config("cwd", "--cwd must exist"));
    }
    if !cwd.is_dir() {
        return Err(PiError::config("cwd", "--cwd must be a directory"));
    }
    cwd.canonicalize()
        .map_err(|err| PiError::io("canonicalize cwd", err))
}

fn validate_skill_paths(values: Vec<PathBuf>) -> PiResult<Vec<PathBuf>> {
    values
        .into_iter()
        .map(|path| {
            let path = validate_non_empty_path("skill_path", path)?;
            if !path.exists() {
                return Err(PiError::config("skill_path", "--skill-path must exist"));
            }
            if !path.is_dir() {
                return Err(PiError::config(
                    "skill_path",
                    "--skill-path must be a directory containing SKILL.md",
                ));
            }
            path.canonicalize()
                .map_err(|err| PiError::io("canonicalize skill path", err))
        })
        .collect()
}
