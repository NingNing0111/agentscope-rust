//! Custom event — for service-layer notifications not specific to the framework.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::base::EventBase;

/// Arbitrary service-layer event with name and value payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomEvent {
    #[serde(flatten)]
    pub base: EventBase,
    pub name: String,
    pub value: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_event_serialization() {
        let mut value = HashMap::new();
        value.insert("key".into(), serde_json::Value::String("val".into()));
        let event = CustomEvent {
            base: EventBase::new(),
            name: "my-event".into(),
            value,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""name":"my-event""#));
        assert!(json.contains(r#""key":"val""#));
    }
}
