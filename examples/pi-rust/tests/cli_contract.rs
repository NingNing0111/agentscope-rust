use std::sync::{Mutex, MutexGuard};

use clap::{CommandFactory, Parser};
use pi_rust::config::{Cli, ProviderConfig, RunMode, RuntimeConfig, mask_secret};
use pi_rust::repl::{LocalCommand, parse_repl_command};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn lock_env() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap()
}

fn scoped_env_remove(key: &str) {
    // SAFETY: Environment mutation is serialized by ENV_LOCK in these tests.
    unsafe { std::env::remove_var(key) };
}

fn scoped_env_set(key: &str, value: &str) {
    // SAFETY: Environment mutation is serialized by ENV_LOCK in these tests.
    unsafe { std::env::set_var(key, value) };
}

#[test]
fn help_contract_contains_core_options() {
    let mut cmd = Cli::command();
    let help = cmd.render_long_help().to_string();
    for expected in [
        "--api-key",
        "--model",
        "--workdir",
        "--cwd",
        "--mode",
        "--skill-path",
        "--prompt",
        "--resume",
        "--list-sessions",
        "--no-tools",
        "--no-memory",
        "--no-rag",
        "--max-iters",
        "--command-timeout-secs",
        "--show-events",
        "--show-json-events",
    ] {
        assert!(
            help.contains(expected),
            "missing {expected} in help: {help}"
        );
    }
}

#[test]
fn repl_command_contract_keywords_are_stable() {
    let cases = [
        ("", LocalCommand::Empty),
        ("   ", LocalCommand::Empty),
        ("/help", LocalCommand::Help),
        ("/model", LocalCommand::Model),
        ("/tools", LocalCommand::Tools),
        ("/skills", LocalCommand::Skills),
        ("/skill demo", LocalCommand::Skill("demo".into())),
        ("/sessions", LocalCommand::Sessions),
        ("/save", LocalCommand::Save),
        ("/events on", LocalCommand::Events(true)),
        ("/events off", LocalCommand::Events(false)),
        ("/json on", LocalCommand::Json(true)),
        ("/json off", LocalCommand::Json(false)),
        ("/exit", LocalCommand::Exit),
        ("/quit", LocalCommand::Exit),
        ("/wat", LocalCommand::Unknown("/wat".into())),
    ];
    for (input, expected) in cases {
        assert_eq!(parse_repl_command(input), expected);
    }
}

#[test]
fn coding_mode_and_skill_paths_are_configured() {
    let _guard = lock_env();
    scoped_env_remove("API_KEY");
    scoped_env_remove("DASHSCOPE_API_KEY");
    let cwd = tempfile::tempdir().unwrap();
    let skill_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        skill_dir.path().join("SKILL.md"),
        "---\nname: demo\ndescription: Demo skill\n---\n\n# Demo\n",
    )
    .unwrap();
    let cli = Cli::parse_from([
        "pi-rust",
        "--api-key",
        "sk-mode",
        "--cwd",
        cwd.path().to_str().unwrap(),
        "--mode",
        "coding",
        "--skill-path",
        skill_dir.path().to_str().unwrap(),
    ]);
    let config = RuntimeConfig::from_cli(cli).unwrap();
    assert_eq!(config.mode, RunMode::Coding);
    assert_eq!(config.skill_paths.len(), 1);
}

#[test]
fn coding_mode_requires_tools() {
    let _guard = lock_env();
    scoped_env_remove("API_KEY");
    scoped_env_remove("DASHSCOPE_API_KEY");
    let cwd = tempfile::tempdir().unwrap();
    let cli = Cli::parse_from([
        "pi-rust",
        "--api-key",
        "sk-mode",
        "--cwd",
        cwd.path().to_str().unwrap(),
        "--mode",
        "coding",
        "--no-tools",
    ]);
    let err = RuntimeConfig::from_cli(cli).unwrap_err();
    assert_eq!(err.exit_code(), 2);
    assert!(err.safe_message().contains("requires tools"));
}

#[test]
fn config_uses_dashscope_defaults_and_masks_key() {
    let _guard = lock_env();
    scoped_env_remove("API_KEY");
    scoped_env_remove("DASHSCOPE_API_KEY");
    let dir = tempfile::tempdir().unwrap();
    let cli = Cli::parse_from([
        "pi-rust",
        "--api-key",
        "sk-1234567890",
        "--cwd",
        dir.path().to_str().unwrap(),
    ]);
    let config = RuntimeConfig::from_cli(cli).unwrap();
    assert_eq!(config.provider.name(), "dashscope");
    assert_eq!(config.model, "qwen-plus");
    assert_eq!(config.masked_api_key, "sk-1…7890");
}

#[test]
fn env_api_key_alias_is_supported() {
    let _guard = lock_env();
    scoped_env_remove("API_KEY");
    scoped_env_set("DASHSCOPE_API_KEY", "dashscope-secret");
    let dir = tempfile::tempdir().unwrap();
    let cli = Cli::parse_from(["pi-rust", "--cwd", dir.path().to_str().unwrap()]);
    let config = RuntimeConfig::from_cli(cli).unwrap();
    assert_eq!(config.api_key, "dashscope-secret");
    scoped_env_remove("DASHSCOPE_API_KEY");
}

#[test]
fn missing_credentials_is_config_error() {
    let _guard = lock_env();
    scoped_env_remove("API_KEY");
    scoped_env_remove("DASHSCOPE_API_KEY");
    let dir = tempfile::tempdir().unwrap();
    let cli = Cli::parse_from(["pi-rust", "--cwd", dir.path().to_str().unwrap()]);
    let err = RuntimeConfig::from_cli(cli).unwrap_err();
    assert_eq!(err.exit_code(), 2);
    assert!(err.safe_message().contains("provide --api-key"));
}

#[test]
fn invalid_numeric_options_fail_validation() {
    let _guard = lock_env();
    scoped_env_remove("API_KEY");
    scoped_env_remove("DASHSCOPE_API_KEY");
    let dir = tempfile::tempdir().unwrap();
    let cli = Cli::parse_from([
        "pi-rust",
        "--api-key",
        "sk-test",
        "--cwd",
        dir.path().to_str().unwrap(),
        "--max-iters",
        "0",
    ]);
    let err = RuntimeConfig::from_cli(cli).unwrap_err();
    assert_eq!(err.exit_code(), 2);
}

#[test]
fn unsupported_provider_is_explicit_error() {
    let err = ProviderConfig::from_name("openai").unwrap_err();
    assert_eq!(err.exit_code(), 1);
    assert!(err.safe_message().contains("not implemented"));
}

#[test]
fn mask_secret_never_returns_raw_secret() {
    assert_eq!(mask_secret("short"), "****");
    assert_eq!(mask_secret("abcdefghi"), "abcd…fghi");
}
