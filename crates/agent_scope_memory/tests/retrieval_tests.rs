use agent_scope_memory::{FileMemory, Memory, MemoryConfig, MemoryEntry, MemoryType};
use agent_scope_message::{Msg, ToolCallBlock};
use agent_scope_model::{ChatResponse, ModelCallResult, ModelError, ToolChoice};
use serde_json::Value as JsonValue;
use std::sync::Arc;

struct SelectionModel {
    files: Vec<String>,
    fail: bool,
}

#[async_trait::async_trait]
impl agent_scope_model::ChatModel for SelectionModel {
    fn model_name(&self) -> &str {
        "selection"
    }
    fn stream_enabled(&self) -> bool {
        false
    }
    async fn call_api(
        &self,
        _: &str,
        _: &[Msg],
        _: Option<&[JsonValue]>,
        _: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError> {
        if self.fail {
            return Err(ModelError::StructuredOutputError {
                reason: "fail".into(),
            });
        }
        let json = serde_json::json!({ "selected_files": self.files }).to_string();
        let mut resp = ChatResponse::default();
        resp.content
            .push(agent_scope_message::ContentBlock::ToolCall(
                ToolCallBlock::new("tc1".into(), "generate_structured_output".into(), json),
            ));
        Ok(ModelCallResult::Complete(resp))
    }
}

async fn setup_memory() -> (tempfile::TempDir, FileMemory) {
    let dir = tempfile::tempdir().unwrap();
    let memory = FileMemory::new(dir.path().to_str().unwrap(), MemoryConfig::default(), None);
    memory
        .write(MemoryEntry::new(
            "auth",
            "Authentication bug",
            MemoryType::Project,
            "OAuth callback failing",
        ))
        .await
        .unwrap();
    memory
        .write(MemoryEntry::new(
            "deploy",
            "Deployment guide",
            MemoryType::Reference,
            "kubectl apply",
        ))
        .await
        .unwrap();
    (dir, memory)
}

#[tokio::test]
async fn valid_selection_returns_content() {
    let (_dir, memory) = setup_memory().await;
    let model: Arc<dyn agent_scope_model::ChatModel> = Arc::new(SelectionModel {
        files: vec!["auth.md".into()],
        fail: false,
    });
    let result = memory
        .retrieve_relevant("auth bug", &model, 5)
        .await
        .unwrap()
        .unwrap();
    assert!(result.contains("OAuth callback"));
    assert!(result.contains("auth"));
}

#[tokio::test]
async fn empty_and_hallucinated_selection_returns_none() {
    let (_dir, memory) = setup_memory().await;
    let model: Arc<dyn agent_scope_model::ChatModel> = Arc::new(SelectionModel {
        files: vec![],
        fail: false,
    });
    assert!(
        memory
            .retrieve_relevant("weather", &model, 5)
            .await
            .unwrap()
            .is_none()
    );

    let model: Arc<dyn agent_scope_model::ChatModel> = Arc::new(SelectionModel {
        files: vec!["missing.md".into()],
        fail: false,
    });
    assert!(
        memory
            .retrieve_relevant("auth", &model, 5)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn model_failure_is_silent_none() {
    let (_dir, memory) = setup_memory().await;
    let model: Arc<dyn agent_scope_model::ChatModel> = Arc::new(SelectionModel {
        files: vec![],
        fail: true,
    });
    assert!(
        memory
            .retrieve_relevant("auth", &model, 5)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn max_results_caps_selection_and_semantic_case() {
    let dir = tempfile::tempdir().unwrap();
    let memory = FileMemory::new(dir.path().to_str().unwrap(), MemoryConfig::default(), None);
    for i in 0..20 {
        let name = if i < 3 {
            format!("auth-{i}")
        } else {
            format!("deploy-{i}")
        };
        let desc = if i < 3 {
            "Authentication bug"
        } else {
            "Deployment note"
        };
        memory
            .write(MemoryEntry::new(
                &name,
                desc,
                MemoryType::Project,
                format!("content {name}"),
            ))
            .await
            .unwrap();
    }
    let model: Arc<dyn agent_scope_model::ChatModel> = Arc::new(SelectionModel {
        files: vec!["auth-0.md".into(), "auth-1.md".into(), "deploy-3.md".into()],
        fail: false,
    });
    let result = memory
        .retrieve_relevant("fix authentication bug", &model, 2)
        .await
        .unwrap()
        .unwrap();
    assert!(result.contains("auth-0"));
    assert!(result.contains("auth-1"));
    assert!(!result.contains("deploy-3"));
}

#[tokio::test]
async fn truncates_large_file_content() {
    let dir = tempfile::tempdir().unwrap();
    let config = MemoryConfig {
        retrieval_max_tokens_per_file: 4,
        ..Default::default()
    };
    let memory = FileMemory::new(dir.path().to_str().unwrap(), config, None);
    memory
        .write(MemoryEntry::new(
            "big",
            "Big file",
            MemoryType::Project,
            "x".repeat(200),
        ))
        .await
        .unwrap();
    let model: Arc<dyn agent_scope_model::ChatModel> = Arc::new(SelectionModel {
        files: vec!["big.md".into()],
        fail: false,
    });
    let result = memory
        .retrieve_relevant("big", &model, 5)
        .await
        .unwrap()
        .unwrap();
    assert!(result.contains("<<<TRUNCATED>>>"));
}
