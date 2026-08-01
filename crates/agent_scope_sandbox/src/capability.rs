//! Capability report types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompatibilityLevel {
    L1,
    L2,
    L3,
    L4,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SandboxCapability {
    TempRoot,
    PathContainment,
    CommandExecution,
    Timeout,
    OutputLimit,
    OutputReferences,
    AuditHistory,
    WorkspaceBackend,
    ReadOnlyMounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsupportedCapability {
    pub capability: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityReport {
    pub backend_name: String,
    pub compatibility_level: CompatibilityLevel,
    pub supported: Vec<SandboxCapability>,
    pub unsupported: Vec<UnsupportedCapability>,
    pub known_deviations: Vec<String>,
}

impl CapabilityReport {
    #[must_use]
    pub fn local_process() -> Self {
        Self {
            backend_name: "local-process".into(),
            compatibility_level: CompatibilityLevel::L2,
            supported: vec![
                SandboxCapability::TempRoot,
                SandboxCapability::CommandExecution,
                SandboxCapability::Timeout,
                SandboxCapability::OutputLimit,
                SandboxCapability::OutputReferences,
                SandboxCapability::AuditHistory,
                SandboxCapability::WorkspaceBackend,
                SandboxCapability::ReadOnlyMounts,
            ],
            unsupported: vec![
                UnsupportedCapability { capability: "network_policy".into(), reason: "local-process backend cannot enforce network namespaces; only NetworkPolicy::Unrestricted is accepted".into() },
                UnsupportedCapability { capability: "subprocess_filesystem_hard_isolation".into(), reason: "local-process commands run as host child processes with sandbox workdir; file APIs are contained, but subprocess syscalls are not chrooted or namespaced".into() },
                UnsupportedCapability { capability: "cpu_limit".into(), reason: "local-process backend cannot enforce CPU limits".into() },
                UnsupportedCapability { capability: "memory_limit".into(), reason: "local-process backend cannot enforce memory limits".into() },
                UnsupportedCapability { capability: "process_limit".into(), reason: "local-process backend cannot enforce process limits".into() },
            ],
            known_deviations: vec!["Local process backend contains Rust file APIs with canonicalized paths, but command execution is not chrooted/containerized and must not be treated as container-grade filesystem isolation".into()],
        }
    }
}
