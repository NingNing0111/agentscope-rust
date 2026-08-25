//! Session trait and lifecycle types.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::capability::CapabilityReport;
use crate::error::SandboxError;
use crate::execution::{ExecutionRecord, ExecutionRequest, ExecutionResult};
use crate::mount::SandboxMount;
use crate::policy::SandboxPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SandboxState {
    Created,
    Ready,
    Closing,
    Closed,
    Failed,
}

#[derive(Debug, Clone, Default)]
pub struct LocalSandboxConfig {
    pub session_id: Option<String>,
    pub root_dir: Option<PathBuf>,
    pub workdir: Option<PathBuf>,
    pub policy: SandboxPolicy,
    pub mounts: Vec<SandboxMount>,
}

#[async_trait]
pub trait SandboxSession: Send + Sync {
    fn session_id(&self) -> &str;
    fn state(&self) -> SandboxState;
    fn policy(&self) -> &SandboxPolicy;

    async fn initialize(&mut self) -> Result<(), SandboxError>;
    async fn execute(&mut self, request: ExecutionRequest)
    -> Result<ExecutionResult, SandboxError>;
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, SandboxError>;
    async fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), SandboxError>;
    async fn delete_path(&mut self, path: &str) -> Result<(), SandboxError>;
    async fn is_dir(&self, path: &str) -> Result<bool, SandboxError>;
    async fn path_exists(&self, path: &str) -> Result<bool, SandboxError>;
    async fn stat_mtime(&self, path: &str) -> Result<Option<f64>, SandboxError>;
    async fn list_dir(&self, path: &str, recursive: bool) -> Result<Vec<String>, SandboxError>;
    async fn history(&self) -> Result<Vec<ExecutionRecord>, SandboxError>;
    async fn capability_report(&self) -> Result<CapabilityReport, SandboxError>;
    async fn close(&mut self) -> Result<(), SandboxError>;
    async fn cleanup(&mut self) -> Result<(), SandboxError>;
}
