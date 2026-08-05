//! Local sandbox session implementation.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
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

/// Upper bound for a single `read_file` call. An agent-created giant file
/// (e.g. via Bash `dd`) must not be read wholly into memory and OOM the host;
/// `policy.max_output_bytes` only bounds command output, not file reads
/// (round-4 M29).
const MAX_READ_FILE_BYTES: usize = 10 * 1024 * 1024;

/// Kill the process group led by `child_pid`. The child is spawned with
/// `.process_group(0)`, so its process-group id equals its pid; signalling the
/// negative pgid reaps the direct child *and* any grandchildren that inherited
/// its stdout/stderr pipes (e.g. `sh -c 'sleep 1000 &'`), which would otherwise
/// survive the direct child, keep the pipes open, and leak (round-4 M28).
/// Best-effort: a missing/unknown pid or a group that no longer exists is
/// silently ignored (the caller also kills the direct child).
#[cfg(unix)]
fn kill_process_group(child_pid: Option<u32>) {
    if let Some(pid) = child_pid {
        use nix::sys::signal::{Signal, killpg};
        use nix::unistd::Pid;
        let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGKILL);
    }
}

#[cfg(not(unix))]
fn kill_process_group(_child_pid: Option<u32>) {}

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
        // Defense against path traversal: `session_id` is concatenated into the
        // temp root path (`agentscope-sandbox-{session_id}`) and later removed
        // with `remove_dir_all`. Reject anything that could escape the temp
        // directory or resolve elsewhere (slashes, `..`, control chars, ...).
        if !session_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(SandboxError::ValidationError {
                message: "session_id must only contain [A-Za-z0-9_-]".into(),
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
        // Do not leak the host environment into the sandboxed child process:
        // clear it and re-inject a minimal base set plus the explicitly
        // requested env. Sandboxed code must not be able to read secrets such
        // as API keys from the parent's environment.
        cmd.env_clear();
        cmd.env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
        cmd.env("HOME", cwd.as_os_str());
        cmd.env("TMPDIR", std::env::temp_dir());
        // Put the child in its own process group so a timeout can reap the
        // whole tree. A grandchild spawned via `sh -c 'sleep 1000 &'` would
        // otherwise survive the direct child and keep the output pipes open,
        // hanging the read and leaking the process (round-4 M28).
        #[cfg(unix)]
        {
            cmd.process_group(0);
        }
        cmd.args(&request.argv[1..])
            .current_dir(&cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(if request.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            // If `execute` aborts early (timeout, stdin hang) the child is
            // dropped; make sure that kills the process instead of leaking it.
            .kill_on_drop(true);
        for (k, v) in &request.env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().map_err(|e| SandboxError::IoError {
            operation: format!("spawn {}", request.argv[0]),
            message: e.to_string(),
        })?;
        let child_pid = child.id();
        if let Some(stdin) = request.stdin.clone()
            && let Some(mut child_stdin) = child.stdin.take()
        {
            // A child that never reads stdin fills the pipe buffer and blocks
            // this write forever. Bound it with the execute deadline; on
            // timeout the future (and `child_stdin`) is dropped, which closes
            // the pipe so the child sees EOF and the wait below can proceed.
            let _ = tokio::time::timeout(timeout_duration, child_stdin.write_all(&stdin)).await;
        }

        let mut stdout_pipe = child.stdout.take();
        let mut stderr_pipe = child.stderr.take();
        // Cap the amount of output read into memory and written to
        // `.sandbox-output`: reading unbounded output (e.g. `yes`) would
        // otherwise be an in-memory / disk DoS even though `max_output_bytes`
        // only trimmed the inline copy afterwards.
        let max_output_bytes = self.policy.max_output_bytes;
        let mut stdout_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            if let Some(stdout) = stdout_pipe.take() {
                use tokio::io::AsyncReadExt;
                stdout
                    .take((max_output_bytes + 1) as u64)
                    .read_to_end(&mut bytes)
                    .await?;
            }
            Ok::<Vec<u8>, std::io::Error>(bytes)
        });
        let mut stderr_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            if let Some(stderr) = stderr_pipe.take() {
                use tokio::io::AsyncReadExt;
                stderr
                    .take((max_output_bytes + 1) as u64)
                    .read_to_end(&mut bytes)
                    .await?;
            }
            Ok::<Vec<u8>, std::io::Error>(bytes)
        });

        let wait_result = timeout(timeout_duration, child.wait()).await;
        // `status`/`exit_code` are immutable once set: the read timeouts below
        // no longer overwrite the child's real exit status (round-4 M28).
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
                // The command outlived its deadline. Reap the whole process
                // group (direct child + any grandchildren holding the output
                // pipes), then confirm the direct child is gone. If the kill is
                // refused (EPERM) or races a reaped PID, do not block forever
                // on `child.wait()`: bound the wait (round-4 M28).
                kill_process_group(child_pid);
                let _ = child.kill().await;
                let _ = tokio::time::timeout(std::time::Duration::from_secs(5), child.wait()).await;
                (
                    ExecutionStatus::TimedOut,
                    None,
                    vec![ResourceLimitHit::Timeout],
                )
            }
        };
        // A grandchild that inherited the stdout/stderr pipe keeps its write
        // end open, so `read_to_end` would never reach EOF and would block
        // forever (the previous timeout only wrapped `child.wait`). Bound the
        // read joins with the same deadline. The direct child has already been
        // waited above, so a read timeout means a grandchild is holding the
        // pipe: preserve the child's *real* exit status (it is authoritative)
        // rather than overwriting it with TimedOut, and reap the process group
        // (round-4 M28). `tokio::select!` with a `&mut` handle (rather than
        // `timeout`) keeps the JoinHandle alive so the timeout branch can abort
        // the detached read task; a grandchild holding the pipe's write end
        // would otherwise keep `read_to_end` blocked on EOF forever, leaking
        // the task and the pipe (audit S3).
        let stdout = tokio::select! {
            joined = &mut stdout_task => joined
                .map_err(|e| SandboxError::IoError {
                    operation: "stdout_join".into(),
                    message: e.to_string(),
                })?
                .map_err(|e| SandboxError::IoError {
                    operation: "stdout_read".into(),
                    message: e.to_string(),
                })?,
            _ = tokio::time::sleep(timeout_duration) => {
                hits.push(ResourceLimitHit::Timeout);
                kill_process_group(child_pid);
                let _ = child.kill().await;
                stdout_task.abort();
                Vec::new()
            }
        };
        let stderr = tokio::select! {
            joined = &mut stderr_task => joined
                .map_err(|e| SandboxError::IoError {
                    operation: "stderr_join".into(),
                    message: e.to_string(),
                })?
                .map_err(|e| SandboxError::IoError {
                    operation: "stderr_read".into(),
                    message: e.to_string(),
                })?,
            _ = tokio::time::sleep(timeout_duration) => {
                hits.push(ResourceLimitHit::Timeout);
                kill_process_group(child_pid);
                let _ = child.kill().await;
                stderr_task.abort();
                Vec::new()
            }
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
        self.append_record(&request, cwd, &result);
        Ok(result)
    }

    async fn read_file(&self, path: &str) -> SandboxResult<Vec<u8>> {
        self.guard("read_file")?;
        let p = self.resolver()?.resolve(path, None, true, "read_file")?;
        let file = tokio::fs::File::open(&p)
            .await
            .map_err(|e| SandboxError::IoError {
                operation: "read_file".into(),
                message: e.to_string(),
            })?;
        let meta = file.metadata().await.map_err(|e| SandboxError::IoError {
            operation: "read_file".into(),
            message: e.to_string(),
        })?;
        // Refuse oversized files up-front so we never allocate them (round-4 M29).
        if meta.len() > MAX_READ_FILE_BYTES as u64 {
            return Err(SandboxError::ValidationError {
                message: format!(
                    "read_file: '{}' is {} bytes, exceeding the {} byte cap",
                    path,
                    meta.len(),
                    MAX_READ_FILE_BYTES
                ),
            });
        }
        use tokio::io::AsyncReadExt;
        let mut buf = Vec::with_capacity(meta.len() as usize);
        file.take((MAX_READ_FILE_BYTES + 1) as u64)
            .read_to_end(&mut buf)
            .await
            .map_err(|e| SandboxError::IoError {
                operation: "read_file".into(),
                message: e.to_string(),
            })?;
        Ok(buf)
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
        // Refuse to delete the sandbox root or its workdir — a request like
        // `delete_path("/")` resolves to the root and would recursively wipe
        // the whole sandbox (audit S9).
        let root = resolver.root_dir();
        if p == root || p == resolver.workdir() {
            return Err(SandboxError::PermissionDenied {
                path: Some(p.display().to_string()),
                operation: "delete_path".into(),
            });
        }
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
