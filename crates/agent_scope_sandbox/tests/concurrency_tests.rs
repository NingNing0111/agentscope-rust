use agent_scope_sandbox::{
    ExecutionRequest, LocalSandboxConfig, LocalSandboxSession, SandboxSession,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sandbox_concurrent_sessions_are_isolated() {
    let mut handles = Vec::new();
    for i in 0..20usize {
        handles.push(tokio::spawn(async move {
            let mut session = LocalSandboxSession::new(LocalSandboxConfig::default()).unwrap();
            session.initialize().await.unwrap();
            let content = format!("session-{i}");
            session
                .write_file("same.txt", content.as_bytes())
                .await
                .unwrap();
            session
                .execute(ExecutionRequest::new(["printf", "ok"]))
                .await
                .unwrap();
            let read = session.read_file("same.txt").await.unwrap();
            assert_eq!(read, content.as_bytes());
            assert_eq!(session.history().await.unwrap().len(), 1);
            session.close().await.unwrap();
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
}
