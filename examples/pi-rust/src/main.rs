//! pi-rust coding Agent example.
//!
//! The TypeScript project under `examples/pi-rust/pi-ts` is reference material only.
//! This binary is a standalone Rust implementation built on agentscope-rust crates.

#![deny(unsafe_code)]

use clap::Parser;
use pi_rust::config::{Cli, RuntimeConfig};
use pi_rust::error::PiResult;
use pi_rust::{agent, repl, session};

#[tokio::main]
async fn main() {
    init_tracing();

    let exit_code = match RuntimeConfig::from_cli(Cli::parse()) {
        Ok(config) => match run(config).await {
            Ok(()) => 0,
            Err(err) => {
                eprintln!("{}", err.safe_message());
                err.exit_code()
            }
        },
        Err(err) => {
            eprintln!("{}", err.safe_message());
            err.exit_code()
        }
    };

    std::process::exit(exit_code);
}

async fn run(config: RuntimeConfig) -> PiResult<()> {
    if config.list_sessions {
        let sessions = session::SessionStore::new(config.workdir.join("sessions"));
        for summary in sessions.list()? {
            println!(
                "{}  {}  {}",
                summary.id, summary.updated_at, summary.summary
            );
        }
        return Ok(());
    }

    let runtime = agent::AgentRuntime::build(config).await?;
    if let Some(prompt) = runtime.config.prompt.clone() {
        repl::run_one_shot(runtime, prompt).await
    } else {
        repl::run_interactive(runtime).await
    }
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .init();
}
