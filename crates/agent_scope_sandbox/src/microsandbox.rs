//! Microsandbox-backed sandbox session implementation.
//!
//! This module is compiled only with the `microsandbox` feature. It
//! adapts the microsandbox Rust SDK to this crate's existing `SandboxSession`
//! contract without exposing SDK handle types in the public API.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use crate::capability::CapabilityReport;
use crate::error::{SandboxError, SandboxResult};
use crate::execution::{
    ExecutionRecord, ExecutionRequest, ExecutionResult, ExecutionStatus, OutputRef, OutputSummary,
    ResourceLimitHit, failure_category, redacted_command_summary,
};
use crate::mount::{MountAccess, SandboxMount};
use crate::policy::{
    NetworkPolicy, SandboxPolicy, memory_bytes_to_mib, validate_microsandbox_policy,
};
use crate::session::{SandboxSession, SandboxState};

const MICROSANDBOX_BACKEND: &str = "microsandbox";
const MAX_READ_FILE_BYTES: usize = 10 * 1024 * 1024;
const LOCAL_BACKEND_ENV_KEYS: &[&str] = &[
    "MSB_API_KEY",
    "MSB_BACKEND",
    "MSB_PROFILE",
    "MSB_CONFIG_PATH",
];

#[derive(Clone)]
pub struct MicrosandboxConfig {
    pub session_id: Option<String>,
    pub image: String,
    pub workdir: String,
    pub policy: SandboxPolicy,
    pub mounts: Vec<SandboxMount>,
    pub env: HashMap<String, String>,
    pub replace_existing: bool,
    pub persist: bool,
    pub startup_timeout: Duration,
    pub stop_timeout: Duration,
}

impl std::fmt::Debug for MicrosandboxConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicrosandboxConfig")
            .field("session_id", &self.session_id)
            .field("image", &self.image)
            .field("workdir", &self.workdir)
            .field("policy", &self.policy)
            .field("mounts", &self.mounts)
            .field("env_keys", &self.env.keys().collect::<Vec<_>>())
            .field("replace_existing", &self.replace_existing)
            .field("persist", &self.persist)
            .field("startup_timeout", &self.startup_timeout)
            .field("stop_timeout", &self.stop_timeout)
            .finish()
    }
}

impl Default for MicrosandboxConfig {
    fn default() -> Self {
        let policy = SandboxPolicy {
            network: NetworkPolicy::Disabled,
            ..SandboxPolicy::default()
        };
        Self {
            session_id: None,
            image: "python".into(),
            workdir: "/workspace".into(),
            policy,
            mounts: Vec::new(),
            env: HashMap::new(),
            replace_existing: false,
            persist: false,
            startup_timeout: Duration::from_secs(120),
            stop_timeout: Duration::from_secs(30),
        }
    }
}

pub struct MicrosandboxSession {
    session_id: String,
    config: MicrosandboxConfig,
    state: SandboxState,
    handle: Option<::microsandbox::Sandbox>,
    history: Vec<ExecutionRecord>,
    created_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    output_dir: PathBuf,
    next_sequence: u64,
}

impl std::fmt::Debug for MicrosandboxSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicrosandboxSession")
            .field("session_id", &self.session_id)
            .field("config", &self.config)
            .field("state", &self.state)
            .field("history", &self.history)
            .field("created_at", &self.created_at)
            .field("closed_at", &self.closed_at)
            .field("output_dir", &self.output_dir)
            .field("next_sequence", &self.next_sequence)
            .finish_non_exhaustive()
    }
}

impl MicrosandboxSession {
    pub fn new(config: MicrosandboxConfig) -> SandboxResult<Self> {
        validate_config(&config)?;
        let session_id = config
            .session_id
            .clone()
            .unwrap_or_else(agent_scope_utils::id::generate_uuid);
        validate_session_id(&session_id)?;
        let output_dir = std::env::temp_dir()
            .join(format!("agentscope-microsandbox-{session_id}"))
            .join(".sandbox-output");
        Ok(Self {
            session_id,
            config,
            state: SandboxState::Created,
            handle: None,
            history: Vec::new(),
            created_at: Utc::now(),
            closed_at: None,
            output_dir,
            next_sequence: 1,
        })
    }

    #[must_use]
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    fn guard(&self, operation: &str) -> SandboxResult<()> {
        match self.state {
            SandboxState::Ready => Ok(()),
            SandboxState::Created if operation == "initialize" => Ok(()),
            SandboxState::Closing
            | SandboxState::Closed
            | SandboxState::Created
            | SandboxState::Failed => Err(SandboxError::LifecycleError {
                state: self.state,
                operation: operation.into(),
            }),
        }
    }

    fn sandbox(&self, operation: &str) -> SandboxResult<&::microsandbox::Sandbox> {
        self.handle
            .as_ref()
            .ok_or_else(|| SandboxError::LifecycleError {
                state: self.state,
                operation: operation.into(),
            })
    }

    fn resolve_guest_path(&self, path: &str, operation: &str) -> SandboxResult<String> {
        resolve_guest_path(&self.config.workdir, &self.config.mounts, path, operation)
    }

    fn check_write_access(&self, guest_path: &str, operation: &str) -> SandboxResult<()> {
        if let Some(mount) = access_for_guest_path(&self.config.mounts, guest_path)?
            && mount.access == MountAccess::ReadOnly
        {
            return Err(SandboxError::PermissionDenied {
                path: Some(guest_path.into()),
                operation: operation.into(),
            });
        }
        Ok(())
    }

    async fn output_summary(
        &self,
        execution_id: &str,
        stream: &str,
        bytes: Vec<u8>,
    ) -> SandboxResult<OutputSummary> {
        let truncated = bytes.len() > self.config.policy.max_output_bytes;
        let inline = if truncated {
            bytes[..self.config.policy.max_output_bytes].to_vec()
        } else {
            bytes.clone()
        };
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let sha256 = format!("{:x}", hasher.finalize());
        tokio::fs::create_dir_all(&self.output_dir)
            .await
            .map_err(|e| SandboxError::IoError {
                operation: "create_output_dir".into(),
                message: e.to_string(),
            })?;
        let path = self.output_dir.join(format!("{execution_id}-{stream}.bin"));
        tokio::fs::write(&path, &bytes)
            .await
            .map_err(|e| SandboxError::IoError {
                operation: "write_output_ref".into(),
                message: e.to_string(),
            })?;
        Ok(OutputSummary {
            inline,
            truncated,
            full_ref: Some(OutputRef {
                path,
                sha256,
                bytes: bytes.len() as u64,
            }),
        })
    }

    fn append_record(
        &mut self,
        request: &ExecutionRequest,
        cwd: PathBuf,
        result: &ExecutionResult,
    ) {
        let record = ExecutionRecord {
            sequence: self.next_sequence,
            execution_id: result.execution_id.clone(),
            command_summary: redacted_command_summary(request),
            cwd,
            status: result.status.clone(),
            duration: result.duration,
            failure_category: failure_category(&result.status),
            stdout_ref: result.stdout.full_ref.clone(),
            stderr_ref: result.stderr.full_ref.clone(),
        };
        self.next_sequence += 1;
        self.history.push(record);
    }

    async fn collect_dir(
        sandbox: &::microsandbox::Sandbox,
        path: String,
        recursive: bool,
    ) -> SandboxResult<Vec<String>> {
        let mut entries = Vec::new();
        let mut stack = vec![path];
        while let Some(dir) = stack.pop() {
            let children = sandbox
                .fs()
                .list(&dir)
                .await
                .map_err(sdk_fs_error("list_dir"))?;
            for child in children {
                let is_dir = matches!(
                    child.kind,
                    ::microsandbox::sandbox::fs::FsEntryKind::Directory
                );
                entries.push(child.path.clone());
                if recursive && is_dir {
                    stack.push(child.path);
                }
            }
        }
        entries.sort();
        Ok(entries)
    }
}

#[async_trait]
impl SandboxSession for MicrosandboxSession {
    fn session_id(&self) -> &str {
        &self.session_id
    }

    fn state(&self) -> SandboxState {
        self.state
    }

    fn policy(&self) -> &SandboxPolicy {
        &self.config.policy
    }

    async fn initialize(&mut self) -> SandboxResult<()> {
        if matches!(self.state, SandboxState::Ready | SandboxState::Closed) {
            return Ok(());
        }
        self.guard("initialize")?;
        validate_microsandbox_policy(&self.config.policy)?;

        let mut builder = ::microsandbox::Sandbox::builder(self.session_id.clone())
            .image(self.config.image.clone())
            .workdir(self.config.workdir.clone())
            .ephemeral(!self.config.persist && !self.config.policy.keep_on_close);

        if self.config.replace_existing {
            builder = builder.replace_with_timeout(self.config.startup_timeout);
        }
        if matches!(self.config.policy.network, NetworkPolicy::Disabled) {
            builder = builder.disable_network();
        }
        if let Some(bytes) = self.config.policy.memory_limit_bytes {
            let mib = memory_bytes_to_mib(bytes)?;
            let mib: u32 = mib.try_into().map_err(|_| SandboxError::ValidationError {
                message: "memory_limit_bytes is too large for microsandbox memory MiB".into(),
            })?;
            builder = builder.memory(mib);
        }
        for (key, value) in &self.config.env {
            builder = builder.env(key, value);
        }
        for mount in &self.config.mounts {
            let guest = guest_mount_path(mount)?;
            let host =
                std::fs::canonicalize(&mount.host_path).map_err(|e| SandboxError::IoError {
                    operation: "canonicalize_mount".into(),
                    message: format!("{}: {e}", mount.host_path.display()),
                })?;
            reject_sensitive_host_path(&host)?;
            builder = builder.volume(guest, |volume| {
                let volume = volume.bind(host);
                if mount.access == MountAccess::ReadOnly {
                    volume.readonly()
                } else {
                    volume
                }
            });
        }

        let local_backend = explicit_local_backend()?;
        let create = ::microsandbox::with_backend(local_backend, builder.create());
        match tokio::time::timeout(self.config.startup_timeout, create).await {
            Ok(Ok(sandbox)) => {
                self.handle = Some(sandbox);
                self.state = SandboxState::Ready;
                Ok(())
            }
            Ok(Err(err)) => {
                self.state = SandboxState::Failed;
                Err(sdk_unavailable(err))
            }
            Err(_) => {
                self.state = SandboxState::Failed;
                Err(SandboxError::SandboxUnavailable {
                    backend: MICROSANDBOX_BACKEND.into(),
                    reason: format!(
                        "sandbox creation exceeded startup timeout of {:?}",
                        self.config.startup_timeout
                    ),
                })
            }
        }
    }

    async fn execute(&mut self, request: ExecutionRequest) -> SandboxResult<ExecutionResult> {
        self.guard("execute")?;
        if request.argv.is_empty() || request.argv[0].is_empty() {
            return Err(SandboxError::ValidationError {
                message: "argv must not be empty".into(),
            });
        }
        let timeout_duration = request
            .timeout
            .unwrap_or(self.config.policy.default_timeout);
        if timeout_duration > self.config.policy.max_timeout {
            return Err(SandboxError::ValidationError {
                message: "timeout exceeds policy max_timeout".into(),
            });
        }
        let cwd = if let Some(cwd) = &request.cwd {
            self.resolve_guest_path(cwd.to_string_lossy().as_ref(), "execute_cwd")?
        } else {
            self.config.workdir.clone()
        };
        let sandbox = self.sandbox("execute")?;
        let execution_id = agent_scope_utils::id::generate_uuid();
        let started_at = Utc::now();
        let started = Instant::now();
        let command = request.argv[0].clone();
        let args = request.argv[1..].to_vec();
        let env = request.env.clone();
        let stdin = request.stdin.clone();

        let output = sandbox
            .exec_with(command, |exec| {
                let mut exec = exec
                    .args(args)
                    .cwd(cwd.clone())
                    .timeout(timeout_duration)
                    .envs(env);
                if let Some(stdin) = stdin {
                    exec = exec.stdin_bytes(stdin);
                }
                exec
            })
            .await;

        let mut hits = Vec::new();
        let (status, exit_code, stdout, stderr) = match output {
            Ok(output) => {
                let status = output.status();
                (
                    ExecutionStatus::Exited { code: status.code },
                    Some(status.code),
                    output.stdout_bytes().to_vec(),
                    output.stderr_bytes().to_vec(),
                )
            }
            Err(::microsandbox::MicrosandboxError::ExecTimeout(_)) => {
                hits.push(ResourceLimitHit::Timeout);
                (ExecutionStatus::TimedOut, None, Vec::new(), Vec::new())
            }
            Err(err) => return Err(sdk_unavailable(err)),
        };

        let stdout_summary = self.output_summary(&execution_id, "stdout", stdout).await?;
        let stderr_summary = self.output_summary(&execution_id, "stderr", stderr).await?;
        if stdout_summary.truncated || stderr_summary.truncated {
            hits.push(ResourceLimitHit::OutputTruncated);
        }
        let finished_at = Utc::now();
        let result = ExecutionResult {
            execution_id,
            status,
            exit_code,
            stdout: stdout_summary,
            stderr: stderr_summary,
            started_at,
            finished_at,
            duration: started.elapsed(),
            resource_hits: hits,
        };
        self.append_record(&request, PathBuf::from(cwd), &result);
        Ok(result)
    }

    async fn read_file(&self, path: &str) -> SandboxResult<Vec<u8>> {
        self.guard("read_file")?;
        let guest_path = self.resolve_guest_path(path, "read_file")?;
        let fs = self.sandbox("read_file")?.fs();
        let meta = fs
            .stat(&guest_path)
            .await
            .map_err(sdk_fs_error("read_file"))?;
        if meta.size > MAX_READ_FILE_BYTES as u64 {
            return Err(SandboxError::ValidationError {
                message: format!(
                    "read_file: '{}' is {} bytes, exceeding the {} byte cap",
                    path, meta.size, MAX_READ_FILE_BYTES
                ),
            });
        }
        let data = fs
            .read(&guest_path)
            .await
            .map_err(sdk_fs_error("read_file"))?;
        if data.len() > MAX_READ_FILE_BYTES {
            return Err(SandboxError::ValidationError {
                message: format!(
                    "read_file: '{}' is {} bytes, exceeding the {} byte cap",
                    path,
                    data.len(),
                    MAX_READ_FILE_BYTES
                ),
            });
        }
        Ok(data.to_vec())
    }

    async fn write_file(&mut self, path: &str, data: &[u8]) -> SandboxResult<()> {
        self.guard("write_file")?;
        let guest_path = self.resolve_guest_path(path, "write_file")?;
        self.check_write_access(&guest_path, "write_file")?;
        let fs = self.sandbox("write_file")?.fs();
        if let Some(parent) = guest_parent_dir(&guest_path) {
            fs.mkdir(&parent)
                .await
                .map_err(sdk_fs_error("write_file"))?;
        }
        fs.write(&guest_path, data)
            .await
            .map_err(sdk_fs_error("write_file"))
    }

    async fn delete_path(&mut self, path: &str) -> SandboxResult<()> {
        self.guard("delete_path")?;
        let guest_path = self.resolve_guest_path(path, "delete_path")?;
        if guest_path == normalized_absolute(&self.config.workdir, "workdir")? {
            return Err(SandboxError::PermissionDenied {
                path: Some(guest_path),
                operation: "delete_path".into(),
            });
        }
        self.check_write_access(&guest_path, "delete_path")?;
        let sandbox = self.sandbox("delete_path")?;
        if !sandbox
            .fs()
            .exists(&guest_path)
            .await
            .map_err(sdk_fs_error("delete_path"))?
        {
            return Ok(());
        }
        let meta = sandbox
            .fs()
            .stat(&guest_path)
            .await
            .map_err(sdk_fs_error("delete_path"))?;
        if matches!(
            meta.kind,
            ::microsandbox::sandbox::fs::FsEntryKind::Directory
        ) {
            sandbox
                .fs()
                .remove_dir(&guest_path)
                .await
                .map_err(sdk_fs_error("delete_path"))
        } else {
            sandbox
                .fs()
                .remove(&guest_path)
                .await
                .map_err(sdk_fs_error("delete_path"))
        }
    }

    async fn is_dir(&self, path: &str) -> SandboxResult<bool> {
        self.guard("is_dir")?;
        let guest_path = self.resolve_guest_path(path, "is_dir")?;
        match self.sandbox("is_dir")?.fs().stat(&guest_path).await {
            Ok(meta) => Ok(matches!(
                meta.kind,
                ::microsandbox::sandbox::fs::FsEntryKind::Directory
            )),
            Err(_) => Ok(false),
        }
    }

    async fn path_exists(&self, path: &str) -> SandboxResult<bool> {
        self.guard("path_exists")?;
        let guest_path = self.resolve_guest_path(path, "path_exists")?;
        self.sandbox("path_exists")?
            .fs()
            .exists(&guest_path)
            .await
            .map_err(sdk_fs_error("path_exists"))
    }

    async fn stat_mtime(&self, path: &str) -> SandboxResult<Option<f64>> {
        self.guard("stat_mtime")?;
        let guest_path = self.resolve_guest_path(path, "stat_mtime")?;
        match self.sandbox("stat_mtime")?.fs().stat(&guest_path).await {
            Ok(meta) => Ok(meta
                .modified
                .map(|modified| modified.timestamp_millis() as f64 / 1000.0)),
            Err(_) => Ok(None),
        }
    }

    async fn list_dir(&self, path: &str, recursive: bool) -> SandboxResult<Vec<String>> {
        self.guard("list_dir")?;
        let guest_path = self.resolve_guest_path(path, "list_dir")?;
        let sandbox = self.sandbox("list_dir")?;
        match sandbox.fs().stat(&guest_path).await {
            Ok(meta)
                if matches!(
                    meta.kind,
                    ::microsandbox::sandbox::fs::FsEntryKind::Directory
                ) =>
            {
                Self::collect_dir(sandbox, guest_path, recursive).await
            }
            Ok(_) | Err(_) => Ok(Vec::new()),
        }
    }

    async fn history(&self) -> SandboxResult<Vec<ExecutionRecord>> {
        Ok(self.history.clone())
    }

    async fn capability_report(&self) -> SandboxResult<CapabilityReport> {
        Ok(CapabilityReport::microsandbox())
    }

    async fn close(&mut self) -> SandboxResult<()> {
        if matches!(self.state, SandboxState::Closed) {
            return Ok(());
        }
        if matches!(self.state, SandboxState::Created | SandboxState::Failed) {
            self.state = SandboxState::Closed;
            self.closed_at = Some(Utc::now());
            return Ok(());
        }
        self.state = SandboxState::Closing;
        if let Some(sandbox) = self.handle.take()
            && let Err(err) = sandbox.stop_with_timeout(self.config.stop_timeout).await
        {
            self.state = SandboxState::Failed;
            self.closed_at = Some(Utc::now());
            return Err(sdk_unavailable(err));
        }
        self.state = SandboxState::Closed;
        self.closed_at = Some(Utc::now());
        if !self.config.persist && !self.config.policy.keep_on_close {
            let _ = tokio::fs::remove_dir_all(&self.output_dir).await;
        }
        Ok(())
    }

    async fn cleanup(&mut self) -> SandboxResult<()> {
        self.close().await?;
        if !self.config.persist && !self.config.policy.keep_on_close {
            let _ = tokio::fs::remove_dir_all(&self.output_dir).await;
        }
        Ok(())
    }
}

fn explicit_local_backend() -> SandboxResult<::microsandbox::backend::LocalBackend> {
    explicit_local_backend_with_env(|key| std::env::var_os(key).is_some())
}

fn explicit_local_backend_with_env(
    has_env: impl Fn(&str) -> bool,
) -> SandboxResult<::microsandbox::backend::LocalBackend> {
    if let Some(key) = LOCAL_BACKEND_ENV_KEYS.iter().find(|key| has_env(key)) {
        return Err(SandboxError::ValidationError {
            message: format!(
                "microsandbox integration requires an explicit local backend; unset {key} to avoid ambient SDK backend/profile selection"
            ),
        });
    }
    Ok(::microsandbox::backend::LocalBackend::lazy())
}

fn guest_parent_dir(path: &str) -> Option<String> {
    let parent = path.rsplit_once('/')?.0;
    if parent.is_empty() {
        Some("/".into())
    } else {
        Some(parent.into())
    }
}

fn validate_config(config: &MicrosandboxConfig) -> SandboxResult<()> {
    if let Some(session_id) = &config.session_id {
        validate_session_id(session_id)?;
    }
    if config.image.trim().is_empty() {
        return Err(SandboxError::ValidationError {
            message: "image must not be empty".into(),
        });
    }
    let workdir = normalized_absolute(&config.workdir, "workdir")?;
    if workdir == "/" {
        return Err(SandboxError::ValidationError {
            message: "workdir must not be the guest filesystem root".into(),
        });
    }
    if config.startup_timeout.is_zero() {
        return Err(SandboxError::ValidationError {
            message: "startup_timeout must be > 0".into(),
        });
    }
    if config.stop_timeout.is_zero() {
        return Err(SandboxError::ValidationError {
            message: "stop_timeout must be > 0".into(),
        });
    }
    validate_microsandbox_policy(&config.policy)?;
    for key in config.env.keys() {
        if key.trim().is_empty() {
            return Err(SandboxError::ValidationError {
                message: "env keys must not be empty".into(),
            });
        }
        if key.starts_with("MSB_") {
            return Err(SandboxError::ValidationError {
                message: format!("environment variable {key:?} uses the reserved MSB_ prefix"),
            });
        }
    }
    for mount in &config.mounts {
        validate_microsandbox_mount(mount)?;
    }
    Ok(())
}

fn validate_session_id(session_id: &str) -> SandboxResult<()> {
    if session_id.is_empty() {
        return Err(SandboxError::ValidationError {
            message: "session_id must not be empty".into(),
        });
    }
    if !session_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(SandboxError::ValidationError {
            message: "session_id must only contain [A-Za-z0-9_-]".into(),
        });
    }
    Ok(())
}

fn validate_microsandbox_mount(mount: &SandboxMount) -> SandboxResult<()> {
    if mount.mount_id.is_empty() {
        return Err(SandboxError::ValidationError {
            message: "mount_id must not be empty".into(),
        });
    }
    if mount.host_path.as_os_str().is_empty() {
        return Err(SandboxError::ValidationError {
            message: "host_path must not be empty".into(),
        });
    }
    let host_path = mount.host_path.as_path();
    if !host_path.exists() {
        return Err(SandboxError::ValidationError {
            message: format!("host_path '{}' does not exist", host_path.display()),
        });
    }
    reject_sensitive_host_path(host_path)?;
    let canonical_host = std::fs::canonicalize(host_path).map_err(|e| SandboxError::IoError {
        operation: "canonicalize_mount".into(),
        message: format!("{}: {e}", host_path.display()),
    })?;
    reject_sensitive_host_path(&canonical_host)?;
    guest_mount_path(mount)?;
    Ok(())
}

fn reject_sensitive_host_path(path: &Path) -> SandboxResult<()> {
    let normalized = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();

    for part in &normalized {
        let lower = part.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            ".ssh"
                | ".aws"
                | ".config"
                | ".gnupg"
                | ".kube"
                | "credentials"
                | "credential"
                | "tokens"
                | "token"
                | "secrets"
                | "secret"
        ) {
            return Err(SandboxError::PermissionDenied {
                path: Some(path.display().to_string()),
                operation: "mount".into(),
            });
        }
    }
    Ok(())
}

fn guest_mount_path(mount: &SandboxMount) -> SandboxResult<String> {
    let guest = mount.sandbox_path.to_string_lossy();
    normalized_absolute(&guest, "mount")
}

fn resolve_guest_path(
    workdir: &str,
    mounts: &[SandboxMount],
    path: &str,
    operation: &str,
) -> SandboxResult<String> {
    if path.trim().is_empty() {
        return Err(SandboxError::ValidationError {
            message: format!("{operation}: path must not be empty"),
        });
    }
    let base = normalized_absolute(workdir, "workdir")?;
    let raw = Path::new(path);
    let mut out =
        if raw.is_absolute() || matches!(raw.components().next(), Some(Component::RootDir)) {
            PathBuf::new()
        } else {
            PathBuf::from(&base)
        };
    for component in raw.components() {
        match component {
            Component::RootDir => out.push("/"),
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(SandboxError::PermissionDenied {
                    path: Some(path.into()),
                    operation: operation.into(),
                });
            }
            Component::Prefix(_) => {
                return Err(SandboxError::PermissionDenied {
                    path: Some(path.into()),
                    operation: operation.into(),
                });
            }
        }
    }
    let normalized = pathbuf_to_guest_string(&out, operation)?;
    if !normalized.starts_with('/') {
        return Err(SandboxError::ValidationError {
            message: format!("{operation}: guest path must be absolute"),
        });
    }
    if is_authorized_guest_path(&normalized, &base, mounts)? {
        return Ok(normalized);
    }
    Err(SandboxError::PermissionDenied {
        path: Some(normalized),
        operation: operation.into(),
    })
}

fn is_authorized_guest_path(
    guest_path: &str,
    workdir: &str,
    mounts: &[SandboxMount],
) -> SandboxResult<bool> {
    if path_is_same_or_child(guest_path, workdir) {
        return Ok(true);
    }
    for mount in mounts {
        let mount_path = guest_mount_path(mount)?;
        if path_is_same_or_child(guest_path, &mount_path) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn access_for_guest_path<'a>(
    mounts: &'a [SandboxMount],
    guest_path: &str,
) -> SandboxResult<Option<&'a SandboxMount>> {
    let mut best: Option<(&SandboxMount, usize)> = None;
    for mount in mounts {
        let mount_path = guest_mount_path(mount)?;
        if path_is_same_or_child(guest_path, &mount_path) {
            let depth = mount_path
                .split('/')
                .filter(|part| !part.is_empty())
                .count();
            if best.is_none_or(|(_, best_depth)| depth > best_depth) {
                best = Some((mount, depth));
            }
        }
    }
    Ok(best.map(|(mount, _)| mount))
}

fn path_is_same_or_child(path: &str, root: &str) -> bool {
    if root == "/" {
        return path.starts_with('/');
    }
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn normalized_absolute(path: &str, operation: &str) -> SandboxResult<String> {
    if path.trim().is_empty() {
        return Err(SandboxError::ValidationError {
            message: format!("{operation}: path must not be empty"),
        });
    }
    let raw = Path::new(path);
    if !(raw.is_absolute() || matches!(raw.components().next(), Some(Component::RootDir))) {
        return Err(SandboxError::ValidationError {
            message: format!("{operation}: guest path must be absolute"),
        });
    }
    let mut out = PathBuf::new();
    for component in raw.components() {
        match component {
            Component::RootDir => out.push("/"),
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(SandboxError::PermissionDenied {
                    path: Some(path.into()),
                    operation: operation.into(),
                });
            }
            Component::Prefix(_) => {
                return Err(SandboxError::PermissionDenied {
                    path: Some(path.into()),
                    operation: operation.into(),
                });
            }
        }
    }
    pathbuf_to_guest_string(&out, operation)
}

fn pathbuf_to_guest_string(path: &Path, operation: &str) -> SandboxResult<String> {
    let display = path.to_string_lossy().replace('\\', "/");
    if display.is_empty() {
        return Err(SandboxError::ValidationError {
            message: format!("{operation}: guest path must not be empty"),
        });
    }
    Ok(display)
}

fn sdk_unavailable(err: ::microsandbox::MicrosandboxError) -> SandboxError {
    SandboxError::SandboxUnavailable {
        backend: MICROSANDBOX_BACKEND.into(),
        reason: err.to_string(),
    }
}

fn sdk_fs_error(
    operation: &'static str,
) -> impl FnOnce(::microsandbox::MicrosandboxError) -> SandboxError {
    move |err| SandboxError::IoError {
        operation: operation.into(),
        message: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mount::{MountOwner, SandboxMount};

    fn mount_at(path: &str, access: MountAccess) -> SandboxMount {
        SandboxMount {
            mount_id: path.trim_start_matches('/').replace('/', "-"),
            host_path: PathBuf::from("/tmp/agentscope-test-mount"),
            sandbox_path: PathBuf::from(path),
            access,
            persist: false,
            owner: MountOwner::User,
        }
    }

    #[test]
    fn guest_path_resolver_rejects_parent_segments_and_unauthorized_absolute_paths() {
        assert!(matches!(
            resolve_guest_path("/workspace", &[], "../etc/passwd", "read_file"),
            Err(SandboxError::PermissionDenied { .. })
        ));
        assert!(matches!(
            resolve_guest_path("/workspace", &[], "/etc/passwd", "read_file"),
            Err(SandboxError::PermissionDenied { .. })
        ));
        assert!(matches!(
            resolve_guest_path("/workspace", &[], "/tmp/x", "write_file"),
            Err(SandboxError::PermissionDenied { .. })
        ));
    }

    #[test]
    fn guest_parent_dir_returns_parent_for_nested_paths() {
        assert_eq!(
            guest_parent_dir("/workspace/dir/file.txt"),
            Some("/workspace/dir".into())
        );
        assert_eq!(
            guest_parent_dir("/workspace/file.txt"),
            Some("/workspace".into())
        );
    }

    #[test]
    fn explicit_local_backend_rejects_ambient_msb_config() {
        let result = explicit_local_backend_with_env(|key| key == "MSB_BACKEND");
        assert!(
            matches!(result, Err(SandboxError::ValidationError { message }) if message.contains("MSB_BACKEND"))
        );
    }

    #[test]
    fn guest_path_resolver_allows_workdir_and_explicit_mount_roots() {
        let mounts = vec![mount_at("/mnt/data", MountAccess::ReadOnly)];
        assert_eq!(
            resolve_guest_path("/workspace", &mounts, "note.txt", "read_file").unwrap(),
            "/workspace/note.txt"
        );
        assert_eq!(
            resolve_guest_path("/workspace", &mounts, "/workspace/note.txt", "read_file").unwrap(),
            "/workspace/note.txt"
        );
        assert_eq!(
            resolve_guest_path("/workspace", &mounts, "/mnt/data/input.txt", "read_file").unwrap(),
            "/mnt/data/input.txt"
        );
    }

    #[test]
    fn guest_path_mount_access_uses_guest_mount_path_boundaries() {
        let mounts = vec![
            mount_at("/mnt", MountAccess::ReadWrite),
            mount_at("/mnt/readonly", MountAccess::ReadOnly),
        ];
        assert_eq!(
            access_for_guest_path(&mounts, "/mnt/readonly/file.txt")
                .unwrap()
                .unwrap()
                .access,
            MountAccess::ReadOnly
        );
        assert_eq!(
            access_for_guest_path(&mounts, "/mnt/readwrite/file.txt")
                .unwrap()
                .unwrap()
                .access,
            MountAccess::ReadWrite
        );
        assert!(
            access_for_guest_path(&mounts, "/mnt2/file.txt")
                .unwrap()
                .is_none()
        );
    }
}
