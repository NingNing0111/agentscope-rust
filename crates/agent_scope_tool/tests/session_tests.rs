//! WorkspaceToolSession — read-state + activation-group integration tests (T010).
//!
//! The core logic is unit-tested inline in `builtin/session.rs`; this file
//! exercises the public surface from the integration boundary.

mod common;

use common::{ctx_in, write_ws_file};

#[test]
fn session_read_state_normalization() {
    let h = ctx_in(&[]);
    let file = write_ws_file(&h, "a.txt", "hello\n");
    let session = &h.session;
    let mut guard = session.write().unwrap();

    assert!(!guard.is_read(std::path::Path::new(&file)));
    assert!(guard.record_read(std::path::Path::new(&file)));
    assert!(guard.is_read(std::path::Path::new(&file)));
    assert_eq!(guard.read_count(), 1);

    guard.clear_reads();
    assert!(!guard.is_read(std::path::Path::new(&file)));
}

#[test]
fn session_activation_final_state() {
    let h = ctx_in(&["coding", "docs"]);
    let session = &h.session;

    {
        let mut guard = session.write().unwrap();
        guard.record_groups(["coding"]);
        assert!(guard.is_group_active("coding"));
        assert!(!guard.is_group_active("docs"));
        // Final state: re-record deactivates coding.
        guard.record_groups(["docs"]);
        assert!(!guard.is_group_active("coding"));
        assert!(guard.is_group_active("docs"));
    }

    // Workspace isolation: another workspace's session is independent.
    let other = ctx_in(&["coding"]);
    assert!(!other.session.read().unwrap().is_group_active("docs"));
}

#[test]
fn session_unauthorized_group_ignored() {
    let h = ctx_in(&["coding"]);
    let mut guard = h.session.write().unwrap();
    let activated = guard.record_groups(["admin", "coding"]);
    // "admin" is outside the authorized boundary → ignored.
    assert_eq!(activated, vec!["coding".to_string()]);
    assert!(guard.is_group_active("coding"));
    assert!(!guard.is_group_active("admin"));
}

#[test]
fn session_basic_group_always_active() {
    let h = ctx_in(&["coding"]);
    let guard = h.session.read().unwrap();
    assert!(guard.is_group_active("basic"));
    assert_eq!(guard.all_active_groups(), vec!["basic".to_string()]);
}
