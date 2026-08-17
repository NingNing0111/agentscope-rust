//! LocalWorkspace — filesystem-based workspace implementation.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::backend::{ContainedBackend, LocalBackend, WorkspaceBackend};
use crate::base::{McpConnectionHandle, McpConnectionsHost, ToolInfo, WorkspaceBase};
use crate::error::WorkspaceError;
use crate::instructions::DEFAULT_WORKSPACE_INSTRUCTIONS;
use crate::mcp::{McpClientConfig, McpRegistry};
use crate::skill::{Skill, SkillManager};

/// Configuration for creating a LocalWorkspace.
#[derive(Debug, Clone)]
pub struct LocalWorkspaceConfig {
    pub workdir: String,
    pub workspace_id: Option<String>,
    pub default_mcps: Vec<McpClientConfig>,
    pub skill_paths: Vec<String>,
    pub instructions: Option<String>,
}

/// Filesystem-based workspace for isolated agent execution.
pub struct LocalWorkspace {
    workdir: String,
    workspace_id: String,
    is_alive: bool,
    instructions: String,
    default_mcps: Vec<McpClientConfig>,
    skill_paths: Vec<String>,
    _backend: Arc<dyn WorkspaceBackend>,
    _mcps: Vec<McpClientConfig>,
    _skill_mgr: Arc<Mutex<SkillManager>>,
    _mcp_connections: Arc<Mutex<HashMap<String, Arc<dyn McpConnectionHandle>>>>,
    _mcp_lock: Mutex<()>,
    _skill_lock: Mutex<()>,
}

impl LocalWorkspace {
    /// Disconnect and drop all active MCP connections (FR-010).
    async fn disconnect_all_mcps(&self) -> Result<(), WorkspaceError> {
        let mut conns = self._mcp_connections.lock().await;
        let mut last_err = Ok(());
        for (name, handle) in conns.drain() {
            if let Err(e) = handle.disconnect().await {
                tracing::warn!("failed to disconnect MCP '{name}': {e}");
                last_err = Err(e);
            }
        }
        last_err
    }

    #[must_use]
    pub fn new(config: LocalWorkspaceConfig) -> Self {
        // Resolve the workdir to a canonicalized absolute path. When the
        // directory does not exist yet (typical first run), create it first so
        // canonicalize() succeeds. This keeps the backend containment root in
        // canonical form even when the workdir sits under a symlinked parent
        // (e.g. macOS `/tmp` → `/private/tmp`) — otherwise the containment
        // check compares a canonicalized ancestor against the un-canonicalized
        // root and spuriously reports PathTraversal.
        let workdir_path = std::path::Path::new(&config.workdir);
        if !workdir_path.exists() {
            let _ = std::fs::create_dir_all(workdir_path);
        }
        // Canonicalize once: the result serves both as the containment root
        // (kept verbatim, including Windows' `\\?\` extended-length prefix) and
        // as the display workdir. Windows `canonicalize()` prefixes the path
        // with `\\?\`; that prefix would leak into agent-facing strings (e.g.
        // `get_instructions`) and break exact matches against the un-prefixed
        // config path, so strip it from the display form only.
        let workdir_root = workdir_path
            .canonicalize()
            .unwrap_or_else(|_| workdir_path.to_path_buf());
        let workdir = {
            let display = workdir_root.to_string_lossy();
            #[cfg(windows)]
            {
                display
                    .strip_prefix("\\\\?\\")
                    .unwrap_or(&display)
                    .to_string()
            }
            #[cfg(not(windows))]
            {
                display.into_owned()
            }
        };

        let workspace_id = config
            .workspace_id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let instructions = config
            .instructions
            .unwrap_or_else(|| DEFAULT_WORKSPACE_INSTRUCTIONS.to_string());

        // Wrap LocalBackend in ContainedBackend to enforce workdir containment
        // (defect 1 fix). All path operations through get_backend() are now
        // automatically confined to the workspace workdir.
        let raw_backend: Arc<dyn WorkspaceBackend> = Arc::new(LocalBackend::new());
        let backend: Arc<dyn WorkspaceBackend> = Arc::new(ContainedBackend::new(
            Arc::clone(&raw_backend),
            workdir_root,
        ));

        let skills_dir = backend.join_path(&workdir, "skills");
        let skill_mgr = SkillManager::new(skills_dir, Arc::clone(&backend));

        Self {
            workdir,
            workspace_id,
            is_alive: false,
            instructions,
            default_mcps: config.default_mcps,
            skill_paths: config.skill_paths,
            _backend: backend,
            _mcps: Vec::new(),
            _skill_mgr: Arc::new(Mutex::new(skill_mgr)),
            _mcp_connections: Arc::new(Mutex::new(HashMap::new())),
            _mcp_lock: Mutex::new(()),
            _skill_lock: Mutex::new(()),
        }
    }

    fn mcp_path(&self) -> String {
        self._backend.join_path(&self.workdir, ".mcp")
    }

    fn skills_dir(&self) -> String {
        self._backend.join_path(&self.workdir, "skills")
    }

    fn sessions_dir(&self) -> String {
        self._backend.join_path(&self.workdir, "sessions")
    }

    fn data_dir(&self) -> String {
        self._backend.join_path(&self.workdir, "data")
    }
}

#[async_trait::async_trait]
#[allow(unused)]
impl WorkspaceBase for LocalWorkspace {
    async fn initialize(&mut self) -> Result<(), WorkspaceError> {
        if self.is_alive {
            return Ok(());
        }

        if !self._backend.file_exists(&self.workdir).await? {
            self._backend
                .write_file(&self._backend.join_path(&self.workdir, ".keep"), b"")
                .await?;
        }
        for dir in [self.data_dir(), self.skills_dir(), self.sessions_dir()] {
            if !self._backend.is_dir(&dir).await? {
                self._backend
                    .write_file(&self._backend.join_path(&dir, ".keep"), b"")
                    .await?;
            }
        }

        let mcp_path = self.mcp_path();
        if self._backend.file_exists(&mcp_path).await? {
            match McpRegistry::load(&*self._backend, &mcp_path).await {
                Ok(configs) => {
                    self._mcps = configs;
                }
                Err(e) => {
                    tracing::warn!("corrupt .mcp ({e}), using defaults");
                    self._mcps = self.default_mcps.clone();
                    McpRegistry::save(&self._mcps, &*self._backend, &mcp_path).await?;
                }
            }
        } else {
            self._mcps = self.default_mcps.clone();
            McpRegistry::save(&self._mcps, &*self._backend, &mcp_path).await?;
        }

        // Seed skills
        {
            let mut skill_mgr = self._skill_mgr.lock().await;
            skill_mgr.load_index().await?;
            skill_mgr.reconcile().await?;
            for skill_path in &self.skill_paths {
                if let Err(e) = skill_mgr.add_skill(skill_path).await {
                    tracing::warn!("failed to seed skill '{skill_path}': {e}");
                }
            }
        }

        self.is_alive = true;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), WorkspaceError> {
        if !self.is_alive {
            return Ok(());
        }
        // Release all stateful MCP connections (FR-010).
        self.disconnect_all_mcps().await?;
        self._mcps.clear();
        self.is_alive = false;
        Ok(())
    }

    async fn reset(&mut self) -> Result<(), WorkspaceError> {
        // Release all stateful MCP connections (FR-010).
        self.disconnect_all_mcps().await?;
        self._mcps.clear();

        let _lock = self._mcp_lock.lock().await;
        let _skill_lock = self._skill_lock.lock().await;

        let mcp_path = self.mcp_path();
        self._backend.delete_path(&mcp_path).await?;

        for dir in [self.skills_dir(), self.sessions_dir(), self.data_dir()] {
            self._backend.delete_path(&dir).await?;
            self._backend
                .write_file(&self._backend.join_path(&dir, ".keep"), b"")
                .await?;
        }

        {
            let mut skill_mgr = self._skill_mgr.lock().await;
            skill_mgr.load_index().await?;
        }

        Ok(())
    }

    fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    fn workdir(&self) -> &str {
        &self.workdir
    }

    fn is_alive(&self) -> bool {
        self.is_alive
    }

    async fn list_tools(&self) -> Result<Vec<ToolInfo>, WorkspaceError> {
        if !self.is_alive {
            return Err(WorkspaceError::NotInitialized);
        }
        let wd = &self.workdir;

        Ok(vec![
            ToolInfo {
                name: "Bash".into(),
                description: format!("Execute shell commands in {wd}"),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {"type": "string", "description": "Shell command to execute"}
                    },
                    "required": ["command"]
                }),
            },
            ToolInfo {
                name: "Read".into(),
                description: "Read file contents from workspace".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "File path within workspace"}
                    },
                    "required": ["path"]
                }),
            },
            ToolInfo {
                name: "Write".into(),
                description: "Write content to files in workspace".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "File to write"},
                        "content": {"type": "string", "description": "Content to write"}
                    },
                    "required": ["path", "content"]
                }),
            },
            ToolInfo {
                name: "Edit".into(),
                description: "Make precise string replacements in files".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "File to edit"},
                        "old": {"type": "string", "description": "String to replace"},
                        "new": {"type": "string", "description": "Replacement string"}
                    },
                    "required": ["path", "old", "new"]
                }),
            },
            ToolInfo {
                name: "Glob".into(),
                description: "Find files matching glob patterns".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "pattern": {"type": "string", "description": "Glob pattern"}
                    },
                    "required": ["pattern"]
                }),
            },
            ToolInfo {
                name: "Grep".into(),
                description: "Search for patterns in workspace files".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "pattern": {"type": "string", "description": "Regex pattern"}
                    },
                    "required": ["pattern"]
                }),
            },
        ])
    }

    async fn get_instructions(&self) -> String {
        self.instructions
            .replace("{workdir}", &self.workdir)
            .replace("{backend}", "LocalBackend")
    }

    async fn list_mcps(&self) -> Result<Vec<McpClientConfig>, WorkspaceError> {
        // Scrub sensitive headers from the in-memory copy before returning
        // (defect 3 fix). The persisted .mcp is also scrubbed at save time,
        // but we also filter here for defense-in-depth.
        Ok(self._mcps.iter().map(|m| m.scrubbed()).collect())
    }

    async fn add_mcp(&mut self, mcp: McpClientConfig) -> Result<(), WorkspaceError> {
        let _lock = self._mcp_lock.lock().await;
        if self._mcps.iter().any(|m| m.name == mcp.name) {
            return Err(WorkspaceError::McpAlreadyExists {
                name: mcp.name.clone(),
            });
        }
        // Warn about sensitive headers (defect 3 fix)
        let sensitive = mcp.transport.sensitive_headers_present();
        if !sensitive.is_empty() {
            tracing::warn!(
                "MCP '{}': contains sensitive headers ({:?}) — they will be scrubbed from .mcp persistence and list_mcps output",
                mcp.name,
                sensitive
            );
        }
        self._mcps.push(mcp);
        let mcp_path = self.mcp_path();
        McpRegistry::save(&self._mcps, &*self._backend, &mcp_path).await?;
        Ok(())
    }

    async fn remove_mcp(&mut self, name: &str) -> Result<(), WorkspaceError> {
        let _lock = self._mcp_lock.lock().await;
        let before = self._mcps.len();
        self._mcps.retain(|m| m.name != name);
        if self._mcps.len() == before {
            tracing::warn!("MCP not found for removal: {name}");
            return Ok(());
        }
        let mcp_path = self.mcp_path();
        McpRegistry::save(&self._mcps, &*self._backend, &mcp_path).await?;
        Ok(())
    }

    async fn list_skills(&self) -> Result<Vec<Skill>, WorkspaceError> {
        let mut skill_mgr = self._skill_mgr.lock().await;
        skill_mgr.list_skills().await
    }

    async fn add_skill(&mut self, skill_path: &str) -> Result<(), WorkspaceError> {
        let _lock = self._skill_lock.lock().await;
        let mut skill_mgr = self._skill_mgr.lock().await;
        let resolved = std::path::Path::new(skill_path)
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from(skill_path))
            .to_string_lossy()
            .to_string();
        skill_mgr.add_skill(&resolved).await
    }

    async fn remove_skill(&mut self, name: &str) -> Result<(), WorkspaceError> {
        let _lock = self._skill_lock.lock().await;
        let mut skill_mgr = self._skill_mgr.lock().await;
        skill_mgr.remove_skill(name).await
    }

    async fn offload_context(
        &self,
        session_id: &str,
        msgs: &[agent_scope_message::Msg],
    ) -> Result<String, WorkspaceError> {
        crate::offload::offload_context(
            session_id,
            msgs,
            &*self._backend,
            &self.sessions_dir(),
            &self.data_dir(),
        )
        .await
    }

    async fn offload_tool_result(
        &self,
        session_id: &str,
        tool_result: &agent_scope_message::ToolResultBlock,
    ) -> Result<String, WorkspaceError> {
        crate::offload::offload_tool_result(
            session_id,
            tool_result,
            &*self._backend,
            &self.sessions_dir(),
            &self.data_dir(),
        )
        .await
    }

    fn get_backend(&self) -> Result<&dyn WorkspaceBackend, WorkspaceError> {
        if !self.is_alive {
            return Err(WorkspaceError::NotInitialized);
        }
        Ok(&*self._backend)
    }

    fn get_backend_arc(&self) -> Result<Arc<dyn WorkspaceBackend>, WorkspaceError> {
        if !self.is_alive {
            return Err(WorkspaceError::NotInitialized);
        }
        Ok(Arc::clone(&self._backend))
    }
}

impl McpConnectionsHost for LocalWorkspace {
    fn mcp_connections(&self) -> &Arc<Mutex<HashMap<String, Arc<dyn McpConnectionHandle>>>> {
        &self._mcp_connections
    }
}
