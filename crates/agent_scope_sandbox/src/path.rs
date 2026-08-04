//! Path canonicalization and containment checks.

use std::path::{Component, Path, PathBuf};

use crate::error::{SandboxError, SandboxResult};

#[derive(Debug, Clone)]
pub struct SandboxPathResolver {
    root_dir: PathBuf,
    workdir: PathBuf,
}

impl SandboxPathResolver {
    pub fn new(root_dir: PathBuf, workdir: PathBuf) -> SandboxResult<Self> {
        let root_dir = root_dir.canonicalize().map_err(|e| SandboxError::IoError {
            operation: "canonicalize_root".into(),
            message: e.to_string(),
        })?;
        let workdir = if workdir.is_absolute() {
            workdir
        } else {
            root_dir.join(workdir)
        };
        std::fs::create_dir_all(&workdir).map_err(|e| SandboxError::IoError {
            operation: "create_workdir".into(),
            message: e.to_string(),
        })?;
        let workdir = workdir.canonicalize().map_err(|e| SandboxError::IoError {
            operation: "canonicalize_workdir".into(),
            message: e.to_string(),
        })?;
        if !workdir.starts_with(&root_dir) {
            return Err(SandboxError::PermissionDenied {
                path: Some(workdir.display().to_string()),
                operation: "workdir".into(),
            });
        }
        Ok(Self { root_dir, workdir })
    }

    #[must_use]
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }
    #[must_use]
    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    pub fn resolve(
        &self,
        path: &str,
        cwd: Option<&Path>,
        must_exist: bool,
        operation: &str,
    ) -> SandboxResult<PathBuf> {
        if path.is_empty() {
            return Err(SandboxError::ValidationError {
                message: "path must not be empty".into(),
            });
        }
        let input = Path::new(path);
        if has_parent_component(input) {
            return Err(SandboxError::PermissionDenied {
                path: Some(path.to_string()),
                operation: operation.into(),
            });
        }
        let joined = if input.is_absolute() {
            self.root_dir.join(strip_absolute(input))
        } else {
            cwd.unwrap_or(&self.workdir).join(input)
        };
        self.resolve_pathbuf(&joined, must_exist, operation)
    }

    pub fn resolve_pathbuf(
        &self,
        path: &Path,
        must_exist: bool,
        operation: &str,
    ) -> SandboxResult<PathBuf> {
        if has_parent_component(path) && !path.is_absolute() {
            return Err(SandboxError::PermissionDenied {
                path: Some(path.display().to_string()),
                operation: operation.into(),
            });
        }
        if must_exist {
            let canon = path.canonicalize().map_err(|e| SandboxError::IoError {
                operation: operation.into(),
                message: e.to_string(),
            })?;
            self.ensure_contained(canon, operation)
        } else {
            let parent = path.parent().ok_or_else(|| SandboxError::ValidationError {
                message: "path has no parent".into(),
            })?;
            // Verify the parent chain BEFORE creating anything: a pre-planted
            // symlink in the chain would otherwise make `create_dir_all` follow
            // it and create directories outside the sandbox root as a side
            // effect, even though the eventual file write is rejected.
            let mut existing = if parent.as_os_str().is_empty() {
                std::path::Path::new(".").to_path_buf()
            } else {
                parent.to_path_buf()
            };
            let mut missing: Vec<std::ffi::OsString> = Vec::new();
            while !existing.exists() {
                let Some(name) = existing.file_name() else { break };
                let Some(up) = existing.parent() else { break };
                missing.push(name.to_os_string());
                existing = up.to_path_buf();
            }
            let mut current = existing
                .canonicalize()
                .map_err(|e| SandboxError::IoError {
                    operation: format!("{operation}_resolve_parent"),
                    message: e.to_string(),
                })?;
            self.ensure_contained(current.clone(), operation)?;
            // Descend through the components that do not exist yet; any that
            // already exist (e.g. planted by a concurrent writer, or a symlink)
            // must be resolved and stay contained.
            for comp in missing.iter().rev() {
                current = current.join(comp);
                if let Ok(meta) = std::fs::symlink_metadata(&current)
                    && meta.file_type().is_symlink()
                {
                    let canon =
                        current.canonicalize().map_err(|e| SandboxError::IoError {
                            operation: operation.into(),
                            message: e.to_string(),
                        })?;
                    self.ensure_contained(canon, operation)?;
                }
            }
            std::fs::create_dir_all(parent).map_err(|e| SandboxError::IoError {
                operation: format!("{operation}_create_parent"),
                message: e.to_string(),
            })?;
            let parent_canon = parent.canonicalize().map_err(|e| SandboxError::IoError {
                operation: operation.into(),
                message: e.to_string(),
            })?;
            let safe_parent = self.ensure_contained(parent_canon, operation)?;
            let file_name = path
                .file_name()
                .ok_or_else(|| SandboxError::ValidationError {
                    message: "path has no leaf".into(),
                })?;
            let candidate = safe_parent.join(file_name);
            // If the leaf already exists as a symlink, resolve and re-check
            // containment so a pre-planted symlink (e.g. `ln -s /etc/passwd
            // workdir/evil.txt` then write) cannot escape the sandbox root.
            match std::fs::symlink_metadata(&candidate) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    let canon = candidate.canonicalize().map_err(|e| {
                        SandboxError::IoError {
                            operation: operation.into(),
                            message: e.to_string(),
                        }
                    })?;
                    self.ensure_contained(canon, operation)
                }
                _ => Ok(candidate),
            }
        }
    }

    fn ensure_contained(&self, canon: PathBuf, operation: &str) -> SandboxResult<PathBuf> {
        if canon.starts_with(&self.root_dir) {
            Ok(canon)
        } else {
            Err(SandboxError::PermissionDenied {
                path: Some(canon.display().to_string()),
                operation: operation.into(),
            })
        }
    }
}

fn strip_absolute(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::RootDir | Component::Prefix(_) => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn has_parent_component(path: &Path) -> bool {
    path.components().any(|c| matches!(c, Component::ParentDir))
}
