//! ToolChoice — tool selection configuration for model calls.
//!
//! Self-contained in the model crate; does not depend on any tool crate.

use serde::{Deserialize, Serialize};

/// Tool selection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoice {
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
}

impl ToolChoice {
    pub fn new(mode: impl Into<String>) -> Self {
        Self {
            mode: mode.into(),
            tools: None,
        }
    }

    pub fn with_tools(mode: impl Into<String>, tools: Vec<String>) -> Self {
        Self {
            mode: mode.into(),
            tools: Some(tools),
        }
    }

    pub fn auto() -> Self {
        Self::new("auto")
    }
    pub fn none() -> Self {
        Self::new("none")
    }
    pub fn required() -> Self {
        Self::new("required")
    }
    pub fn specific_tool(name: impl Into<String>) -> Self {
        Self::new(name)
    }

    /// Validate mode and tools filter against available tools.
    pub fn validate(&self, available_tool_names: Option<&[String]>) -> Result<(), String> {
        match self.mode.as_str() {
            "auto" | "none" | "required" => {}
            tool_name => {
                if let Some(names) = available_tool_names
                    && !names.contains(&tool_name.to_string())
                {
                    return Err(format!("Tool '{tool_name}' not found in available tools"));
                }
            }
        }
        if let (Some(filter), Some(available)) = (&self.tools, available_tool_names) {
            for t in filter {
                if !available.contains(t) {
                    return Err(format!("Filter tool '{t}' not found in available tools"));
                }
            }
        }
        Ok(())
    }
}

impl Default for ToolChoice {
    fn default() -> Self {
        Self::auto()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_modes() {
        assert!(ToolChoice::auto().validate(None).is_ok());
        assert!(ToolChoice::none().validate(None).is_ok());
        assert!(ToolChoice::required().validate(None).is_ok());
    }

    #[test]
    fn test_specific_tool_valid() {
        let available = vec!["search".to_string(), "calc".to_string()];
        assert!(
            ToolChoice::specific_tool("search")
                .validate(Some(&available))
                .is_ok()
        );
    }

    #[test]
    fn test_specific_tool_invalid() {
        let available = vec!["search".to_string()];
        assert!(
            ToolChoice::specific_tool("nonexistent")
                .validate(Some(&available))
                .is_err()
        );
    }

    #[test]
    fn test_tools_filter_validation() {
        let available = vec!["search".to_string(), "calc".to_string()];
        let tc = ToolChoice::with_tools("auto", vec!["search".to_string()]);
        assert!(tc.validate(Some(&available)).is_ok());
        let tc = ToolChoice::with_tools("auto", vec!["bad_tool".to_string()]);
        assert!(tc.validate(Some(&available)).is_err());
    }
}
