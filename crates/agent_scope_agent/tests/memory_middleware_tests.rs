use std::sync::{Arc, Mutex};

use agent_scope_agent::{
    Agent, AgentConfig, ContextConfig, MemoryMiddleware, ReActAgent, ReActConfig,
};
use agent_scope_memory::{FileMemory, Memory, MemoryConfig, MemoryEntry, MemoryType};
use agent_scope_message::{Msg, factory::user_msg};
use agent_scope_model::{ChatModel, ChatResponse, ModelCallResult, ModelError, ToolChoice};
use serde_json::Value as JsonValue;

struct InspectModel {
    seen: Arc<Mutex<Vec<Vec<Msg>>>>,
}

#[async_trait::async_trait]
impl ChatModel for InspectModel {
    fn model_name(&self) -> &str {
        "inspect"
    }

    fn stream_enabled(&self) -> bool {
        false
    }

    async fn call_api(
        &self,
        _model: &str,
        messages: &[Msg],
        _tools: Option<&[JsonValue]>,
        _tool_choice: Option<&ToolChoice>,
    ) -> Result<ModelCallResult, ModelError> {
        self.seen.lock().unwrap().push(messages.to_vec());
        let mut response = ChatResponse::default();
        response.append_text("ok", None);
        Ok(ModelCallResult::Complete(response))
    }
}

#[tokio::test]
async fn react_agent_injects_memory_system_prompt_before_model_call() {
    let dir = tempfile::tempdir().unwrap();
    let memory = Arc::new(FileMemory::new(
        dir.path().to_str().unwrap(),
        MemoryConfig::default(),
        None,
    ));
    memory
        .write(MemoryEntry::new(
            "auth",
            "Authentication project note",
            MemoryType::Project,
            "OAuth callback fails",
        ))
        .await
        .unwrap();

    let seen = Arc::new(Mutex::new(Vec::new()));
    let model: Arc<dyn ChatModel> = Arc::new(InspectModel { seen: seen.clone() });
    let memory_dyn: Arc<dyn Memory> = memory;
    let middleware = Arc::new(MemoryMiddleware::new(memory_dyn, MemoryConfig::default()));
    let config = AgentConfig::builder()
        .name("agent")
        .system_prompt("base prompt")
        .model(model)
        .build()
        .unwrap();
    let agent = ReActAgent::new(
        config,
        ReActConfig::default(),
        ContextConfig::default(),
        vec![middleware],
    )
    .unwrap();

    let _ = agent
        .reply(Some(vec![user_msg("user", "hello").unwrap()]))
        .await
        .unwrap();
    let calls = seen.lock().unwrap();
    let first_call = calls.first().expect("model should be called");
    let system_text = first_call[0].get_text_content("\n").unwrap();
    assert!(system_text.contains("base prompt"));
    assert!(system_text.contains("MEMORY.md"));
    assert!(system_text.contains("Authentication project note"));
}
