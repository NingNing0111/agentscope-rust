//! WorkspaceToolSession — per-agent tool-session state shared by the
//! workspace built-in tools.
//!
//! Carries two concerns:
//! 1. **Read-state**: the set of workspace paths successfully read during
//!    the current tool session. `Read` records reads; `Edit` and `Write`
//!    require membership before modifying an existing file
//!    (read-before-modify guard, FR-008/FR-012).
//! 2. **Activation groups**: the tool groups currently active within the
//!    authorization boundary, managed by `ResetTools` (FR-019).
//!
//! The authority for activation state is `AgentState.tool_context
//! .activated_groups` (persisted with the session); this struct holds a
//! synchronous mirror so the tool layer (`agent_scope_tool`, which must not
//! depend on `agent_scope_agent`/`agent_scope_state`) can enforce guards
//! without a cross-crate dependency. The agent layer keeps the two in sync.

use std::collections::BTreeSet;
use std::path::Path;

/// Upper bound on the number of tracked read paths, so a pathological agent
/// cannot grow the set without limit.
const MAX_READ_PATHS: usize = 4096;

/// Per-agent tool-session state shared by workspace built-in tools.
///
/// Not `Clone` by design — it is a mutable per-session store, not a value
/// type. Share it behind `Arc<RwLock<_>>` (or `Mutex`) between tools.
#[derive(Debug, Default)]
pub struct WorkspaceToolSession {
    /// Workspace this state belongs to (for isolation diagnostics).
    workspace_id: String,
    /// Normalized, workspace-contained paths successfully read this session.
    read_files: BTreeSet<String>,
    /// Currently active tool groups (managed by `ResetTools`).
    active_tool_groups: BTreeSet<String>,
    /// Paths that are authorized for activation by `ResetTools`.
    authorized_tool_groups: BTreeSet<String>,
}

impl WorkspaceToolSession {
    /// Create a new empty session bound to `workspace_id`.
    #[must_use]
    pub fn new(workspace_id: impl Into<String>) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            ..Self::default()
        }
    }

    /// Create a new session with a pre-set authorized group set.
    #[must_use]
    pub fn with_authorized_groups<I, S>(
        workspace_id: impl Into<String>,
        authorized_tool_groups: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            workspace_id: workspace_id.into(),
            authorized_tool_groups: authorized_tool_groups.into_iter().map(Into::into).collect(),
            ..Self::default()
        }
    }

    /// The workspace this session is bound to.
    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    // ── read-state ──

    /// Record a successful read of `path`. The path should be normalized and
    /// workspace-contained before insertion. Returns `false` if the read set
    /// is full (path not recorded).
    pub fn record_read(&mut self, path: &Path) -> bool {
        let key = path.to_string_lossy().to_string();
        if self.read_files.len() >= MAX_READ_PATHS {
            tracing::warn!(
                path = %key,
                "read-state set full ({}), refusing to record",
                MAX_READ_PATHS
            );
            return false;
        }
        self.read_files.insert(key)
    }

    /// Whether `path` was successfully read earlier in this session.
    pub fn is_read(&self, path: &Path) -> bool {
        let key = path.to_string_lossy().to_string();
        self.read_files.contains(&key)
    }

    /// Number of paths currently in the read set.
    pub fn read_count(&self) -> usize {
        self.read_files.len()
    }

    /// All currently-read paths.
    pub fn read_paths(&self) -> impl Iterator<Item = &String> {
        self.read_files.iter()
    }

    /// Clear the read set (e.g. on session reset).
    pub fn clear_reads(&mut self) {
        self.read_files.clear();
    }

    // ── activation groups ──

    /// Replace the active group set with `groups`, intersecting with the
    /// authorized boundary. Returns the groups actually activated.
    pub fn record_groups<I, S>(&mut self, groups: I) -> Vec<String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.active_tool_groups.clear();
        let mut activated = Vec::new();
        for group in groups {
            let group = group.into();
            // `basic` is always active by convention; nothing to record.
            if group == "basic" {
                continue;
            }
            // FR-019: never expand beyond the authorized boundary.
            if self.authorized_tool_groups.is_empty()
                || self.authorized_tool_groups.contains(&group)
            {
                self.active_tool_groups.insert(group.clone());
                activated.push(group);
            } else {
                tracing::warn!(
                    group = %group,
                    "ResetTools requested group outside authorization boundary, ignoring"
                );
            }
        }
        activated
    }

    /// Whether the given tool group is currently active.
    pub fn is_group_active(&self, name: &str) -> bool {
        name == "basic" || self.active_tool_groups.contains(name)
    }

    /// Current active groups (excluding the implicit `basic`).
    pub fn list_groups(&self) -> impl Iterator<Item = &String> {
        self.active_tool_groups.iter()
    }

    /// Tool groups authorized for activation by `ResetTools` (excluding the
    /// implicit `basic`). Empty means no boundary is imposed.
    pub fn authorized_groups(&self) -> impl Iterator<Item = &String> {
        self.authorized_tool_groups.iter()
    }

    /// Number of tool groups authorized for activation.
    pub fn authorized_groups_count(&self) -> usize {
        self.authorized_tool_groups.len()
    }

    /// Current active groups including the implicit `basic`.
    pub fn all_active_groups(&self) -> Vec<String> {
        let mut groups = vec!["basic".to_string()];
        groups.extend(self.active_tool_groups.iter().cloned());
        groups
    }

    /// Reset activation to the default (empty — `basic` only).
    pub fn reset_groups(&mut self) {
        self.active_tool_groups.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws() -> WorkspaceToolSession {
        WorkspaceToolSession::with_authorized_groups("ws-1", ["coding", "docs"])
    }

    #[test]
    fn read_state_records_and_checks() {
        let mut session = ws();
        assert!(!session.is_read(Path::new("/ws/a.txt")));
        assert!(session.record_read(Path::new("/ws/a.txt")));
        assert!(session.is_read(Path::new("/ws/a.txt")));
        assert_eq!(session.read_count(), 1);
        session.clear_reads();
        assert!(!session.is_read(Path::new("/ws/a.txt")));
    }

    #[test]
    fn record_groups_intersects_authorized() {
        let mut session = ws();
        let activated = session.record_groups(["coding", "admin"]);
        // "admin" is not in the authorized boundary → ignored.
        assert_eq!(activated, vec!["coding".to_string()]);
        assert!(session.is_group_active("coding"));
        assert!(!session.is_group_active("admin"));
    }

    #[test]
    fn record_groups_is_final_state() {
        let mut session = ws();
        session.record_groups(["coding"]);
        assert!(session.is_group_active("coding"));
        // Final-state semantics: re-record without "coding" deactivates it.
        session.record_groups(["docs"]);
        assert!(!session.is_group_active("coding"));
        assert!(session.is_group_active("docs"));
    }

    #[test]
    fn basic_group_always_active() {
        let mut session = ws();
        session.reset_groups();
        assert!(session.is_group_active("basic"));
        assert_eq!(session.all_active_groups(), vec!["basic".to_string()]);
    }
}
