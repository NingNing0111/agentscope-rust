//! Execution request/result and audit types.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub argv: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
    #[serde(with = "duration_opt_secs")]
    pub timeout: Option<Duration>,
    pub stdin: Option<Vec<u8>>,
}

impl ExecutionRequest {
    #[must_use]
    pub fn new(argv: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            argv: argv.into_iter().map(Into::into).collect(),
            cwd: None,
            env: HashMap::new(),
            timeout: None,
            stdin: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Exited { code: i32 },
    TimedOut,
    PermissionDenied,
    UnsupportedFeature,
    SandboxError,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceLimitHit {
    Timeout,
    OutputTruncated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputRef {
    pub path: PathBuf,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputSummary {
    pub inline: Vec<u8>,
    pub truncated: bool,
    pub full_ref: Option<OutputRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub execution_id: String,
    pub status: ExecutionStatus,
    pub exit_code: Option<i32>,
    pub stdout: OutputSummary,
    pub stderr: OutputSummary,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    #[serde(with = "duration_secs")]
    pub duration: Duration,
    pub resource_hits: Vec<ResourceLimitHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub sequence: u64,
    pub execution_id: String,
    pub command_summary: String,
    pub cwd: PathBuf,
    pub status: ExecutionStatus,
    #[serde(with = "duration_secs")]
    pub duration: Duration,
    pub failure_category: Option<String>,
    pub stdout_ref: Option<OutputRef>,
    pub stderr_ref: Option<OutputRef>,
}

#[must_use]
pub fn failure_category(status: &ExecutionStatus) -> Option<String> {
    match status {
        ExecutionStatus::Exited { code: 0 } => None,
        ExecutionStatus::Exited { .. } => Some("non_zero_exit".into()),
        ExecutionStatus::TimedOut => Some("timeout".into()),
        ExecutionStatus::PermissionDenied => Some("permission_denied".into()),
        ExecutionStatus::UnsupportedFeature => Some("unsupported_feature".into()),
        ExecutionStatus::SandboxError => Some("sandbox_error".into()),
        ExecutionStatus::Cancelled => Some("cancelled".into()),
    }
}

#[must_use]
pub fn redacted_command_summary(req: &ExecutionRequest) -> String {
    let mut parts = req.argv.clone();
    if !req.env.is_empty() {
        let mut keys: Vec<_> = req.env.keys().cloned().collect();
        keys.sort();
        parts.push(format!("env=[{}]", keys.join(",")));
    }
    parts.join(" ")
}

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;
    pub fn serialize<S>(d: &Duration, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_f64(d.as_secs_f64())
    }
    pub fn deserialize<'de, D>(d: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Duration::from_secs_f64(f64::deserialize(d)?))
    }
}
mod duration_opt_secs {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;
    pub fn serialize<S>(d: &Option<Duration>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        d.map(|v| v.as_secs_f64()).serialize(s)
    }
    pub fn deserialize<'de, D>(d: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Option::<f64>::deserialize(d)?.map(Duration::from_secs_f64))
    }
}
