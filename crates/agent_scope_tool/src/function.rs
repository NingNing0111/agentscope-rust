//! FunctionTool — adapts an async handler function into a [`Tool`]
//! implementation.
//!
//! Uses [`schemars::JsonSchema`] to automatically derive the input JSON Schema
//! from the handler's parameter type.

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use agent_scope_message::{ToolOutput, ToolResultBlock, ToolResultState};
use futures::FutureExt;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;

use crate::tool_trait::{Tool, ToolError, ToolExecOutput};

// ---------------------------------------------------------------------------
// IntoChunk trait (T009)
// ---------------------------------------------------------------------------

/// Converts a handler return value into a [`ToolResultBlock`].
///
/// Built-in implementations:
/// - [`String`] → wrapped as `ToolOutput::Text(s)` with `state: Success` and
///   `is_last: true`.
/// - [`ToolResultBlock`] → pass-through (ensures `is_last: true`).
pub trait IntoChunk: Send + 'static {
    /// Convert `self` into a [`ToolResultBlock`].
    fn into_chunk(self) -> ToolResultBlock;
}

impl IntoChunk for String {
    fn into_chunk(self) -> ToolResultBlock {
        ToolResultBlock {
            id: uuid::Uuid::new_v4().as_simple().to_string(),
            name: String::new(),
            output: ToolOutput::Text(self),
            state: ToolResultState::Success,
            is_last: true,
            metadata: std::collections::HashMap::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
        }
    }
}

impl IntoChunk for ToolResultBlock {
    fn into_chunk(mut self) -> ToolResultBlock {
        self.is_last = true;
        self
    }
}

// ---------------------------------------------------------------------------
// FunctionToolHandler — internal type-erased handler trait (T010)
// ---------------------------------------------------------------------------

/// Type-erased async handler.  Each concrete handler type produces an
/// implementation of this trait so that [`FunctionTool`] can store handlers
/// with different type parameters in a single [`Box`].
trait FunctionToolHandler: Send + Sync {
    /// Execute the handler with JSON input.
    fn call(
        &self,
        input: JsonValue,
    ) -> Pin<Box<dyn Future<Output = Result<ToolExecOutput, ToolError>> + Send + '_>>;
}

// ---------------------------------------------------------------------------
// HandlerImpl — concrete handler wrapper (T011, T013)
// ---------------------------------------------------------------------------

/// Private implementation of [`FunctionToolHandler`] that stores the actual
/// handler closure and deserializes input via `serde_json`.
struct HandlerImpl<F, T, Fut, R> {
    handler: F,
    tool_name: String,
    _phantom: PhantomData<fn(T, Fut) -> R>,
}

impl<F, T, Fut, R> HandlerImpl<F, T, Fut, R> {
    fn new(handler: F, tool_name: String) -> Self {
        Self {
            handler,
            tool_name,
            _phantom: PhantomData,
        }
    }
}

impl<F, T, Fut, R> FunctionToolHandler for HandlerImpl<F, T, Fut, R>
where
    T: DeserializeOwned + Send + 'static,
    F: Fn(T) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send,
    R: IntoChunk,
{
    fn call(
        &self,
        input: JsonValue,
    ) -> Pin<Box<dyn Future<Output = Result<ToolExecOutput, ToolError>> + Send + '_>> {
        let tool_name = self.tool_name.clone();

        Box::pin(async move {
            // Deserialize input JSON → T
            let typed: T = serde_json::from_value(input).map_err(|e| ToolError::InvalidInput {
                tool_name: tool_name.clone(),
                reason: e.to_string(),
            })?;

            // Execute handler; catch panics via futures::FutureExt::catch_unwind
            let fut = (self.handler)(typed);
            let result = std::panic::AssertUnwindSafe(fut).catch_unwind().await;

            match result {
                Ok(output) => {
                    let mut chunk = output.into_chunk();
                    chunk.name = tool_name.clone();
                    Ok(ToolExecOutput::Complete(chunk))
                }
                Err(_panic) => Err(ToolError::Execution {
                    tool_name,
                    reason: "handler panicked".to_string(),
                }),
            }
        })
    }
}

// ---------------------------------------------------------------------------
// FunctionTool struct (T010 continued)
// ---------------------------------------------------------------------------

/// Adapts an async handler function into a [`Tool`].
///
/// # Type parameters (used by the constructor, erased internally)
///
/// * `T` — Input type; must implement [`JsonSchema`] and [`Deserialize`].
/// * `F` — Handler closure type.
/// * `R` — Return type implementing [`IntoChunk`].
///
/// # Examples
///
/// ```rust
/// use agent_scope_tool::FunctionTool;
/// use schemars::JsonSchema;
/// use serde::Deserialize;
///
/// #[derive(Debug, Clone, Deserialize, JsonSchema)]
/// struct SearchInput { query: String }
///
/// async fn search(input: SearchInput) -> String {
///     format!("Results for: {}", input.query)
/// }
///
/// let tool = FunctionTool::new("search", "Search the web", search);
/// ```
pub struct FunctionTool {
    name: String,
    description: String,
    input_schema: JsonValue,
    handler: Box<dyn FunctionToolHandler>,
}

impl FunctionTool {
    /// Creates a [`FunctionTool`] with a schema automatically derived from `T`
    /// via [`schemars::schema_for!`].
    ///
    /// # Type Parameters
    /// * `T` — Input type: `schemars::JsonSchema` + `Deserialize<'de>` +
    ///   `Send + 'static`.  Must be an owned type (not a reference).
    /// * `F` — Handler function `Fn(T) -> Fut`, `Send + Sync + 'static`.
    /// * `R` — Return type implementing [`IntoChunk`].
    pub fn new<F, Fut, T, R>(
        name: impl Into<String>,
        description: impl Into<String>,
        handler: F,
    ) -> Self
    where
        T: JsonSchema + DeserializeOwned + Send + 'static,
        F: Fn(T) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoChunk + Sync,
    {
        let schema = schemars::schema_for!(T);
        let schema_value = serde_json::to_value(&schema)
            .expect("schemars RootSchema should serialize to serde_json::Value");

        let name = name.into();
        Self {
            input_schema: schema_value,
            handler: Box::new(HandlerImpl::new(handler, name.clone())),
            name,
            description: description.into(),
        }
    }

    /// Creates a [`FunctionTool`] with a manually-provided JSON Schema (escape
    /// hatch).
    ///
    /// The caller is responsible for ensuring the schema matches the handler's
    /// expected input.  No validation is performed.
    pub fn new_with_schema<F, Fut, R>(
        name: impl Into<String>,
        description: impl Into<String>,
        schema: JsonValue,
        handler: F,
    ) -> Self
    where
        F: Fn(JsonValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoChunk + Sync,
    {
        // new_with_schema uses a different HandlerImpl pattern: it deserializes
        // JsonValue → JsonValue (identity) inside the handler wrapper.
        struct RawHandlerImpl<F, Fut, R> {
            handler: F,
            tool_name: String,
            _phantom: PhantomData<fn(Fut, R)>,
        }

        impl<F, Fut, R> FunctionToolHandler for RawHandlerImpl<F, Fut, R>
        where
            F: Fn(JsonValue) -> Fut + Send + Sync + 'static,
            Fut: Future<Output = R> + Send + 'static,
            R: IntoChunk + Sync,
        {
            fn call(
                &self,
                input: JsonValue,
            ) -> Pin<Box<dyn Future<Output = Result<ToolExecOutput, ToolError>> + Send + '_>>
            {
                let tool_name = self.tool_name.clone();

                Box::pin(async move {
                    let fut = (self.handler)(input);
                    let result = std::panic::AssertUnwindSafe(fut).catch_unwind().await;

                    match result {
                        Ok(output) => {
                            let mut chunk = output.into_chunk();
                            chunk.name = tool_name.clone();
                            Ok(ToolExecOutput::Complete(chunk))
                        }
                        Err(_panic) => Err(ToolError::Execution {
                            tool_name,
                            reason: "handler panicked".to_string(),
                        }),
                    }
                })
            }
        }

        let name = name.into();
        Self {
            input_schema: schema,
            handler: Box::new(RawHandlerImpl {
                handler,
                tool_name: name.clone(),
                _phantom: PhantomData,
            }),
            name,
            description: description.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool trait impl for FunctionTool (T013 continued)
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl Tool for FunctionTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> JsonValue {
        self.input_schema.clone()
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        self.handler.call(input).await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    /// Input struct used across multiple tests.
    #[derive(Debug, Clone, Deserialize, JsonSchema)]
    struct SearchInput {
        query: String,
        #[serde(default)]
        max_results: Option<usize>,
    }

    // -- T015: name & description --
    #[tokio::test]
    async fn test_name_and_description() {
        async fn handler(_input: SearchInput) -> String {
            "ok".into()
        }

        let tool = FunctionTool::new("web_search", "Search the web", handler);
        assert_eq!(tool.name(), "web_search");
        assert_eq!(tool.description(), "Search the web");
    }

    // -- T016: input_schema format --
    #[tokio::test]
    async fn test_input_schema_is_valid_json_schema() {
        async fn handler(_input: SearchInput) -> String {
            "ok".into()
        }

        let tool = FunctionTool::new("search", "desc", handler);
        let schema = tool.input_schema();

        // Root must be "object" (schemars wraps in RootSchema, extract inner)
        // schemars::schema_for! produces {"$ref": "#/definitions/...",
        // "definitions": {...}}  We check the definitions entry.
        assert!(schema.is_object(), "schema should be an object: {schema:?}");
        // The top-level fields from schemars
        assert!(
            schema.get("$ref").is_some() || schema.get("properties").is_some(),
            "schema should have $ref or properties"
        );
    }

    // -- T017: call() returns Complete with Text --
    #[tokio::test]
    async fn test_call_returns_complete_text() {
        async fn handler(input: SearchInput) -> String {
            format!(
                "Results for '{}': found {} items",
                input.query,
                input.max_results.unwrap_or(5)
            )
        }

        let tool = FunctionTool::new("search", "desc", handler);
        let result = tool
            .call(serde_json::json!({"query": "rust", "max_results": 3}))
            .await
            .unwrap();

        match result {
            ToolExecOutput::Complete(chunk) => {
                assert_eq!(chunk.state, ToolResultState::Success);
                assert!(chunk.is_last);
                assert_eq!(chunk.name, "search");
                match &chunk.output {
                    ToolOutput::Text(text) => {
                        assert!(text.contains("Results for 'rust'"));
                        assert!(text.contains("3 items"));
                    }
                    _ => panic!("Expected ToolOutput::Text"),
                }
            }
            _ => panic!("Expected ToolExecOutput::Complete"),
        }
    }

    // -- T018: handler returns ToolResultBlock → direct passthrough --
    #[tokio::test]
    async fn test_handler_returns_tool_result_block_passthrough() {
        async fn handler(_input: SearchInput) -> ToolResultBlock {
            ToolResultBlock {
                id: "custom-id".into(),
                name: "custom-name".into(),
                output: ToolOutput::Text("custom output".into()),
                state: ToolResultState::Success,
                is_last: false,
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().to_rfc3339(),
                finished_at: None,
            }
        }

        let tool = FunctionTool::new("passthrough", "desc", handler);
        let result = tool
            .call(serde_json::json!({"query": "test"}))
            .await
            .unwrap();

        match result {
            ToolExecOutput::Complete(chunk) => {
                // is_last is forced to true by IntoChunk
                assert!(chunk.is_last);
                assert_eq!(chunk.name, "passthrough");
                match &chunk.output {
                    ToolOutput::Text(text) => assert_eq!(text, "custom output"),
                    _ => panic!("Expected Text output"),
                }
            }
            _ => panic!("Expected Complete"),
        }
    }

    // -- T019: new_with_schema --
    #[tokio::test]
    async fn test_new_with_schema() {
        async fn handler(input: JsonValue) -> String {
            format!("got: {}", input)
        }

        let custom_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "x": {"type": "integer"}
            }
        });

        let tool = FunctionTool::new_with_schema(
            "custom",
            "Custom schema tool",
            custom_schema.clone(),
            handler,
        );

        assert_eq!(tool.name(), "custom");
        assert_eq!(tool.input_schema(), custom_schema);

        let result = tool.call(serde_json::json!({"x": 42})).await.unwrap();
        match result {
            ToolExecOutput::Complete(chunk) => match &chunk.output {
                ToolOutput::Text(text) => {
                    assert!(text.contains("got:"));
                }
                _ => panic!("Expected Text"),
            },
            _ => panic!("Expected Complete"),
        }
    }

    // -- T020: handler panic → ToolError::Execution --
    #[tokio::test]
    async fn test_handler_panic_becomes_execution_error() {
        async fn panicking(_input: SearchInput) -> String {
            panic!("intentional test panic");
        }

        let tool = FunctionTool::new("panic_test", "Will panic", panicking);
        let result = tool.call(serde_json::json!({"query": "test"})).await;

        match result {
            Err(ToolError::Execution { tool_name, reason }) => {
                assert_eq!(tool_name, "panic_test");
                assert!(reason.contains("panicked"));
            }
            other => panic!("Expected ToolError::Execution, got: {other:?}"),
        }
    }

    // -- T021: invalid JSON input → ToolError::InvalidInput --
    #[tokio::test]
    async fn test_invalid_input_type_returns_error() {
        async fn handler(_input: SearchInput) -> String {
            "never reached".into()
        }

        let tool = FunctionTool::new("search", "desc", handler);
        // Missing required "query" field
        let result = tool.call(serde_json::json!({"bad_field": 1})).await;

        match result {
            Err(ToolError::InvalidInput {
                tool_name,
                reason: _,
            }) => {
                assert_eq!(tool_name, "search");
            }
            other => panic!("Expected ToolError::InvalidInput, got: {other:?}"),
        }
    }

    // -- is_concurrency_safe / is_read_only defaults --
    #[tokio::test]
    async fn test_default_concurrency_and_read_only_flags() {
        async fn handler(_input: SearchInput) -> String {
            "x".into()
        }

        let tool = FunctionTool::new("t", "d", handler);
        assert!(tool.is_concurrency_safe());
        assert!(!tool.is_read_only());
    }
}
