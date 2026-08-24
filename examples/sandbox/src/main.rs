//! Sandbox example: initialize, execute a command, read/write files, and inspect
//! capability reports for the sandbox backends.
//!
//! The default path uses `LocalSandboxSession`, which needs no model, API key, or
//! external runtime. When compiled with the `microsandbox` feature, this
//! example also includes an opt-in microsandbox path guarded by
//! `AGENTSCOPE_RUN_MICROSANDBOX_EXAMPLE=1` so normal example runs never require a
//! real microsandbox runtime.

use agent_scope_sandbox::{
    ExecutionRequest, LocalSandboxConfig, LocalSandboxSession, SandboxSession,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_local_sandbox().await?;

    #[cfg(feature = "microsandbox")]
    run_microsandbox_if_requested().await?;

    #[cfg(not(feature = "microsandbox"))]
    println!("\nmicrosandbox path not compiled; enable with `--features microsandbox`.");

    Ok(())
}

async fn run_local_sandbox() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().to_path_buf();

    // 1. Create and initialize the local-process reference sandbox session.
    let mut sandbox = LocalSandboxSession::new(LocalSandboxConfig {
        session_id: None,
        root_dir: Some(root.clone()),
        workdir: None,
        policy: Default::default(),
        mounts: vec![],
    })?;
    sandbox.initialize().await?;
    println!("local sandbox session {} initialized", sandbox.session_id());

    // 2. Execute a command inside the sandbox.
    let result = sandbox
        .execute(ExecutionRequest {
            argv: vec!["echo".into(), "hello from sandbox".into()],
            cwd: None,
            env: Default::default(),
            timeout: None,
            stdin: None,
        })
        .await?;
    println!(
        "local command exit_code={:?} stdout={}",
        result.exit_code,
        String::from_utf8_lossy(&result.stdout.inline).trim()
    );

    // 3. Write and read a file inside the sandbox root.
    sandbox.write_file("note.txt", b"classified").await?;
    let data = sandbox.read_file("note.txt").await?;
    println!(
        "local wrote + read note.txt → {}",
        String::from_utf8_lossy(&data)
    );

    // 4. Capability report — what this local backend can and cannot enforce.
    let report = sandbox.capability_report().await?;
    println!(
        "local capability: {} (supported: {})",
        report.backend_name,
        report.supported.len()
    );

    sandbox.close().await?;
    println!("OK: local sandbox executed a command, managed files, and reported capabilities.");
    Ok(())
}

#[cfg(feature = "microsandbox")]
async fn run_microsandbox_if_requested() -> anyhow::Result<()> {
    use std::path::PathBuf;
    use std::time::Duration;

    use agent_scope_sandbox::{
        MicrosandboxConfig, MountAccess, MountOwner, NetworkPolicy, SandboxMount, SandboxPolicy,
    };

    if std::env::var("AGENTSCOPE_RUN_MICROSANDBOX_EXAMPLE").as_deref() != Ok("1") {
        println!(
            "\nmicrosandbox path compiled but skipped; set AGENTSCOPE_RUN_MICROSANDBOX_EXAMPLE=1 to run it."
        );
        return Ok(());
    }

    let workspace = tempfile::tempdir()?;
    std::fs::write(
        workspace.path().join("input.txt"),
        b"hello from host workspace",
    )?;

    let mut sandbox = agent_scope_sandbox::MicrosandboxSession::new(MicrosandboxConfig {
        image: std::env::var("AGENTSCOPE_MICROSANDBOX_IMAGE").unwrap_or_else(|_| "python".into()),
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
            default_timeout: Duration::from_secs(10),
            max_timeout: Duration::from_secs(30),
            max_output_bytes: 1024,
            ..Default::default()
        },
        ..Default::default()
    })?;
    sandbox.initialize().await?;
    println!("microsandbox session {} initialized", sandbox.session_id());

    let result = sandbox
        .execute(ExecutionRequest {
            argv: vec![
                "python".into(),
                "-c".into(),
                "from pathlib import Path; print(Path.cwd()); print(Path('input.txt').read_text())"
                    .into(),
            ],
            cwd: None,
            env: Default::default(),
            timeout: Some(Duration::from_secs(10)),
            stdin: None,
        })
        .await?;
    println!(
        "microsandbox command exit_code={:?} stdout={}",
        result.exit_code,
        String::from_utf8_lossy(&result.stdout.inline).trim()
    );

    sandbox.close().await?;
    println!("OK: microsandbox backend ran with network disabled by explicit policy.");
    Ok(())
}
