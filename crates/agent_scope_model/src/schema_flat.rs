//! JSON Schema flatten — resolve $ref/$defs into self-contained schemas.

use serde_json::Value as JsonValue;
use std::collections::HashSet;

/// Flatten a full JSON Schema by resolving all `$ref` → `$defs` references.
pub fn flatten_json_schema_with_defs(schema: &JsonValue) -> JsonValue {
    let defs = schema
        .get("$defs")
        .and_then(|d| d.as_object())
        .cloned()
        .unwrap_or_default();
    // `visiting` tracks the definitions currently on the resolution path, so
    // only genuine cycles are cut. Re-using an already-*resolved* definition
    // (e.g. two properties referencing the same `$defs` entry) is allowed.
    let mut visiting = HashSet::new();
    flatten_with_defs(schema, &defs, &mut visiting)
}

fn flatten_with_defs(
    node: &JsonValue,
    defs: &serde_json::Map<String, JsonValue>,
    visiting: &mut HashSet<String>,
) -> JsonValue {
    if let Some(ref_str) = node.get("$ref").and_then(|v| v.as_str())
        && let Some(type_name) = ref_str.strip_prefix("#/$defs/")
    {
        // A definition that is already on the current resolution path is a
        // genuine cycle; everything else (resolved earlier) is expanded again.
        if !visiting.contains(type_name)
            && let Some(def) = defs.get(type_name)
        {
            visiting.insert(type_name.to_string());
            let result = flatten_with_defs(def, defs, visiting);
            visiting.remove(type_name);
            return result;
        }
        return JsonValue::String(format!("__CIRCULAR_OR_MISSING_{type_name}"));
    }

    if let Some(obj) = node.as_object() {
        let mut result = serde_json::Map::new();
        for (key, val) in obj {
            if key == "$defs" {
                continue;
            }
            result.insert(key.clone(), flatten_with_defs(val, defs, visiting));
        }
        return JsonValue::Object(result);
    }

    if let Some(arr) = node.as_array() {
        return JsonValue::Array(
            arr.iter()
                .map(|v| flatten_with_defs(v, defs, visiting))
                .collect(),
        );
    }

    node.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_ref_resolution() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "item": { "$ref": "#/$defs/Item" } },
            "$defs": { "Item": { "type": "object", "properties": { "name": { "type": "string" } } } }
        });
        let flattened = flatten_json_schema_with_defs(&schema);
        let props = &flattened["properties"]["item"];
        assert_eq!(props["type"], "object");
        assert_eq!(props["properties"]["name"]["type"], "string");
        assert!(flattened.get("$defs").is_none());
    }

    #[test]
    fn test_circular_ref_prevention() {
        let schema = serde_json::json!({
            "$defs": { "Node": { "type": "object", "properties": { "child": { "$ref": "#/$defs/Node" } } } },
            "type": "object",
            "properties": { "root": { "$ref": "#/$defs/Node" } }
        });
        let flattened = flatten_json_schema_with_defs(&schema);
        let root = &flattened["properties"]["root"];
        assert_eq!(root["type"], "object");
    }
}
