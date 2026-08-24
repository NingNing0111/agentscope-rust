# Data Model: Microsandbox Sandbox Backend

**Feature**: `035-microsandbox-sandbox-backend`
**Date**: 2026-08-24

## Overview

Feature 035 adds a feature-gated microsandbox backend inside `agent_scope_sandbox`. The public data model remains the existing sandbox model: `SandboxSession`, `SandboxPolicy`, `ExecutionRequest`, `ExecutionResult`, `ExecutionRecord`, `SandboxError`, and `CapabilityReport`. The new backend adds `MicrosandboxConfig` and `MicrosandboxSession` without exposing microsandbox SDK handle types in the public API.

## `MicrosandboxConfig`

```rust
#[derive(Debug, Clone)]
pub struct MicrosandboxConfig {
    pub session_id: Option<String>,
    pub image: String,
    pub workdir: String,
    pub policy: SandboxPolicy,
    pub mounts: Vec<SandboxMount>,
    pub env: std::collections::HashMap<String, String>,
    pub replace_existing: bool,
    pub persist: bool,
    pub startup_timeout: std::time::Duration,
    pub stop_timeout: std::time::Duration,
}
```

### Validation

- `session_id`, when present, must be non-empty and contain only ASCII alphanumeric characters, `_`, or `-`.
- `image` must not be empty.
- `workdir` must not be empty and must be a guest path, not a host credential path.
- `startup_timeout` and `stop_timeout` must be non-zero.
- `policy.validate()` must pass before SDK creation.
- `env` stores only non-secret values. Errors, history, logs, and command summaries may include env keys but must not include env values.

## `MicrosandboxSession`

`MicrosandboxSession` implements `SandboxSession` and owns:

- stable `session_id`
- validated `MicrosandboxConfig`
- current `SandboxState`
- hidden microsandbox SDK handle
- execution history (`Vec<ExecutionRecord>`)
- monotonic execution sequence
- output-reference storage metadata

The session starts in `SandboxState::Created`. `initialize()` creates the microVM and transitions to `Ready`. Runtime/platform/SDK failures transition to `Failed` and return `SandboxError::SandboxUnavailable { backend: "microsandbox", reason }`. The implementation must not create or run `LocalSandboxSession` as fallback.

## Policy Mapping

Policy mapping is microsandbox-specific and must not use local-process `requested_unsupported_features()`.

| `SandboxPolicy` field | Microsandbox behavior |
|----------------------|-----------------------|
| `default_timeout` / `max_timeout` | Enforced by Rust layer and SDK timeout if available |
| `max_output_bytes` | Enforced by Rust output summarization |
| `network = Disabled` | Map to SDK no-network policy |
| `network = Unrestricted` | Map only when explicitly configured |
| `network = LoopbackOnly` | `UnsupportedFeature` unless SDK has exact support |
| `network = Allowlist` | `UnsupportedFeature` unless SDK has exact host allowlist support |
| `memory_limit_bytes` | Round up to MiB and map to SDK memory limit |
| `cpu_limit.cpu_shares` | `UnsupportedFeature`; not equivalent to vCPU count |
| `process_limit` | `UnsupportedFeature` until exact support exists |

## Guest Path Model

Guest paths are not host paths. The microsandbox backend must use a guest path sanitizer that:

- rejects empty paths;
- rejects any `..` component;
- treats relative paths as relative to config `workdir`;
- rejects unauthorized absolute paths outside the guest workdir or declared mounts;
- never canonicalizes guest paths through the host filesystem.

## Capability Report

`CapabilityReport::microsandbox()` reports backend name `microsandbox`, a higher compatibility level than local-process, supported command execution/output/history/workspace capabilities, and known deviations for unsupported secret injection, CPU shares, process limit, loopback-only, and host allowlist behavior.

`CapabilityReport::local_process()` must remain unchanged.

## Execution Records

`MicrosandboxSession` uses existing `ExecutionRecord` semantics:

- non-zero command exit codes are `ExecutionStatus::Exited { code }`;
- system/runtime failures are sandbox errors;
- stdout/stderr inline data is capped by `policy.max_output_bytes`;
- full output references include byte count and SHA-256;
- command summaries use `redacted_command_summary()` so env values are not recorded.
