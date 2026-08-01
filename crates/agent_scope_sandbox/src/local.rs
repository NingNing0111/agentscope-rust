//! Local sandbox session implementation.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::timeout;
use uuid::Uuid;

use crate::capability::CapabilityReport;
use crate::error::{SandboxError, SandboxResult};
use crate::execution::{
    ExecutionRecord, ExecutionRequest, ExecutionResult, ExecutionStatus, OutputRef, OutputSummary,
    ResourceLimitHit, failure_category, redacted_command_summary,
};
use crate::mount::{MountAccess, SandboxMount, access_for_path};
use crate::path::SandboxPathResolver;
use crate::policy::SandboxPolicy;
use crate::session::{LocalSandboxConfig, SandboxSession, SandboxState};

#[derive(Debug)]
pub struct LocalSandboxSession {
    session_id: String,
    root_dir: PathBuf,
    workdir: PathBuf,
    state: SandboxState,
    policy: SandboxPolicy,
    mounts: Vec<SandboxMount>,
    history: Vec<ExecutionRecord>,
    created_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    owned_temp_root: bool,
    next_sequence: u64,
}

impl LocalSandboxSession {
    pub fn new(config: LocalSandboxConfig) -> SandboxResult<Self> {
        config.policy.validate()?;
        let session_id = config
            .session_id
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        if session_id.is_empty() {
            return Err(SandboxError::ValidationError {
                message: "session_id must not be empty".into(),
            });
        }
        if let Some((feature, reason)) = config
            .policy
            .requested_unsupported_features()
            .into_iter()
            .next()
        {
            return Err(SandboxError::UnsupportedFeature {
                feature: feature.into(),
                reason: reason.into(),
            });
        }
        let owned_temp_root = config.root_dir.is_none();
        let root_dir = config.root_dir.unwrap_or_else(|| {
            std::env::temp_dir().join(format!("agentscope-sandbox-{session_id}"))
        });
        let workdir = config.workdir.unwrap_or_else(|| root_dir.join("work"));
        Ok(Self {
            session_id,
            root_dir,
            workdir,
            state: SandboxState::Created,
            policy: config.policy,
            mounts: config.mounts,
            history: Vec::new(),
            created_at: Utc::now(),
            closed_at: None,
            owned_temp_root,
            next_sequence: 1,
        })
    }

    #[must_use]
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }
    #[must_use]
    pub fn workdir(&self) -> &Path {
        &self.workdir
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

    fn resolver(&self) -> SandboxResult<SandboxPathResolver> {
        SandboxPathResolver::new(self.root_dir.clone(), self.workdir.clone())
    }

    fn check_write_access(&self, path: &Path, operation: &str) -> SandboxResult<()> {
        if let Some(mount) = access_for_path(&self.mounts, path)
            && mount.access == MountAccess::ReadOnly
        {
            return Err(SandboxError::PermissionDenied {
                path: Some(path.display().to_string()),
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
        let truncated = bytes.len() > self.policy.max_output_bytes;
        let inline = if truncated {
            bytes[..self.policy.max_output_bytes].to_vec()
        } else {
            bytes.clone()
        };
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let sha256 = format!("{:x}", hasher.finalize());
        let out_dir = self.root_dir.join(".sandbox-output");
        tokio::fs::create_dir_all(&out_dir)
            .await
            .map_err(|e| SandboxError::IoError {
                operation: "create_output_dir".into(),
                message: e.to_string(),
            })?;
        let path = out_dir.join(format!("{execution_id}-{stream}.bin"));
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

    async fn collect_dir(path: PathBuf, recursive: bool) -> SandboxResult<Vec<String>> {
        let mut entries = Vec::new();
        let mut stack = vec![path];
        while let Some(dir) = stack.pop() {
            let mut rd = tokio::fs::read_dir(&dir)
                .await
                .map_err(|e| SandboxError::IoError {
                    operation: "list_dir".into(),
                    message: e.to_string(),
                })?;
            while let Some(entry) = rd.next_entry().await.map_err(|e| SandboxError::IoError {
                operation: "list_dir_entry".into(),
                message: e.to_string(),
            })? {
                let p = entry.path();
                entries.push(p.display().to_string());
                if recursive
                    && entry
                        .file_type()
                        .await
                        .map(|ft| ft.is_dir())
                        .unwrap_or(false)
                {
                    stack.push(p);
                }
            }
        }
        entries.sort();
        Ok(entries)
    }
}

#[async_trait]
impl SandboxSession for LocalSandboxSession {
    fn session_id(&self) -> &str {
        &self.session_id
    }
    fn state(&self) -> SandboxState {
        self.state
    }
    fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    async fn initialize(&mut self) -> SandboxResult<()> {
        if matches!(self.state, SandboxState::Ready) {
            return Ok(());
        }
        if matches!(self.state, SandboxState::Closed) {
            return Ok(());
        }
        self.guard("initialize")?;
        tokio::fs::create_dir_all(&self.workdir)
            .await
            .map_err(|e| SandboxError::IoError {
                operation: "initialize".into(),
                message: e.to_string(),
            })?;
        self.root_dir = self
            .root_dir
            .canonicalize()
            .map_err(|e| SandboxError::IoError {
                operation: "canonicalize_root".into(),
                message: e.to_string(),
            })?;
        self.workdir = self
            .workdir
            .canonicalize()
            .map_err(|e| SandboxError::IoError {
                operation: "canonicalize_workdir".into(),
                message: e.to_string(),
            })?;
        for mount in &mut self.mounts {
            mount.validate(&self.root_dir)?;
        }
        self.state = SandboxState::Ready;
        Ok(())
    }

    async fn execute(&mut self, request: ExecutionRequest) -> SandboxResult<ExecutionResult> {
        self.guard("execute")?;
        if request.argv.is_empty() || request.argv[0].is_empty() {
            return Err(SandboxError::ValidationError {
                message: "argv must not be empty".into(),
            });
        }
        let timeout_duration = request.timeout.unwrap_or(self.policy.default_timeout);
        if timeout_duration > self.policy.max_timeout {
            return Err(SandboxError::ValidationError {
                message: "timeout exceeds policy max_timeout".into(),
            });
        }
        let resolver = self.resolver()?;
        let cwd = if let Some(cwd) = &request.cwd {
            resolver.resolve(cwd.to_string_lossy().as_ref(), None, true, "execute_cwd")?
        } else {
            self.workdir.clone()
        };
        let execution_id = Uuid::new_v4().to_string();
        let started_at = Utc::now();
        let started = Instant::now();
        let mut cmd = Command::new(&request.argv[0]);
        cmd.args(&request.argv[1..])
            .current_dir(&cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(if request.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            });
        for (k, v) in &request.env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().map_err(|e| SandboxError::IoError {
            operation: format!("spawn {}", request.argv[0]),
            message: e.to_string(),
        })?;
        if let Some(stdin) = request.stdin.clone()
            && let Some(mut child_stdin) = child.stdin.take()
        {
            child_stdin
                .write_all(&stdin)
                .await
                .map_err(|e| SandboxError::IoError {
                    operation: "stdin".into(),
                    message: e.to_string(),
                })?;
        }

        let mut stdout_pipe = child.stdout.take();
        let mut stderr_pipe = child.stderr.take();
        let stdout_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            if let Some(mut stdout) = stdout_pipe.take() {
                stdout.read_to_end(&mut bytes).await?;
            }
            Ok::<Vec<u8>, std::io::Error>(bytes)
        });
        let stderr_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            if let Some(mut stderr) = stderr_pipe.take() {
                stderr.read_to_end(&mut bytes).await?;
            }
            Ok::<Vec<u8>, std::io::Error>(bytes)
        });

        let wait_result = timeout(timeout_duration, child.wait()).await;
        let (status, exit_code, mut hits) = match wait_result {
            Ok(Ok(status)) => {
                let code = status.code().unwrap_or(-1);
                (ExecutionStatus::Exited { code }, Some(code), Vec::new())
            }
            Ok(Err(e)) => {
                return Err(SandboxError::IoError {
                    operation: "wait".into(),
                    message: e.to_string(),
                });
            }
            Err(_) => {
                child.kill().await.map_err(|e| SandboxError::IoError {
                    operation: "timeout_kill".into(),
                    message: e.to_string(),
                })?;
                child.wait().await.map_err(|e| SandboxError::IoError {
                    operation: "timeout_wait".into(),
                    message: e.to_string(),
                })?;
                (
                    ExecutionStatus::TimedOut,
                    None,
                    vec![ResourceLimitHit::Timeout],
                )
            }
        };
        let stdout = stdout_task
            .await
            .map_err(|e| SandboxError::IoError {
                operation: "stdout_join".into(),
                message: e.to_string(),
            })?
            .map_err(|e| SandboxError::IoError {
                operation: "stdout_read".into(),
                message: e.to_string(),
            })?;
        let stderr = stderr_task
            .await
            .map_err(|e| SandboxError::IoError {
                operation: "stderr_join".into(),
                message: e.to_string(),
            })?
            .map_err(|e| SandboxError::IoError {
                operation: "stderr_read".into(),
                message: e.to_string(),
            })?;
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
        self.append_record(&request, cwd, &result);
        Ok(result)
    }

    async fn read_file(&self, path: &str) -> SandboxResult<Vec<u8>> {
        self.guard("read_file")?;
        let p = self.resolver()?.resolve(path, None, true, "read_file")?;
        tokio::fs::read(&p)
            .await
            .map_err(|e| SandboxError::IoError {
                operation: "read_file".into(),
                message: e.to_string(),
            })
    }

    async fn write_file(&mut self, path: &str, data: &[u8]) -> SandboxResult<()> {
        self.guard("write_file")?;
        let p = self.resolver()?.resolve(path, None, false, "write_file")?;
        self.check_write_access(&p, "write_file")?;
        tokio::fs::write(&p, data)
            .await
            .map_err(|e| SandboxError::IoError {
                operation: "write_file".into(),
                message: e.to_string(),
            })
    }

    async fn delete_path(&mut self, path: &str) -> SandboxResult<()> {
        self.guard("delete_path")?;
        let resolver = self.resolver()?;
        let p = match resolver.resolve(path, None, true, "delete_path") {
            Ok(p) => p,
            Err(SandboxError::IoError { .. }) => return Ok(()),
            Err(e) => return Err(e),
        };
        self.check_write_access(&p, "delete_path")?;
        if p.is_dir() {
            tokio::fs::remove_dir_all(&p)
                .await
                .map_err(|e| SandboxError::IoError {
                    operation: "delete_path".into(),
                    message: e.to_string(),
                })?;
        } else {
            tokio::fs::remove_file(&p)
                .await
                .map_err(|e| SandboxError::IoError {
                    operation: "delete_path".into(),
                    message: e.to_string(),
                })?;
        }
        Ok(())
    }

    async fn is_dir(&self, path: &str) -> SandboxResult<bool> {
        self.guard("is_dir")?;
        match self.resolver()?.resolve(path, None, true, "is_dir") {
            Ok(p) => Ok(p.is_dir()),
            Err(SandboxError::IoError { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn stat_mtime(&self, path: &str) -> SandboxResult<Option<f64>> {
        self.guard("stat_mtime")?;
        let p = match self.resolver()?.resolve(path, None, true, "stat_mtime") {
            Ok(p) => p,
            Err(SandboxError::IoError { .. }) => return Ok(None),
            Err(e) => return Err(e),
        };
        let metadata = tokio::fs::metadata(&p)
            .await
            .map_err(|e| SandboxError::IoError {
                operation: "stat_mtime".into(),
                message: e.to_string(),
            })?;
        Ok(metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64()))
    }

    async fn list_dir(&self, path: &str, recursive: bool) -> SandboxResult<Vec<String>> {
        self.guard("list_dir")?;
        let p = match self.resolver()?.resolve(path, None, true, "list_dir") {
            Ok(p) => p,
            Err(SandboxError::IoError { .. }) => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        if !p.is_dir() {
            return Ok(Vec::new());
        }
        Self::collect_dir(p, recursive).await
    }

    async fn history(&self) -> SandboxResult<Vec<ExecutionRecord>> {
        self.guard("history")?;
        Ok(self.history.clone())
    }

    async fn capability_report(&self) -> SandboxResult<CapabilityReport> {
        Ok(CapabilityReport::local_process())
    }

    async fn close(&mut self) -> SandboxResult<()> {
        if matches!(self.state, SandboxState::Closed) {
            return Ok(());
        }
        self.state = SandboxState::Closing;
        self.closed_at = Some(Utc::now());
        self.cleanup().await
    }

    async fn cleanup(&mut self) -> SandboxResult<()> {
        if matches!(self.state, SandboxState::Closed) {
            return Ok(());
        }
        if !self.policy.keep_on_close && self.owned_temp_root && self.root_dir.exists() {
            tokio::fs::remove_dir_all(&self.root_dir)
                .await
                .map_err(|e| SandboxError::IoError {
                    operation: "cleanup".into(),
                    message: e.to_string(),
                })?;
        }
        self.state = SandboxState::Closed;
        Ok(())
    }
}
