//! Sandbox example: a `LocalSandboxSession` — initialize, execute a command,
//! read/write files, and inspect the capability report.
//!
//! Needs no model or API key. The sandbox scopes execution to a temporary root
//! directory and enforces path containment and command timeouts.

use agent_scope_sandbox::{
    ExecutionRequest, LocalSandboxConfig, LocalSandboxSession, SandboxSession,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().to_path_buf();

    // 1. Create and initialize the sandbox session.
    let mut sandbox = LocalSandboxSession::new(LocalSandboxConfig {
        session_id: None,
        root_dir: Some(root.clone()),
        workdir: None,
        policy: Default::default(),
        mounts: vec![],
    })?;
    sandbox.initialize().await?;
    println!("sandbox session {} initialized", sandbox.session_id());

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
        "command exit_code={:?} stdout={}",
        result.exit_code,
        String::from_utf8_lossy(&result.stdout.inline).trim()
    );

    // 3. Write and read a file inside the sandbox root.
    sandbox.write_file("note.txt", b"classified").await?;
    let data = sandbox.read_file("note.txt").await?;
    println!("wrote + read note.txt → {}", String::from_utf8_lossy(&data));

    // 4. Capability report — what this local backend can and cannot enforce.
    let report = sandbox.capability_report().await?;
    println!(
        "capability: {} (supported: {})",
        report.backend_name,
        report.supported.len()
    );

    sandbox.close().await?;
    println!("\nOK: sandbox executed a command, managed files, and reported capabilities.");
    Ok(())
}
