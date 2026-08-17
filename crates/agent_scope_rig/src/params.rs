//! `RigParameters` → rig `CompletionRequest` 参数映射（T008）。
//!
//! 契约见 `specs/034-rig-llm-integration/contracts/rig-mapping.md` §3。
//! `CompletionRequest` 其余字段（model/preamble/chat_history/documents/
//! output_schema/record_telemetry_content）由 `RigChatModel::call_api` 按需填充，
//! 本模块只负责参数层。

use rig::completion::CompletionRequest;

/// 生成参数（全部可选；`None` → 不设置对应字段）。
///
/// `top_p`/`top_k`/`seed`/`stop`/`thinking_budget` 经 `additional_params` 透传
/// （provider 支持时）；`thinking_budget` 以顶层键 `"thinking_budget"` 写入，
/// Anthropic/DeepSeek 后端在需要时读取并转换为 provider 专用结构（契约 §3）。
#[derive(Debug, Clone, Default)]
pub struct RigParameters {
    /// 生成 token 上限 → 顶层 `max_tokens`。
    pub max_tokens: Option<u64>,
    /// 采样温度 → 顶层 `temperature`。
    pub temperature: Option<f64>,
    /// nucleus 采样 → `additional_params["top_p"]`。
    pub top_p: Option<f64>,
    /// top-k 采样 → `additional_params["top_k"]`。
    pub top_k: Option<u64>,
    /// 随机种子 → `additional_params["seed"]`。
    pub seed: Option<u64>,
    /// 停止序列 → `additional_params["stop"]`。
    pub stop: Option<Vec<String>>,
    /// 思考 token 预算 → `additional_params["thinking_budget"]`。
    pub thinking_budget: Option<u64>,
    /// 用户兜底参数，合并进 `additional_params`（本模块生成参数优先覆盖同键）。
    pub additional_params: Option<serde_json::Value>,
}

impl RigParameters {
    /// 空参数集。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置 `max_tokens`。
    pub fn with_max_tokens(mut self, v: u64) -> Self {
        self.max_tokens = Some(v);
        self
    }

    /// 设置 `temperature`。
    pub fn with_temperature(mut self, v: f64) -> Self {
        self.temperature = Some(v);
        self
    }

    /// 设置 `top_p`。
    pub fn with_top_p(mut self, v: f64) -> Self {
        self.top_p = Some(v);
        self
    }

    /// 设置 `top_k`。
    pub fn with_top_k(mut self, v: u64) -> Self {
        self.top_k = Some(v);
        self
    }

    /// 设置 `seed`。
    pub fn with_seed(mut self, v: u64) -> Self {
        self.seed = Some(v);
        self
    }

    /// 设置 `stop` 序列。
    pub fn with_stop(mut self, v: Vec<String>) -> Self {
        self.stop = Some(v);
        self
    }

    /// 设置 `thinking_budget`。
    pub fn with_thinking_budget(mut self, v: u64) -> Self {
        self.thinking_budget = Some(v);
        self
    }

    /// 设置用户兜底 `additional_params`。
    pub fn with_additional_params(mut self, v: serde_json::Value) -> Self {
        self.additional_params = Some(v);
        self
    }
}

/// 把 `RigParameters` 应用到 `CompletionRequest`（契约 §3）。
///
/// 生成参数合并规则：用户兜底 `additional_params` 先并入，随后
/// `top_p`/`top_k`/`seed`/`stop`/`thinking_budget` 逐一写入（同键覆盖用户值），
/// 最终仅当非空时设置 `request.additional_params`。
pub fn apply_params(mut request: CompletionRequest, params: &RigParameters) -> CompletionRequest {
    request.max_tokens = params.max_tokens;
    request.temperature = params.temperature;

    let mut extra = serde_json::Map::new();
    if let Some(v) = params
        .additional_params
        .as_ref()
        .and_then(|v| v.as_object())
    {
        for (k, val) in v {
            extra.insert(k.clone(), val.clone());
        }
    }
    if let Some(v) = params.top_p {
        extra.insert("top_p".to_string(), serde_json::json!(v));
    }
    if let Some(v) = params.top_k {
        extra.insert("top_k".to_string(), serde_json::json!(v));
    }
    if let Some(v) = params.seed {
        extra.insert("seed".to_string(), serde_json::json!(v));
    }
    if let Some(v) = &params.stop {
        extra.insert("stop".to_string(), serde_json::json!(v));
    }
    if let Some(v) = params.thinking_budget {
        extra.insert("thinking_budget".to_string(), serde_json::json!(v));
    }
    if !extra.is_empty() {
        request.additional_params = Some(serde_json::Value::Object(extra));
    }
    request
}
