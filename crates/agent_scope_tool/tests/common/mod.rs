//! Shared helpers for `agent_scope_tool` integration tests (T004).
//!
//! Each test builds a temporary workspace and a [`BuiltInToolContext`] bound to
//! it, so tools can be exercised against real files.
//!
//! Each integration-test binary includes this module via `mod common;` and only
//! uses a subset of the helpers, so `dead_code` is expected.

#![allow(dead_code)]

use std::sync::{Arc, RwLock};

use agent_scope_message::{ToolOutput, ToolResultBlock};
use agent_scope_tool::builtin::{BuiltInToolContext, WorkspaceToolSession};
use agent_scope_workspace::backend::{LocalBackend, WorkspaceBackend};

/// A test harness: an owned tempdir plus the shared tool context + session.
pub struct TestContext {
    /// Tempdir; kept alive for the lifetime of the context.
    pub _dir: tempfile::TempDir,
    /// Workspace root path.
    pub workdir: String,
    /// Shared tool execution context.
    pub ctx: BuiltInToolContext,
    /// Shared read-state / activation session.
    pub session: Arc<RwLock<WorkspaceToolSession>>,
}

/// Build a context rooted at a fresh temp directory.
pub fn ctx_in(authorized_groups: &[&str]) -> TestContext {
    let dir = tempfile::tempdir().expect("tempdir");
    let workdir = dir.path().to_string_lossy().to_string();
    let backend: Arc<dyn WorkspaceBackend> = Arc::new(LocalBackend::new());
    let session = Arc::new(RwLock::new(WorkspaceToolSession::with_authorized_groups(
        "ws-test",
        authorized_groups.iter().map(|s| s.to_string()),
    )));
    let ctx = BuiltInToolContext::new(backend, workdir.clone(), Arc::clone(&session));
    TestContext {
        _dir: dir,
        workdir,
        ctx,
        session,
    }
}

/// Build a context with no authorized-group boundary.
pub fn ctx_plain() -> TestContext {
    ctx_in(&[])
}

/// Write a file into the workspace, returning its absolute path.
pub fn write_ws_file(harness: &TestContext, rel: &str, content: &str) -> String {
    let path = std::path::Path::new(&harness.workdir).join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    path.to_string_lossy().to_string()
}

/// Extract the text payload of a tool result block.
pub fn text_of(block: &ToolResultBlock) -> String {
    match &block.output {
        ToolOutput::Text(t) => t.clone(),
        _ => String::new(),
    }
}

/// Mark a workspace file as read in the session (bypasses the guard for tests
/// that only exercise the mutation behaviour).
pub fn mark_read(harness: &TestContext, path: &str) {
    harness
        .session
        .write()
        .unwrap()
        .record_read(std::path::Path::new(path));
}
