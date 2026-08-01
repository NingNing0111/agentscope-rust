//! Mount policy types.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{SandboxError, SandboxResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MountAccess {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MountOwner {
    Session,
    Workspace,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxMount {
    pub mount_id: String,
    pub host_path: PathBuf,
    pub sandbox_path: PathBuf,
    pub access: MountAccess,
    pub persist: bool,
    pub owner: MountOwner,
}

impl SandboxMount {
    pub fn validate(&mut self, root_dir: &Path) -> SandboxResult<()> {
        if self.mount_id.is_empty() {
            return Err(SandboxError::ValidationError {
                message: "mount_id must not be empty".into(),
            });
        }
        if self.sandbox_path.as_os_str().is_empty() {
            return Err(SandboxError::ValidationError {
                message: "sandbox_path must not be empty".into(),
            });
        }
        let target = if self.sandbox_path.is_absolute() {
            self.sandbox_path.clone()
        } else {
            root_dir.join("work").join(&self.sandbox_path)
        };
        if !target.starts_with(root_dir) {
            return Err(SandboxError::PermissionDenied {
                path: Some(self.sandbox_path.display().to_string()),
                operation: "mount".into(),
            });
        }
        if !self.sandbox_path.starts_with(root_dir) {
            self.sandbox_path = target;
        }
        Ok(())
    }
}

pub fn access_for_path<'a>(mounts: &'a [SandboxMount], path: &Path) -> Option<&'a SandboxMount> {
    mounts
        .iter()
        .filter(|m| path.starts_with(&m.sandbox_path))
        .max_by_key(|m| m.sandbox_path.components().count())
}
