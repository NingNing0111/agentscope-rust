//! Lexical path helpers shared across workspace-facing crates.

use std::path::{Component, Path, PathBuf};

/// Remove `.` and resolve `..` components lexically.
///
/// This does not touch the filesystem and therefore does not resolve symlinks.
#[must_use]
pub fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Return true when a path contains a parent-directory component.
#[must_use]
pub fn has_parent_component(path: impl AsRef<Path>) -> bool {
    path.as_ref()
        .components()
        .any(|component| matches!(component, Component::ParentDir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_lexical_removes_dot_and_parent_segments() {
        assert_eq!(
            normalize_lexical(Path::new("/ws/a/../b/./c")),
            PathBuf::from("/ws/b/c")
        );
    }

    #[test]
    fn has_parent_component_detects_traversal() {
        assert!(has_parent_component("../x"));
        assert!(has_parent_component("a/../x"));
        assert!(!has_parent_component("a/b"));
    }
}
