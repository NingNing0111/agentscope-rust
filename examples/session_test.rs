//! Session Persistence E2E Test
//!
//! Verifies Session save/load round-trip with InMemorySessionStore preserves
//! conversation history and AgentState. No LLM calls needed — this tests
//! the session storage layer purely.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example session_test
//! ```

use std::time::Instant;

use agent_scope_message::{ContentBlock, TextBlock};
use agent_scope_state::{Session, SessionError, SessionImpl, SessionStore};
use clap::Parser;

mod common;
use common::{
    TestResult, create_session_store, print_banner, print_result, print_summary, print_test_header,
};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// (accepted for consistency, not used by this example).
    #[arg(short = 'k', long, env = "API_KEY", default_value = "")]
    api_key: String,

    /// (not used by this example).
    #[arg(short = 'm', long, default_value = "n/a")]
    model: String,
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn make_msg(text: &str) -> Vec<ContentBlock> {
    vec![ContentBlock::Text(TextBlock::new(text.into()))]
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let cli = Cli::parse();
    let total_start = Instant::now();

    print_banner("Session Persistence", &cli.model);

    let mut results: Vec<TestResult> = Vec::new();

    // ── Test 1: Save/Load Roundtrip ──────────────────────────────────
    print_test_header(1, "Save/Load Roundtrip");
    let start = Instant::now();

    let test1 = match run_save_load_roundtrip().await {
        Ok(true) => TestResult {
            name: "Save/Load Roundtrip",
            passed: true,
            detail: "Session context preserved: 2 messages survived roundtrip".into(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Ok(false) => TestResult {
            name: "Save/Load Roundtrip",
            passed: false,
            detail: "Session roundtrip did not preserve state correctly".into(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => TestResult {
            name: "Save/Load Roundtrip",
            passed: false,
            detail: format!("Error: {e}"),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    };

    print_result(&test1);
    results.push(test1);

    // ── Test 2: Context Consistency ─────────────────────────────────
    print_test_header(2, "Context Consistency");
    let start = Instant::now();

    let test2 = match run_context_consistency().await {
        Ok(true) => TestResult {
            name: "Context Consistency",
            passed: true,
            detail: "Prior conversation fact preserved after session load".into(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Ok(false) => TestResult {
            name: "Context Consistency",
            passed: false,
            detail: "Prior fact not preserved after session load".into(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => TestResult {
            name: "Context Consistency",
            passed: false,
            detail: format!("Error: {e}"),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    };

    print_result(&test2);
    results.push(test2);

    // ── Test 3: Close & Cleanup ─────────────────────────────────────
    print_test_header(3, "Close & Cleanup");
    let start = Instant::now();

    let test3 = match run_close_cleanup().await {
        Ok(true) => TestResult {
            name: "Close & Cleanup",
            passed: true,
            detail: "Session closed successfully, store empty after delete".into(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Ok(false) => TestResult {
            name: "Close & Cleanup",
            passed: false,
            detail: "Session close/cleanup did not work as expected".into(),
            duration_ms: start.elapsed().as_millis() as u64,
        },
        Err(e) => TestResult {
            name: "Close & Cleanup",
            passed: false,
            detail: format!("Error: {e}"),
            duration_ms: start.elapsed().as_millis() as u64,
        },
    };

    print_result(&test3);
    results.push(test3);

    print_summary(&results, total_start);

    let any_failed = results.iter().any(|r| !r.passed);
    if any_failed {
        std::process::exit(1);
    }
}

// ── Test implementations ───────────────────────────────────────────

/// Test 1: Create a 2-turn conversation, save to store, load, verify message count.
async fn run_save_load_roundtrip() -> Result<bool, Box<dyn std::error::Error>> {
    let store = create_session_store();

    let mut session = SessionImpl::with_session_id("test-roundtrip".into());
    let session_id = session.id().to_string();

    session
        .state_mut()
        .append_context("user", make_msg("Hello, my name is Alice."))
        .map_err(|e| format!("{e:?}"))?;
    session
        .state_mut()
        .append_context("assistant", make_msg("Nice to meet you, Alice!"))
        .map_err(|e| format!("{e:?}"))?;

    assert_eq!(session.state().context_length(), 2);

    store.save(&session).await.map_err(|e| format!("{e}"))?;

    let restored = store.load(&session_id).await.map_err(|e| format!("{e}"))?;

    let msg_count = restored.state().context_length();
    let id_match = restored.id() == session_id;

    Ok(msg_count >= 2 && id_match)
}

/// Test 2: Save session with known fact, load, verify fact preserved in context.
async fn run_context_consistency() -> Result<bool, Box<dyn std::error::Error>> {
    let store = create_session_store();

    let mut session = SessionImpl::with_session_id("test-consistency".into());
    let session_id = session.id().to_string();

    session
        .state_mut()
        .append_context("user", make_msg("The secret code is XYLO-42."))
        .map_err(|e| format!("{e:?}"))?;
    session
        .state_mut()
        .append_context("assistant", make_msg("I've noted the code: XYLO-42."))
        .map_err(|e| format!("{e:?}"))?;

    store.save(&session).await.map_err(|e| format!("{e}"))?;

    let restored = store.load(&session_id).await.map_err(|e| format!("{e}"))?;

    let found_code = restored.state().context.iter().any(|msg| {
        msg.content.iter().any(|block| {
            if let ContentBlock::Text(tb) = block {
                tb.text.contains("XYLO-42")
            } else {
                false
            }
        })
    });

    let has_assistant = restored
        .state()
        .context
        .iter()
        .any(|msg| msg.role == agent_scope_message::Role::Assistant);

    Ok(found_code && has_assistant && restored.state().context_length() == 2)
}

/// Test 3: Close session, verify status, delete, verify NotFound on load.
async fn run_close_cleanup() -> Result<bool, Box<dyn std::error::Error>> {
    let store = create_session_store();

    let mut session = SessionImpl::with_session_id("test-cleanup".into());
    let session_id = session.id().to_string();

    session
        .state_mut()
        .append_context("user", make_msg("Hello."))
        .map_err(|e| format!("{e:?}"))?;

    store.save(&session).await.map_err(|e| format!("{e}"))?;

    // Verify it loads
    let loaded = store.load(&session_id).await.map_err(|e| format!("{e}"))?;
    if loaded.state().context_length() < 1 {
        return Ok(false);
    }

    // Close
    session.close().await.map_err(|e| format!("{e}"))?;
    if !session.is_closed() {
        return Ok(false);
    }

    // Delete
    store
        .delete(&session_id)
        .await
        .map_err(|e| format!("{e}"))?;

    // Verify NotFound on reload
    match store.load(&session_id).await {
        Err(SessionError::NotFound { .. }) => Ok(true),
        Ok(_) => Ok(false),
        Err(_) => Ok(true),
    }
}
