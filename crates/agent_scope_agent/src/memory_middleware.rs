use std::sync::Arc;

use agent_scope_memory::{FileMemory, Memory, MemoryConfig, MemoryError, index::truncate_index};
use agent_scope_message::{ContentBlock, HintBlock, HintContent, Msg, Role};
use agent_scope_model::ChatModel;
use tokio::sync::Mutex;
use tracing::{debug, warn};

use crate::agent_error::AgentError;
use crate::middleware::Middleware;

type RetrievalTask = tokio::task::JoinHandle<Result<Option<String>, MemoryError>>;

pub struct MemoryMiddleware {
    memory: Arc<dyn Memory>,
    config: MemoryConfig,
    retrieval_handle: Mutex<Option<RetrievalTask>>,
    model: Mutex<Option<Arc<dyn ChatModel>>>,
}

impl MemoryMiddleware {
    pub fn new(memory: Arc<dyn Memory>, config: MemoryConfig) -> Self {
        Self {
            memory,
            config,
            retrieval_handle: Mutex::new(None),
            model: Mutex::new(None),
        }
    }

    pub fn with_config(workdir: &str, memory_dir: &str, mut config: MemoryConfig) -> Self {
        config.memory_dir = memory_dir.to_string();
        let memory: Arc<dyn Memory> = Arc::new(FileMemory::new(workdir, config.clone(), None));
        Self::new(memory, config)
    }
}

#[async_trait::async_trait]
impl Middleware for MemoryMiddleware {
    #[tracing::instrument(skip(self, current_prompt))]
    async fn on_system_prompt(
        &self,
        _agent_name: &str,
        current_prompt: &mut String,
    ) -> Result<(), AgentError> {
        debug!("memory middleware on_system_prompt start");
        let raw_index = match self.memory.get_index_content().await {
            Ok(index) if !index.trim().is_empty() => index,
            Ok(_) => "Your MEMORY.md is currently empty.".into(),
            Err(err) => {
                warn!(error = %err, "failed to read memory index");
                "Your MEMORY.md is currently empty.".into()
            }
        };

        let index = if raw_index == "Your MEMORY.md is currently empty." {
            raw_index
        } else if let Some(model) = self.model.lock().await.as_ref() {
            truncate_index(&raw_index, self.config.max_index_tokens, model.as_ref())
        } else {
            raw_index
        };

        if !current_prompt.is_empty() {
            current_prompt.push_str("\n\n");
        }
        current_prompt.push_str(&self.config.memory_instructions);
        current_prompt.push_str("\n\nMEMORY.md:\n");
        current_prompt.push_str(&index);
        debug!("memory middleware on_system_prompt end");
        Ok(())
    }

    #[tracing::instrument(skip(self, input, model))]
    async fn pre_reply(
        &self,
        _agent_name: &str,
        input: &mut Option<Vec<Msg>>,
        model: &Arc<dyn ChatModel>,
    ) -> Result<(), AgentError> {
        debug!("memory middleware pre_reply start");
        *self.model.lock().await = Some(Arc::clone(model));
        if !self.config.retrieval_async {
            return Ok(());
        }
        let query = input
            .as_ref()
            .map(|msgs| extract_user_text(msgs))
            .unwrap_or_default();
        if query.trim().is_empty() {
            return Ok(());
        }
        let memory = Arc::clone(&self.memory);
        let model = Arc::clone(model);
        let max_results = self.config.retrieval_max_files;
        let handle =
            tokio::spawn(
                async move { memory.retrieve_relevant(&query, &model, max_results).await },
            );
        *self.retrieval_handle.lock().await = Some(handle);
        debug!("memory middleware pre_reply end");
        Ok(())
    }

    #[tracing::instrument(skip(self, messages, _tools))]
    async fn pre_reasoning(
        &self,
        _agent_name: &str,
        messages: &mut Vec<Msg>,
        _tools: &mut Option<Vec<serde_json::Value>>,
    ) -> Result<(), AgentError> {
        debug!("memory middleware pre_reasoning start");
        let handle = {
            let mut guard = self.retrieval_handle.lock().await;
            if guard.as_ref().is_some_and(|handle| handle.is_finished()) {
                guard.take()
            } else {
                None
            }
        };

        let Some(handle) = handle else {
            return Ok(());
        };

        match handle.await {
            Ok(Ok(Some(content))) if !content.trim().is_empty() => inject_hint(messages, content),
            Ok(Ok(_)) => {}
            Ok(Err(err)) => warn!(error = %err, "memory retrieval task returned error"),
            Err(err) => warn!(error = %err, "memory retrieval task join failed"),
        }
        debug!("memory middleware pre_reasoning end");
        Ok(())
    }
}

fn extract_user_text(messages: &[Msg]) -> String {
    messages
        .iter()
        .filter(|msg| msg.role == Role::User)
        .filter_map(|msg| msg.get_text_content("\n"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn inject_hint(messages: &mut Vec<Msg>, content: String) {
    let hint = ContentBlock::Hint(HintBlock::new(HintContent::Text(content)));
    if let Some(last_user) = messages.iter_mut().rev().find(|msg| msg.role == Role::User) {
        last_user.content.push(hint);
        return;
    }
    if let Ok(msg) = Msg::new("memory".into(), vec![hint], Role::Assistant) {
        messages.push(msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_scope_memory::{MemoryEntry, MemoryType};
    use agent_scope_message::factory::user_msg;
    use agent_scope_model::{ChatResponse, ModelCallResult, ModelError, ToolChoice};
    use serde_json::Value as JsonValue;

    struct TestModel;

    #[async_trait::async_trait]
    impl ChatModel for TestModel {
        fn model_name(&self) -> &str {
            "test"
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
            let mut resp = ChatResponse::default();
            resp.content.push(ContentBlock::ToolCall(
                agent_scope_message::ToolCallBlock::new(
                    "tc1".into(),
                    "generate_structured_output".into(),
                    r#"{"selected_files":["auth.md"]}"#.into(),
                ),
            ));
            Ok(ModelCallResult::Complete(resp))
        }
    }

    #[tokio::test]
    async fn system_prompt_includes_instructions_and_index() {
        let dir = tempfile::tempdir().unwrap();
        let memory = Arc::new(FileMemory::new(
            dir.path().to_str().unwrap(),
            MemoryConfig::default(),
            None,
        ));
        memory
            .write(MemoryEntry::new(
                "auth",
                "Auth info",
                MemoryType::Project,
                "body",
            ))
            .await
            .unwrap();
        let mw = MemoryMiddleware::new(memory, MemoryConfig::default());
        let mut prompt = String::new();
        mw.on_system_prompt("agent", &mut prompt).await.unwrap();
        assert!(prompt.contains("MEMORY.md"));
        assert!(prompt.contains("Auth info"));
    }

    #[tokio::test]
    async fn pre_reply_noop_when_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let memory = Arc::new(FileMemory::new(
            dir.path().to_str().unwrap(),
            MemoryConfig::default(),
            None,
        ));
        let config = MemoryConfig {
            retrieval_async: false,
            ..Default::default()
        };
        let mw = MemoryMiddleware::new(memory, config);
        let mut input = Some(vec![user_msg("user", "hello").unwrap()]);
        let model: Arc<dyn ChatModel> = Arc::new(TestModel);
        mw.pre_reply("agent", &mut input, &model).await.unwrap();
        assert!(mw.retrieval_handle.lock().await.is_none());
    }

    #[tokio::test]
    async fn pre_reasoning_injects_finished_hint() {
        let dir = tempfile::tempdir().unwrap();
        let memory = Arc::new(FileMemory::new(
            dir.path().to_str().unwrap(),
            MemoryConfig::default(),
            None,
        ));
        memory
            .write(MemoryEntry::new(
                "auth",
                "Auth info",
                MemoryType::Project,
                "auth body",
            ))
            .await
            .unwrap();
        let mw = MemoryMiddleware::new(memory, MemoryConfig::default());
        let mut input = Some(vec![user_msg("user", "auth").unwrap()]);
        let model: Arc<dyn ChatModel> = Arc::new(TestModel);
        mw.pre_reply("agent", &mut input, &model).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let mut messages = input.unwrap();
        let mut tools = None;
        mw.pre_reasoning("agent", &mut messages, &mut tools)
            .await
            .unwrap();
        assert!(
            messages
                .iter()
                .any(|m| m.content.iter().any(|b| matches!(b, ContentBlock::Hint(_))))
        );
    }

    #[tokio::test]
    async fn unfinished_task_leaves_messages_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let memory = Arc::new(FileMemory::new(
            dir.path().to_str().unwrap(),
            MemoryConfig::default(),
            None,
        ));
        let mw = MemoryMiddleware::new(memory, MemoryConfig::default());
        *mw.retrieval_handle.lock().await = Some(tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            Ok(Some("late".into()))
        }));
        let mut messages = vec![user_msg("user", "hello").unwrap()];
        let mut tools = None;
        mw.pre_reasoning("agent", &mut messages, &mut tools)
            .await
            .unwrap();
        assert_eq!(messages[0].content.len(), 1);
    }
}
