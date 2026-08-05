//! Tests for the confirmation loop (`run_confirmation_loop`) and the y/n
//! response parser, using injected fake turns so no real agent is needed.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use pi_rust::render::{ConfirmationCandidate, RenderedTurn};
use pi_rust::repl::{parse_confirmation_response, run_confirmation_loop};

fn candidate(tool: &str, fingerprint: &str, description: &str) -> ConfirmationCandidate {
    ConfirmationCandidate {
        tool_name: tool.into(),
        fingerprint: fingerprint.into(),
        description: description.into(),
    }
}

fn turn_with(candidates: Vec<ConfirmationCandidate>) -> RenderedTurn {
    RenderedTurn {
        confirmation_candidates: candidates,
        ..Default::default()
    }
}

#[tokio::test]
async fn loop_grants_and_populates_approvals() {
    let approvals = Arc::new(Mutex::new(HashSet::new()));
    let first = turn_with(vec![candidate("Bash", "bash:rm x", "[Bash] $ rm x")]);
    let attempts = Arc::new(AtomicU32::new(0));
    let attempts_clone = Arc::clone(&attempts);
    let result = run_confirmation_loop(
        &approvals,
        first,
        move || {
            let counter = Arc::clone(&attempts_clone);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                RenderedTurn::default() // retry has no pending candidates
            }
        },
        |_| async { vec![true] },
    )
    .await;
    assert!(approvals.lock().unwrap().contains("bash:rm x"));
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        1,
        "expected exactly one retry"
    );
    assert!(result.confirmation_candidates.is_empty());
}

#[tokio::test]
async fn loop_stops_on_user_deny() {
    let approvals = Arc::new(Mutex::new(HashSet::new()));
    let first = turn_with(vec![candidate("Bash", "bash:rm x", "[Bash] $ rm x")]);
    let attempts = Arc::new(AtomicU32::new(0));
    let attempts_clone = Arc::clone(&attempts);
    let result = run_confirmation_loop(
        &approvals,
        first,
        move || {
            let counter = Arc::clone(&attempts_clone);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                RenderedTurn::default()
            }
        },
        |_| async { vec![false] },
    )
    .await;
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        0,
        "must not retry when nothing is approved"
    );
    assert!(approvals.lock().unwrap().is_empty());
    assert_eq!(
        result.confirmation_candidates.len(),
        1,
        "denied candidate remains visible"
    );
}

#[tokio::test]
async fn loop_does_not_reask_denied_fingerprint() {
    let approvals = Arc::new(Mutex::new(HashSet::new()));
    let first = turn_with(vec![
        candidate("Bash", "bash:rm x", "[Bash] $ rm x"),
        candidate("Bash", "bash:rm y", "[Bash] $ rm y"),
    ]);
    let attempts = Arc::new(AtomicU32::new(0));
    let attempts_clone = Arc::clone(&attempts);
    let mut ask_calls = 0u32;
    let result = run_confirmation_loop(
        &approvals,
        first,
        move || {
            let counter = Arc::clone(&attempts_clone);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                // The retry re-issues only the fingerprint the user denied.
                turn_with(vec![candidate("Bash", "bash:rm x", "[Bash] $ rm x")])
            }
        },
        |_| {
            ask_calls += 1;
            async { vec![false, true] }
        },
    )
    .await;
    assert_eq!(ask_calls, 1, "denied fingerprint must not be re-asked");
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    assert!(approvals.lock().unwrap().contains("bash:rm y"));
    assert!(!approvals.lock().unwrap().contains("bash:rm x"));
    assert!(result.tool_lines.is_empty());
}

#[tokio::test]
async fn loop_hits_max_retries() {
    let approvals = Arc::new(Mutex::new(HashSet::new()));
    let first = turn_with(vec![candidate("Bash", "bash:rm n0", "[Bash] $ rm n0")]);
    let attempts = Arc::new(AtomicU32::new(0));
    let attempts_clone = Arc::clone(&attempts);
    run_confirmation_loop(
        &approvals,
        first,
        move || {
            let counter = Arc::clone(&attempts_clone);
            async move {
                let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
                // Every retry produces a *fresh* un-approved fingerprint, so the
                // convergence guard cannot stop the loop early.
                turn_with(vec![candidate("Bash", &format!("bash:rm n{n}"), "x")])
            }
        },
        |_| async { vec![true] },
    )
    .await;
    // MAX_CONFIRMATION_RETRIES == 3.
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        3,
        "expected exactly the retry cap"
    );
}

#[tokio::test]
async fn loop_skips_ask_after_interrupt() {
    let approvals = Arc::new(Mutex::new(HashSet::new()));
    let mut first = turn_with(vec![candidate("Bash", "bash:rm x", "[Bash] $ rm x")]);
    first.interrupted = true;
    let mut ask_calls = 0u32;
    let result = run_confirmation_loop(
        &approvals,
        first,
        || async { RenderedTurn::default() },
        |_| {
            ask_calls += 1;
            async { vec![true] }
        },
    )
    .await;
    assert_eq!(ask_calls, 0, "must not ask after an interrupt");
    assert!(approvals.lock().unwrap().is_empty());
    assert!(result.interrupted);
}

#[tokio::test]
async fn loop_concats_events_and_keeps_last_text() {
    let approvals = Arc::new(Mutex::new(HashSet::new()));
    let mut first = turn_with(vec![candidate("Bash", "bash:rm x", "[Bash] $ rm x")]);
    first.text = "first reply".into();
    first.events.push(agent_scope_event::AgentEvent::ReplyStart(
        agent_scope_event::ReplyStartEvent {
            base: agent_scope_event::EventBase::new(),
            session_id: "s1".into(),
            reply_id: "r1".into(),
            name: "test".into(),
            role: "assistant".into(),
        },
    ));
    let result = run_confirmation_loop(
        &approvals,
        first,
        || async {
            RenderedTurn {
                text: "second reply".into(),
                ..Default::default()
            }
        },
        |_| async { vec![true] },
    )
    .await;
    assert_eq!(result.text, "second reply");
    assert!(
        !result.events.is_empty(),
        "first-turn events must be merged"
    );
}

#[test]
fn parse_confirmation_response_accepts_yes_forms() {
    assert!(parse_confirmation_response("y"));
    assert!(parse_confirmation_response("Y"));
    assert!(parse_confirmation_response("yes"));
    assert!(parse_confirmation_response("  y  "));
    assert!(!parse_confirmation_response("n"));
    assert!(!parse_confirmation_response(""));
    assert!(!parse_confirmation_response("maybe"));
}
