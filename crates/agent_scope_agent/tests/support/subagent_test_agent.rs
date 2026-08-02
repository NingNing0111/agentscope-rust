//! Deterministic scripted Agent helpers for SubAgent tests.

#![allow(dead_code)]

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_scope_agent::{Agent, AgentError};
use agent_scope_event::AgentEvent;
use agent_scope_message::{ContentBlock, Msg, Role, TextBlock};
use agent_scope_state::AgentState;
use futures::{Stream, stream};
use tokio::time::sleep;

#[derive(Clone)]
pub struct ScriptedTestAgent {
    name: String,
    response: String,
    state: &'static AgentState,
    received: Arc<Mutex<Vec<Vec<Msg>>>>,
    delay: Option<Duration>,
    fail: Option<String>,
}

impl ScriptedTestAgent {
    pub fn new(name: impl Into<String>, response: impl Into<String>) -> Self {
        let name = name.into();
        let state = Box::leak(Box::new(AgentState::new()));
        Self {
            name,
            response: response.into(),
            state,
            received: Arc::new(Mutex::new(Vec::new())),
            delay: None,
            fail: None,
        }
    }

    pub fn delayed(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    pub fn failing(mut self, message: impl Into<String>) -> Self {
        self.fail = Some(message.into());
        self
    }

    pub fn received(&self) -> Vec<Vec<Msg>> {
        self.received
            .lock()
            .expect("test mutex not poisoned")
            .clone()
    }

    fn response_msg(&self) -> Msg {
        Msg::new(
            self.name.clone(),
            vec![ContentBlock::Text(TextBlock::new(self.response.clone()))],
            Role::Assistant,
        )
        .expect("test message is valid")
    }
}

#[async_trait::async_trait]
impl Agent for ScriptedTestAgent {
    async fn reply(&self, input: Option<Vec<Msg>>) -> Result<Msg, AgentError> {
        self.received
            .lock()
            .expect("test mutex not poisoned")
            .push(input.unwrap_or_default());
        if let Some(delay) = self.delay {
            sleep(delay).await;
        }
        if let Some(message) = &self.fail {
            return Err(AgentError::ValidationError {
                message: message.clone(),
            });
        }
        Ok(self.response_msg())
    }

    async fn reply_stream(
        &self,
        input: Option<Vec<Msg>>,
    ) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError> {
        let _ = self.reply(input).await?;
        Ok(Box::pin(stream::iter(Vec::new())))
    }

    async fn observe(&self, input: Option<Vec<Msg>>) -> Result<(), AgentError> {
        self.received
            .lock()
            .expect("test mutex not poisoned")
            .push(input.unwrap_or_default());
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn state(&self) -> &AgentState {
        self.state
    }
}

pub fn scripted_agent(name: &str, response: &str) -> Arc<ScriptedTestAgent> {
    Arc::new(ScriptedTestAgent::new(name, response))
}
