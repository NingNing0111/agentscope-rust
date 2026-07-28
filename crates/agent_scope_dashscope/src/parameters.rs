//! DashScopeParameters — model configuration for DashScope (Qwen) models.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// DashScope Chat Completions parameters with Qwen-specific extensions.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct DashScopeParameters {
    /// Maximum number of tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Sampling temperature (0–2). Higher = more random.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// Nucleus sampling: only tokens with cumulative probability < top_p are kept.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,

    /// Top-K sampling: limit to the K most probable tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,

    /// Enable web search augmentation (DashScope extension).
    #[serde(default)]
    pub enable_search: bool,

    /// Enable thinking/reasoning mode (returns `reasoning_content` in delta).
    #[serde(default)]
    pub enable_thinking: bool,

    /// Maximum token budget for thinking (only meaningful when enable_thinking=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<u32>,

    /// Repetition penalty coefficient. Valid range: (0, +∞). 1.0 = no penalty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repetition_penalty: Option<f64>,

    /// Random seed for reproducible output. Valid range: [0, 2^31-1].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,

    /// Stop sequences. Generation stops when any of these strings is encountered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

/// Validation errors for DashScopeParameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamError {
    RepetitionPenaltyMustBePositive,
    ThinkingNotCompatibleWithRequired,
    EnableSearchNotSupported(String),
}

impl std::fmt::Display for ParamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RepetitionPenaltyMustBePositive => {
                write!(f, "repetition_penalty must be > 0")
            }
            Self::ThinkingNotCompatibleWithRequired => {
                write!(
                    f,
                    "enable_thinking=true is not compatible with tool_choice=\"required\""
                )
            }
            Self::EnableSearchNotSupported(model) => {
                write!(f, "enable_search is not supported for model '{model}'")
            }
        }
    }
}

impl DashScopeParameters {
    /// Validate parameter constraints before sending to API.
    pub fn validate(&self) -> Result<(), ParamError> {
        if let Some(rp) = self.repetition_penalty
            && rp <= 0.0
        {
            return Err(ParamError::RepetitionPenaltyMustBePositive);
        }
        Ok(())
    }

    /// Check if enable_thinking is incompatible with tool_choice="required".
    pub fn is_thinking_enabled(&self) -> bool {
        self.enable_thinking
    }

    /// Get the thinking_budget value if thinking is enabled.
    pub fn thinking_budget_for_request(&self) -> Option<u32> {
        if self.enable_thinking {
            self.thinking_budget
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_parameters() {
        let params = DashScopeParameters::default();
        assert!(!params.enable_search);
        assert!(!params.enable_thinking);
        assert!(params.max_tokens.is_none());
        assert!(params.repetition_penalty.is_none());
    }

    #[test]
    fn test_serde_round_trip() {
        let params = DashScopeParameters {
            max_tokens: Some(1024),
            temperature: Some(0.7),
            top_p: Some(0.9),
            enable_search: true,
            ..Default::default()
        };
        let json = serde_json::to_value(&params).unwrap();
        let parsed: DashScopeParameters = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.max_tokens, Some(1024));
        assert_eq!(parsed.temperature, Some(0.7));
        assert!(parsed.enable_search);
    }

    #[test]
    fn test_repetition_penalty_positive() {
        let params = DashScopeParameters {
            repetition_penalty: Some(0.0),
            ..Default::default()
        };
        assert!(params.validate().is_err());

        let params = DashScopeParameters {
            repetition_penalty: Some(1.5),
            ..Default::default()
        };
        assert!(params.validate().is_ok());
    }

    #[test]
    fn test_thinking_budget() {
        let params = DashScopeParameters::default();
        assert!(params.thinking_budget_for_request().is_none());

        let params = DashScopeParameters {
            enable_thinking: true,
            thinking_budget: Some(8192),
            ..Default::default()
        };
        assert_eq!(params.thinking_budget_for_request(), Some(8192));
    }
}
