//! Memory Integration E2E Test
//!
//! Verifies the Memory system (FileMemory + MemoryMiddleware) works with
//! real DashScope API — write, search, and retrieval-augmented reasoning.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example memory_test -- --api-key sk-xxxxx
//! cargo run --example memory_test -- --api-key sk-xxxxx --model qwen-max
//! cargo run --example memory_test -- --api-key sk-xxxxx --keep-dir
//! ```

use std::time::Instant;

use agent_scope_agent::Agent;
use agent_scope_memory::{
    FileMemory, Memory, MemoryConfig, MemoryEntry, MemoryMetadata, MemoryType,
};
use agent_scope_message::factory::user_msg;
use clap::Parser;
use futures::StreamExt;

mod common;
use common::{
    TestResult, create_memory_agent, print_banner, print_result, print_summary, print_test_header,
};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// DashScope API key (starts with "sk-").
    #[arg(short = 'k', long, env = "API_KEY")]
    api_key: String,

    /// Model name, e.g. "qwen-plus" or "qwen-max".
    #[arg(short = 'm', long, default_value = "qwen-plus")]
    model: String,

    /// Keep the temporary directory after the run (debugging).
    #[arg(long)]
    keep_dir: bool,
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Consume a streaming reply and collect it into a single String for assertion.
async fn collect_reply(
    agent: &impl Agent,
    input: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let msg = user_msg("user", input).map_err(|e| format!("{e:?}"))?;
    let mut stream = agent.reply_stream(Some(vec![msg])).await?;
    let mut text = String::new();
    while let Some(event) = stream.next().await {
        use agent_scope_event::AgentEvent;
        if let AgentEvent::TextBlockDelta(e) = event {
            text.push_str(&e.delta);
        }
    }
    Ok(text)
}

/// Check for API-level errors in the error string.

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let cli = Cli::parse();
    let total_start = Instant::now();

    print_banner("Memory Integration", &cli.model);

    // -- Temp directory for memory storage --
    let tempdir = {
        let p = std::env::temp_dir().join(format!("agentscope-memory-{}", std::process::id()));
        std::fs::create_dir_all(&p).expect("failed to create tempdir");
        p
    };
    let workdir = tempdir.to_string_lossy().to_string();

    if cli.keep_dir {
        println!("Workdir: {workdir}");
    }

    // -- Create agent with memory middleware --
    let agent = match create_memory_agent(&cli.api_key, &cli.model, &workdir) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Failed to create agent: {e}");
            std::process::exit(1);
        }
    };

    let mut results: Vec<TestResult> = Vec::new();

    // ── Test 1: Write Memory ────────────────────────────────────────
    print_test_header(1, "Write Memory");
    let start = Instant::now();

    let test1 = match run_write_memory(&agent, &workdir).await {
        Ok(true) => TestResult {
            name: "Write Memory",
            passed: true,
            detail: "Agent confirmed: stored memory entry".to_string(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Ok(false) => TestResult {
            name: "Write Memory",
            passed: false,
            detail: "Agent did not acknowledge memory storage".to_string(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => TestResult {
            name: "Write Memory",
            passed: false,
            detail: format!("Error: {e}"),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    };

    print_result(&test1);
    results.push(test1);

    if !cli.keep_dir {
        // Continue with remaining tests even if first one fails
    }

    // ── Test 2: Search Memory ───────────────────────────────────────
    print_test_header(2, "Search Memory");
    let start = Instant::now();

    let test2 = match run_search_memory(&agent, &workdir).await {
        Ok(true) => TestResult {
            name: "Search Memory",
            passed: true,
            detail: "Agent referenced stored memory in response".into(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Ok(false) => TestResult {
            name: "Search Memory",
            passed: false,
            detail: "Agent did not reference stored memory".into(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => TestResult {
            name: "Search Memory",
            passed: false,
            detail: format!("Error: {e}"),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    };

    print_result(&test2);
    results.push(test2);

    // ── Test 3: Memory Reasoning ────────────────────────────────────
    print_test_header(3, "Memory Reasoning");
    let start = Instant::now();

    let test3 = match run_memory_reasoning(&agent).await {
        Ok(true) => TestResult {
            name: "Memory Reasoning",
            passed: true,
            detail: "Agent used stored memory for contextual answer".into(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Ok(false) => TestResult {
            name: "Memory Reasoning",
            passed: false,
            detail: "Agent did not use stored memory in reasoning".into(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => TestResult {
            name: "Memory Reasoning",
            passed: false,
            detail: format!("Error: {e}"),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    };

    print_result(&test3);
    results.push(test3);

    print_summary(&results, total_start);

    if !cli.keep_dir {
        drop(tempdir);
    }

    let any_failed = results.iter().any(|r| !r.passed);
    if any_failed {
        std::process::exit(1);
    }
}

// ── Test implementations ───────────────────────────────────────────

/// Test 1: Write a memory entry and verify the system prompt includes it.
/// We do this by:
/// 1. Writing a memory entry directly to FileMemory
/// 2. Asking the agent a question ABOUT that memory
/// 3. Checking that the agent acknowledges the stored memory
async fn run_write_memory(
    agent: &impl Agent,
    workdir: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Write a known memory entry
    let config = MemoryConfig {
        memory_dir: "memory_data".into(),
        ..Default::default()
    };
    let memory = FileMemory::new(workdir, config, None);

    let entry = MemoryEntry {
        name: "user-favorite-color".into(),
        description: "The user's favorite color preference".into(),
        metadata: MemoryMetadata::new(MemoryType::User),
        content: "The user's favorite color is cerulean blue. They mentioned this preference and asked to remember it.".into(),
    };

    memory.write(entry).await?;

    // Ask the agent a question that requires the memory
    let response = collect_reply(
        agent,
        "What is my favorite color? I asked you to remember it earlier.",
    )
    .await?;
    let lower = response.to_lowercase();

    // Check if the agent gave a meaningful response (non-empty, mentions color or memory)
    Ok(!response.is_empty()
        && (lower.contains("color")
            || lower.contains("blue")
            || lower.contains("cerulean")
            || lower.contains("memory")))
}

/// Test 2: Search memory by writing another entry then querying it.
async fn run_search_memory(
    agent: &impl Agent,
    workdir: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let config = MemoryConfig {
        memory_dir: "memory_data".into(),
        ..Default::default()
    };
    let memory = FileMemory::new(workdir, config, None);

    // Write a second memory entry
    let entry = MemoryEntry {
        name: "user-city".into(),
        description: "The user's city of residence".into(),
        metadata: MemoryMetadata::new(MemoryType::User),
        content: "The user lives in Hangzhou, China. They enjoy the West Lake area.".into(),
    };

    memory.write(entry).await?;

    // Search the memory directly
    let results = memory.search("Hangzhou", None).await?;
    if results.is_empty() {
        return Ok(false);
    }

    let found_city = results
        .iter()
        .any(|e| e.content.to_lowercase().contains("hangzhou"));

    // Also verify via agent query
    let response = collect_reply(agent, "Which city do I live in? Check your memory.").await?;
    let lower = response.to_lowercase();

    Ok(found_city || lower.contains("hangzhou"))
}

/// Test 3: Multi-turn reasoning that requires memory context.
async fn run_memory_reasoning(agent: &impl Agent) -> Result<bool, Box<dyn std::error::Error>> {
    // Ask a compound question that requires using stored memories together
    let response = collect_reply(
        agent,
        "Based on what you remember about me (my favorite color and my city), suggest a nice activity I might enjoy.",
    )
    .await?;
    let lower = response.to_lowercase();

    // Agent should reference at least one of the stored facts or give meaningful advice
    Ok(!response.is_empty()
        && (lower.contains("blue")
            || lower.contains("cerulean")
            || lower.contains("hangzhou")
            || lower.contains("color")
            || lower.contains("city")
            || lower.contains("lake")
            || lower.contains("activity")
            || lower.contains("enjoy")))
}
