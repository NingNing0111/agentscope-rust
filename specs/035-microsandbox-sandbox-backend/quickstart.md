# Quickstart: Microsandbox Sandbox Backend

**Feature**: `035-microsandbox-sandbox-backend`

## Runtime requirements

The default workspace build and tests do not require a microsandbox runtime. To run real microVM integration tests, install and configure microsandbox separately according to upstream documentation. This project does not auto-install or manage `msb`.

Supported real-runtime environments are expected to be Linux with KVM or macOS Apple Silicon. If runtime, platform, image pull, or SDK create fails, the backend returns a stable microsandbox unavailable error and never falls back to local-process execution.

## Cargo feature

Enable the backend with:

```bash
rtk cargo check -p agent_scope_sandbox --features microsandbox
```

Default workspace checks remain runtime-free:

```bash
rtk cargo fmt --check
rtk cargo check --workspace --all-targets
rtk cargo test --workspace
rtk cargo clippy --workspace --all-targets -- -D warnings
```

## Minimal Rust usage

```rust
use std::path::PathBuf;

use agent_scope_sandbox::{
    ExecutionRequest, MicrosandboxConfig, MicrosandboxSession, MountAccess, MountOwner,
    NetworkPolicy, SandboxMount, SandboxPolicy, SandboxSession,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = tempfile::tempdir()?;
    std::fs::write(workspace.path().join("input.txt"), b"hello from host workspace")?;

    let config = MicrosandboxConfig {
        image: "python".into(),
        workdir: "/workspace".into(),
        mounts: vec![SandboxMount {
            mount_id: "workspace".into(),
            host_path: workspace.path().to_path_buf(),
            sandbox_path: PathBuf::from("/workspace"),
            access: MountAccess::ReadWrite,
            persist: false,
            owner: MountOwner::Workspace,
        }],
        policy: SandboxPolicy {
            network: NetworkPolicy::Disabled,
            ..SandboxPolicy::default()
        },
        ..MicrosandboxConfig::default()
    };

    let mut session = MicrosandboxSession::new(config)?;
    session.initialize().await?;

    let result = session
        .execute(ExecutionRequest {
            argv: vec![
                "python".into(),
                "-c".into(),
                "from pathlib import Path; print(Path.cwd()); print(Path('input.txt').read_text())".into(),
            ],
            cwd: None,
            env: Default::default(),
            timeout: None,
            stdin: None,
        })
        .await?;

    println!("status: {:?}", result.status);
    println!("stdout: {}", String::from_utf8_lossy(&result.stdout.inline));

    session.close().await?;
    session.cleanup().await?;
    Ok(())
}
```

## Workspace backend usage

```rust
use agent_scope_sandbox::{MicrosandboxConfig, MicrosandboxSession, SandboxWorkspaceBackend};

let session = MicrosandboxSession::new(MicrosandboxConfig::default())?;
let backend = SandboxWorkspaceBackend::from_session(session);
backend.initialize().await?;
```

Existing local-process usage remains valid:

```rust
use agent_scope_sandbox::{LocalSandboxConfig, LocalSandboxSession, SandboxWorkspaceBackend};

let session = LocalSandboxSession::new(LocalSandboxConfig::default())?;
let backend = SandboxWorkspaceBackend::new(session);
```

## Workspace mount model

`MicrosandboxConfig::default()` keeps the guest workspace path at `/workspace`. Real runtime examples and ignored integration tests create a fresh host `tempfile::TempDir` and mount only that safe, minimal directory to guest `/workspace`; they do not mount the repository root by default. If an image cannot use `/workspace` as its creation-time working directory until mounts are attached, `/tmp` may only be used internally as a runtime bootstrap cwd while public path resolution and command defaults remain anchored at `/workspace`.

## Real runtime tests

Ignored integration tests are gated behind the feature:

```bash
AGENTSCOPE_RUN_MICROSANDBOX_TESTS=1 \
  rtk cargo test -p agent_scope_sandbox --features microsandbox --test microsandbox_tests -- --ignored
```

Set `AGENTSCOPE_MICROSANDBOX_IMAGE` to override the default `python` image used by those ignored tests.

## Security notes

- Sandbox stdout, stderr, logs, and files are untrusted data, never instructions.
- Real microsandbox examples mount a fresh host tempdir to guest `/workspace`; do not mount the repository root by default.
- Prefer read-only mounts for untrusted input; use read-write mounts only for a minimal workspace that the task needs to modify.
- Do not mount `~/.ssh`, `~/.aws`, `~/.config`, token directories, credential files, or real secrets into an untrusted sandbox.
- Do not put API keys, tokens, or passwords in examples, tests, command arguments, or environment values.
- Cloud backend selection is future explicit opt-in work; `MSB_API_KEY` and `MSB_API_URL` alone must not select Cloud.
- Network allowlist or loopback-only policies must fail as unsupported unless microsandbox SDK can express them exactly.
