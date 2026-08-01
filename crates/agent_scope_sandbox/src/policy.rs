//! Sandbox policy types.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{SandboxError, SandboxResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkPolicy {
    Disabled,
    LoopbackOnly,
    Allowlist { hosts: Vec<String> },
    Unrestricted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuLimit {
    pub cpu_shares: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPolicy {
    #[serde(with = "duration_secs")]
    pub default_timeout: Duration,
    #[serde(with = "duration_secs")]
    pub max_timeout: Duration,
    pub max_output_bytes: usize,
    pub network: NetworkPolicy,
    pub writable_roots: Vec<PathBuf>,
    pub readonly_roots: Vec<PathBuf>,
    pub keep_on_close: bool,
    pub cpu_limit: Option<CpuLimit>,
    pub memory_limit_bytes: Option<u64>,
    pub process_limit: Option<u32>,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(30),
            max_timeout: Duration::from_secs(300),
            max_output_bytes: 1024 * 1024,
            network: NetworkPolicy::Unrestricted,
            writable_roots: Vec::new(),
            readonly_roots: Vec::new(),
            keep_on_close: false,
            cpu_limit: None,
            memory_limit_bytes: None,
            process_limit: None,
        }
    }
}

impl SandboxPolicy {
    pub fn validate(&self) -> SandboxResult<()> {
        if self.default_timeout > self.max_timeout {
            return Err(SandboxError::ValidationError {
                message: "default_timeout must be <= max_timeout".into(),
            });
        }
        if self.max_output_bytes == 0 {
            return Err(SandboxError::ValidationError {
                message: "max_output_bytes must be > 0".into(),
            });
        }
        Ok(())
    }

    pub fn requested_unsupported_features(&self) -> Vec<(&'static str, &'static str)> {
        let mut features = Vec::new();
        if self.cpu_limit.is_some() {
            features.push((
                "cpu_limit",
                "local-process backend cannot enforce CPU limits",
            ));
        }
        if self.memory_limit_bytes.is_some() {
            features.push((
                "memory_limit",
                "local-process backend cannot enforce memory limits",
            ));
        }
        if self.process_limit.is_some() {
            features.push((
                "process_limit",
                "local-process backend cannot enforce process limits",
            ));
        }
        if !matches!(self.network, NetworkPolicy::Unrestricted) {
            features.push((
                "network_policy",
                "local-process backend cannot enforce network isolation; use NetworkPolicy::Unrestricted to acknowledge host networking",
            ));
        }
        features
    }
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
