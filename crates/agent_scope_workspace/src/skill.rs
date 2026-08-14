//! Skill types and SkillManager.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::backend::WorkspaceBackend;
use crate::error::WorkspaceError;

/// Agent-visible skill metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub dir: String,
    pub markdown: String,
    pub updated_at: f64,
}

/// Entry in the `.skills` index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    pub hash: String,
    pub skill_name: String,
}

/// `.skills` index file representation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillsIndex {
    pub skills_dir_mtime: f64,
    pub skills: HashMap<String, SkillEntry>,
}

/// Manages the skills/ directory and `.skills` index.
/// Thread-safe via Arc<Mutex<>> wrapper in LocalWorkspace.
pub struct SkillManager {
    skills_dir: String,
    backend: Arc<dyn WorkspaceBackend>,
    index: SkillsIndex,
}

impl fmt::Debug for SkillManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkillManager")
            .field("skills_dir", &self.skills_dir)
            .field("index", &self.index)
            .finish()
    }
}

impl SkillManager {
    #[must_use]
    pub fn new(skills_dir: String, backend: Arc<dyn WorkspaceBackend>) -> Self {
        Self {
            skills_dir,
            backend,
            index: SkillsIndex::default(),
        }
    }

    pub async fn load_index(&mut self) -> Result<(), WorkspaceError> {
        let index_path = self.backend.join_path(&self.skills_dir, ".skills");
        if self.backend.file_exists(&index_path).await? {
            let data = self.backend.read_file(&index_path).await?;
            let text = String::from_utf8_lossy(&data);
            if let Ok(idx) = serde_json::from_str::<SkillsIndex>(&text) {
                self.index = idx;
                return Ok(());
            }
        }
        self.index = SkillsIndex::default();
        Ok(())
    }

    pub async fn save_index(&self) -> Result<(), WorkspaceError> {
        let index_path = self.backend.join_path(&self.skills_dir, ".skills");
        let json = serde_json::to_string_pretty(&self.index).map_err(|e| {
            WorkspaceError::BackendError {
                message: format!("failed to serialize .skills: {e}"),
            }
        })?;
        self.backend.write_file(&index_path, json.as_bytes()).await
    }

    pub async fn reconcile(&mut self) -> Result<(), WorkspaceError> {
        let mtime = self
            .backend
            .stat_mtime(&self.skills_dir)
            .await?
            .unwrap_or(0.0);
        if (mtime - self.index.skills_dir_mtime).abs() < f64::EPSILON {
            return Ok(());
        }
        self.index.skills_dir_mtime = mtime;
        self.index.skills.clear();

        if self.backend.file_exists(&self.skills_dir).await? {
            let dirs = self.backend.list_dir(&self.skills_dir, false).await?;
            for dir_path in dirs {
                if !self.backend.is_dir(&dir_path).await? {
                    continue;
                }
                let skill_md_path = self.backend.join_path(&dir_path, "SKILL.md");
                if self.backend.file_exists(&skill_md_path).await? {
                    let data = self.backend.read_file(&skill_md_path).await?;
                    let skill_md = String::from_utf8_lossy(&data);
                    if let Ok((name, _desc, _body)) = parse_skill_md(&skill_md) {
                        let hash = hash_content(&skill_md);
                        let dir_name = self.backend.basename(&dir_path);
                        self.index.skills.insert(
                            dir_name,
                            SkillEntry {
                                hash,
                                skill_name: name,
                            },
                        );
                    }
                }
            }
        }
        self.save_index().await
    }

    #[allow(clippy::unwrap_used)]
    pub fn validate_skill(path: &str) -> Result<(String, String, String), WorkspaceError> {
        let p = std::path::Path::new(path).join("SKILL.md");
        if !p.exists() {
            return Err(WorkspaceError::InvalidSkill {
                path: path.to_string(),
                reason: "SKILL.md not found".into(),
            });
        }
        let content = std::fs::read_to_string(&p).map_err(|e| WorkspaceError::BackendError {
            message: format!("read SKILL.md '{:?}': {e}", p),
        })?;
        let (name, description, body) = parse_skill_md(&content)?;
        if name.is_empty() {
            return Err(WorkspaceError::InvalidSkill {
                path: path.to_string(),
                reason: "name is empty".into(),
            });
        }
        if description.is_empty() {
            return Err(WorkspaceError::InvalidSkill {
                path: path.to_string(),
                reason: "description is empty".into(),
            });
        }
        Ok((name, description, body))
    }

    pub fn hash_skill(path: &str) -> Result<String, WorkspaceError> {
        let p = std::path::Path::new(path).join("SKILL.md");
        let content = std::fs::read_to_string(&p).map_err(|e| WorkspaceError::BackendError {
            message: format!("hash_skill read '{:?}': {e}", p),
        })?;
        Ok(hash_content(&content))
    }

    pub async fn add_skill(&mut self, skill_path: &str) -> Result<(), WorkspaceError> {
        let (name, _description, _body) = Self::validate_skill(skill_path)?;
        let hash = Self::hash_skill(skill_path)?;

        for entry in self.index.skills.values() {
            if entry.hash == hash {
                tracing::info!("skill {name} already exists (hash match), skipping");
                return Ok(());
            }
        }

        let agent_name = resolve_name_conflict(&name, &self.index.skills);

        let src_dir_name = std::path::Path::new(skill_path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| name.clone());
        let dest_dir_name =
            resolve_dir_conflict(&src_dir_name, &self.skills_dir, &*self.backend).await?;
        let dest_dir = self.backend.join_path(&self.skills_dir, &dest_dir_name);

        // Canonicalize to prevent path traversal
        let canonical_dest = std::fs::canonicalize(&dest_dir)
            .unwrap_or_else(|_| std::path::PathBuf::from(&dest_dir));
        let canonical_skills = std::fs::canonicalize(&self.skills_dir)
            .unwrap_or_else(|_| std::path::PathBuf::from(&self.skills_dir));
        if !canonical_dest.starts_with(&canonical_skills) {
            return Err(WorkspaceError::PathTraversal { path: dest_dir });
        }

        copy_dir_recursive(skill_path, &dest_dir)
            .await
            .map_err(|e| WorkspaceError::BackendError {
                message: format!("failed to copy skill '{skill_path}': {e}"),
            })?;

        self.index.skills.insert(
            dest_dir_name,
            SkillEntry {
                hash,
                skill_name: agent_name,
            },
        );
        self.save_index().await
    }

    pub async fn remove_skill(&mut self, name: &str) -> Result<(), WorkspaceError> {
        let mut found_dir: Option<String> = None;
        for (dir_name, entry) in &self.index.skills {
            if entry.skill_name == name {
                found_dir = Some(dir_name.clone());
                break;
            }
        }
        let dir_name = match found_dir {
            Some(d) => d,
            None => {
                tracing::warn!("skill not found: {name}");
                return Ok(());
            }
        };
        let dir_path = self.backend.join_path(&self.skills_dir, &dir_name);
        self.backend.delete_path(&dir_path).await?;
        self.index.skills.remove(&dir_name);
        self.save_index().await
    }

    pub async fn list_skills(&mut self) -> Result<Vec<Skill>, WorkspaceError> {
        self.reconcile().await?;
        let mut skills = Vec::new();
        for (dir_name, entry) in &self.index.skills {
            let dir_path = self.backend.join_path(&self.skills_dir, dir_name);
            let skill_md_path = self.backend.join_path(&dir_path, "SKILL.md");
            if self.backend.file_exists(&skill_md_path).await? {
                let data = self.backend.read_file(&skill_md_path).await?;
                let content = String::from_utf8_lossy(&data).to_string();
                if let Ok((name, description, markdown)) = parse_skill_md(&content) {
                    let mtime = self
                        .backend
                        .stat_mtime(&skill_md_path)
                        .await?
                        .unwrap_or(0.0);
                    skills.push(Skill {
                        name,
                        description,
                        dir: dir_path,
                        markdown,
                        updated_at: mtime,
                    });
                    continue;
                }
            }
            skills.push(Skill {
                name: entry.skill_name.clone(),
                description: String::new(),
                dir: dir_path,
                markdown: String::new(),
                updated_at: 0.0,
            });
        }
        Ok(skills)
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

pub(crate) fn parse_skill_md(content: &str) -> Result<(String, String, String), WorkspaceError> {
    let parsed = agent_scope_utils::frontmatter::parse_skill_frontmatter(content);
    Ok((parsed.name, parsed.description, parsed.body))
}

pub(crate) fn hash_content(content: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn resolve_name_conflict(name: &str, index: &HashMap<String, SkillEntry>) -> String {
    let mut candidate = name.to_string();
    let mut counter = 1;
    while index.values().any(|e| e.skill_name == candidate) {
        candidate = format!("{name} ({counter})");
        counter += 1;
    }
    candidate
}

async fn resolve_dir_conflict(
    dir_name: &str,
    skills_dir: &str,
    backend: &dyn WorkspaceBackend,
) -> Result<String, WorkspaceError> {
    let mut candidate = dir_name.to_string();
    let mut counter = 1;
    while backend
        .file_exists(&backend.join_path(skills_dir, &candidate))
        .await?
    {
        candidate = format!("{dir_name}_{counter}");
        counter += 1;
    }
    Ok(candidate)
}

async fn copy_dir_recursive(src: &str, dst: &str) -> Result<(), std::io::Error> {
    tokio::fs::create_dir_all(dst).await?;
    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let file_type = entry.file_type().await?;
        let src_path = entry.path();
        let file_name = entry.file_name();
        let dst_path = std::path::Path::new(dst).join(&file_name);
        if file_type.is_symlink() {
            // A symlink inside a skill directory can point anywhere on the
            // host; `tokio::fs::copy` would follow it and copy the target's
            // contents (e.g. a private key) into the workspace. Skip symlinks
            // rather than dereference them (round-4 M30).
            tracing::warn!(
                path = %src_path.display(),
                "skipping symlink in skill directory during copy"
            );
            continue;
        }
        if file_type.is_dir() {
            Box::pin(copy_dir_recursive(
                &src_path.to_string_lossy(),
                &dst_path.to_string_lossy(),
            ))
            .await?;
        } else {
            tokio::fs::copy(&src_path, &dst_path).await?;
        }
    }
    Ok(())
}
