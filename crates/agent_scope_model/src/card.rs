//! ModelCard — model metadata loaded from YAML files (parsed into serde_json::Value by Provider).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::model_error::ModelError;

/// Model lifecycle status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelStatus {
    Active,
    Deprecated,
    Sunset,
}

/// Model metadata card. Fields are populated from a pre-parsed JSON value
/// (the Provider is responsible for YAML→JSON conversion).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCard {
    #[serde(rename = "type")]
    pub card_type: String,
    pub name: String,
    pub label: String,
    pub status: ModelStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated_at: Option<DateTime<Utc>>,
    #[serde(default = "default_input_types")]
    pub input_types: Vec<String>,
    #[serde(default = "default_output_types")]
    pub output_types: Vec<String>,
    pub context_size: i64,
    pub output_size: i64,
    #[serde(default)]
    pub parameter_schema: JsonValue,
    #[serde(default)]
    pub parameters_overrides: HashMap<String, JsonValue>,
}

fn default_input_types() -> Vec<String> {
    vec!["text/plain".to_string()]
}
fn default_output_types() -> Vec<String> {
    vec!["text/plain".to_string()]
}

/// Raw model card fields deserialized from JSON (YAML→JSON pre-converted by Provider).
#[derive(Debug, Deserialize)]
struct RawModelCardValue {
    name: String,
    label: String,
    #[serde(default = "default_status")]
    status: String,
    #[serde(default)]
    deprecated_at: Option<String>,
    #[serde(default = "default_input_types")]
    input_types: Vec<String>,
    #[serde(default = "default_output_types")]
    output_types: Vec<String>,
    context_size: i64,
    output_size: i64,
    #[serde(default)]
    parameter_overrides: HashMap<String, JsonValue>,
}

fn default_status() -> String {
    "active".to_string()
}

impl ModelCard {
    /// Build a ModelCard from a pre-parsed serde_json::Value and a base parameter schema.
    ///
    /// The Provider is responsible for reading the YAML file, converting it to a
    /// `serde_json::Value`, and calling this method. This keeps the core crate free
    /// of `serde_yaml` and file I/O dependencies.
    pub fn from_value(
        json_value: &JsonValue,
        base_parameter_schema: &JsonValue,
    ) -> Result<Self, ModelError> {
        let raw: RawModelCardValue =
            RawModelCardValue::deserialize(json_value).map_err(|e| ModelError::ConfigError {
                message: format!("Failed to deserialize model card value: {e}"),
            })?;

        let status = match raw.status.as_str() {
            "active" => ModelStatus::Active,
            "deprecated" => ModelStatus::Deprecated,
            "sunset" => ModelStatus::Sunset,
            _s => ModelStatus::Active,
        };

        let deprecated_at = raw.deprecated_at.and_then(|d| {
            chrono::DateTime::parse_from_rfc3339(&d)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        });

        let mut param_schema = base_parameter_schema.clone();

        // Apply auto-filters
        let has_thinking = raw
            .output_types
            .iter()
            .any(|t| t == "application/x-thinking");
        let has_audio = raw.output_types.iter().any(|t| t.starts_with("audio/"));

        if !has_thinking
            && let Some(props) = param_schema
                .get_mut("properties")
                .and_then(|p| p.as_object_mut())
        {
            props.remove("thinking_enable");
            props.remove("thinking_budget");
        }
        if !has_audio
            && let Some(props) = param_schema
                .get_mut("properties")
                .and_then(|p| p.as_object_mut())
        {
            props.remove("voice");
        }

        // Apply parameter overrides
        for (key, override_val) in &raw.parameter_overrides {
            if override_val.is_null() {
                if let Some(props) = param_schema
                    .get_mut("properties")
                    .and_then(|p| p.as_object_mut())
                {
                    props.remove(key);
                }
            } else if let Some(hidden) = override_val.get("hidden").and_then(|v| v.as_bool()) {
                if hidden
                    && let Some(props) = param_schema
                        .get_mut("properties")
                        .and_then(|p| p.as_object_mut())
                {
                    props.remove(key);
                }
            } else {
                if let Some(props) = param_schema
                    .get_mut("properties")
                    .and_then(|p| p.as_object_mut())
                    && let Some(prop) = props.get_mut(key)
                    && let Some(obj) = override_val.as_object()
                    && let Some(schema_obj) = prop.as_object_mut()
                {
                    for (k, v) in obj {
                        schema_obj.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        // Set max_tokens maximum from output_size
        if let Some(props) = param_schema
            .get_mut("properties")
            .and_then(|p| p.as_object_mut())
            && let Some(max_tokens) = props.get_mut("max_tokens")
            && let Some(obj) = max_tokens.as_object_mut()
        {
            obj.insert(
                "maximum".to_string(),
                JsonValue::Number(raw.output_size.into()),
            );
        }

        Ok(ModelCard {
            card_type: "chat_model".to_string(),
            name: raw.name,
            label: raw.label,
            status,
            deprecated_at,
            input_types: raw.input_types,
            output_types: raw.output_types,
            context_size: raw.context_size,
            output_size: raw.output_size,
            parameter_schema: param_schema,
            parameters_overrides: raw.parameter_overrides,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_base_schema() -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "max_tokens": { "type": "integer", "minimum": 1 },
                "temperature": { "type": "number", "minimum": 0.0, "maximum": 2.0 },
                "thinking_enable": { "type": "boolean", "default": false },
                "voice": { "type": "string" }
            }
        })
    }

    #[test]
    fn test_from_value_basic() {
        let json_val = serde_json::json!({
            "name": "test-model-v1",
            "label": "Test Model",
            "status": "active",
            "input_types": ["text/plain"],
            "output_types": ["text/plain"],
            "context_size": 32768,
            "output_size": 4096,
            "parameter_overrides": {}
        });

        let card = ModelCard::from_value(&json_val, &test_base_schema()).unwrap();
        assert_eq!(card.name, "test-model-v1");
        assert_eq!(card.card_type, "chat_model");
        assert_eq!(card.status, ModelStatus::Active);
        assert_eq!(card.context_size, 32768);
        assert_eq!(card.output_size, 4096);
    }

    #[test]
    fn test_from_value_thinking_filter() {
        let json_val = serde_json::json!({
            "name": "no-think",
            "label": "No Thinking",
            "status": "active",
            "output_types": ["text/plain"],
            "context_size": 16384,
            "output_size": 2048,
            "parameter_overrides": {}
        });

        let card = ModelCard::from_value(&json_val, &test_base_schema()).unwrap();
        let props = card.parameter_schema["properties"].as_object().unwrap();
        assert!(
            !props.contains_key("thinking_enable"),
            "thinking_enable should be filtered"
        );
        assert!(
            props.contains_key("temperature"),
            "temperature should remain"
        );
    }

    #[test]
    fn test_from_value_hidden_override() {
        let json_val = serde_json::json!({
            "name": "hidden-param",
            "label": "Hidden Param",
            "status": "active",
            "output_types": ["text/plain"],
            "context_size": 16384,
            "output_size": 2048,
            "parameter_overrides": {
                "temperature": { "hidden": true }
            }
        });

        let card = ModelCard::from_value(&json_val, &test_base_schema()).unwrap();
        let props = card.parameter_schema["properties"].as_object().unwrap();
        assert!(
            !props.contains_key("temperature"),
            "hidden param should be removed"
        );
    }

    #[test]
    fn test_from_value_max_tokens_from_output_size() {
        let json_val = serde_json::json!({
            "name": "max-tokens-test",
            "label": "Max Tokens",
            "status": "active",
            "output_types": ["text/plain"],
            "context_size": 32768,
            "output_size": 8192,
            "parameter_overrides": {}
        });

        let card = ModelCard::from_value(&json_val, &test_base_schema()).unwrap();
        let max_tokens = &card.parameter_schema["properties"]["max_tokens"];
        assert_eq!(max_tokens["maximum"], 8192);
    }
}
