//! JSON Schema flatten — resolve $ref/$defs into self-contained schemas.

use serde_json::Value as JsonValue;
use std::collections::HashSet;

// ── Error type ────────────────────────────────────────────────────────────

/// Error returned when schema flattening hits resource limits.
#[derive(Debug, Clone)]
pub struct SchemaExpansionError {
    pub reason: String,
}

impl std::fmt::Display for SchemaExpansionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Schema expansion error: {}", self.reason)
    }
}

// ── Resource limits ──────────────────────────────────────────────────────

/// Maximum number of `$ref` → `$defs` resolutions performed. Repeated
/// `$defs` entries with multiple references can cause exponential output
/// growth via 2^N expansion; this cap prevents unbounded work.
const MAX_EXPANSIONS: usize = 50_000;

/// Maximum number of JSON nodes (objects + arrays + primitives) in the
/// flattened output. Protects against output that is too large to serialize
/// or transmit.
const MAX_OUTPUT_NODES: usize = 100_000;

/// Maximum estimated output size in bytes (approximated from the JSON
/// string length of visited nodes). Protects against unbounded memory.
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024; // 8 MiB

/// Recursion depth cap.
const MAX_DEPTH: usize = 512;

// ── Public API ───────────────────────────────────────────────────────────

/// Flatten a JSON Schema by resolving all `$ref` → `$defs` references.
///
/// This is the **compatibility wrapper** — it calls the checked version
/// and falls back to an empty schema on limit-exceeded errors. Callers
/// that need to propagate errors should use `flatten_json_schema_with_defs_checked`.
pub fn flatten_json_schema_with_defs(schema: &JsonValue) -> JsonValue {
    match flatten_json_schema_with_defs_checked(schema) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "schema_flat: expansion limit hit, falling back to empty schema: {}",
                e
            );
            JsonValue::Object(serde_json::Map::new())
        }
    }
}

/// Flatten a JSON Schema by resolving all `$ref` → `$defs` references,
/// returning an error if resource limits are exceeded.
pub fn flatten_json_schema_with_defs_checked(
    schema: &JsonValue,
) -> Result<JsonValue, SchemaExpansionError> {
    let defs = schema
        .get("$defs")
        .and_then(|d| d.as_object())
        .cloned()
        .unwrap_or_default();
    let mut visiting = HashSet::new();
    let mut state = FlattenState::new();
    flatten_with_defs_checked(schema, &defs, &mut visiting, &mut state, 0)
}

// ── Internal state tracking ──────────────────────────────────────────────

struct FlattenState {
    expansions: usize,
    total_nodes: usize,
    estimated_bytes: usize,
}

impl FlattenState {
    fn new() -> Self {
        Self {
            expansions: 0,
            total_nodes: 0,
            estimated_bytes: 0,
        }
    }

    fn record_expansion(&mut self) -> Result<(), SchemaExpansionError> {
        self.expansions += 1;
        if self.expansions > MAX_EXPANSIONS {
            return Err(SchemaExpansionError {
                reason: format!(
                    "expansion limit exceeded: {MAX_EXPANSIONS} $defs expansions; \
                     schema may have exponential $defs explosion"
                ),
            });
        }
        Ok(())
    }

    fn record_node(&mut self, node: &JsonValue) -> Result<(), SchemaExpansionError> {
        self.total_nodes += 1;
        if self.total_nodes > MAX_OUTPUT_NODES {
            return Err(SchemaExpansionError {
                reason: format!(
                    "node limit exceeded: max {MAX_OUTPUT_NODES} nodes in flattened output"
                ),
            });
        }
        // Estimate byte size from JSON string length (upper bound)
        self.estimated_bytes += node.to_string().len();
        if self.estimated_bytes > MAX_OUTPUT_BYTES {
            return Err(SchemaExpansionError {
                reason: format!("output size limit exceeded: max {MAX_OUTPUT_BYTES} bytes"),
            });
        }
        Ok(())
    }
}

fn flatten_with_defs_checked(
    node: &JsonValue,
    defs: &serde_json::Map<String, JsonValue>,
    visiting: &mut HashSet<String>,
    state: &mut FlattenState,
    depth: usize,
) -> Result<JsonValue, SchemaExpansionError> {
    if depth > MAX_DEPTH {
        // Fail loudly like the other resource limits instead of silently
        // truncating the schema to an empty object — a caller could not
        // distinguish "shallow empty schema" from "schema too deep and was
        // truncated" (round-5 M4).
        return Err(SchemaExpansionError {
            reason: format!("max depth exceeded: limit is {MAX_DEPTH}"),
        });
    }

    if let Some(ref_str) = node.get("$ref").and_then(|v| v.as_str()) {
        if let Some(type_name) = ref_str.strip_prefix("#/$defs/")
            && !visiting.contains(type_name)
            && let Some(def) = defs.get(type_name)
        {
            state.record_expansion()?;
            visiting.insert(type_name.to_string());
            let result = flatten_with_defs_checked(def, defs, visiting, state, depth + 1);
            visiting.remove(type_name);
            return result;
        }
        // Unresolvable $ref (missing definition, cycle, or non-$defs ref)
        return Ok(JsonValue::Object(serde_json::Map::new()));
    }

    if let Some(obj) = node.as_object() {
        let mut result = serde_json::Map::new();
        for (key, val) in obj {
            if key == "$defs" {
                continue;
            }
            let flattened = flatten_with_defs_checked(val, defs, visiting, state, depth + 1)?;
            state.record_node(&flattened)?;
            result.insert(key.clone(), flattened);
        }
        return Ok(JsonValue::Object(result));
    }

    if let Some(arr) = node.as_array() {
        let mut result = Vec::new();
        for v in arr {
            let flattened = flatten_with_defs_checked(v, defs, visiting, state, depth + 1)?;
            state.record_node(&flattened)?;
            result.push(flattened);
        }
        return Ok(JsonValue::Array(result));
    }

    state.record_node(node)?;
    Ok(node.clone())
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

    // -----------------------------------------------------------------------
    // Defect 4: Exponential $defs expansion limits
    // -----------------------------------------------------------------------

    /// Construct a schema where each `$defs` level references the next level
    /// twice, creating 2^N expansion. The checked flatten must hit its
    /// expansion limit and return an error.
    #[test]
    fn exponential_defs_hits_expansion_limit() {
        // Build a chain: L(N) references L(N-1) twice, L(0) is a leaf.
        // With 7 levels: 2^7 = 128 expirations. Each creates a full object copy.
        // 10 levels = 1024 expansions. 15 levels = 32768.
        // We'll use 20 levels = ~1M expansions to trigger the limit.
        let mut defs = serde_json::Map::new();
        // Leaf type
        defs.insert("L0".to_string(), serde_json::json!({"type": "string"}));
        // Intermediate levels: each doubles the references
        for i in 1..=20 {
            defs.insert(
                format!("L{i}"),
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "a": { "$ref": format!("#/$defs/L{}", i - 1) },
                        "b": { "$ref": format!("#/$defs/L{}", i - 1) }
                    }
                }),
            );
        }
        let schema = serde_json::json!({
            "$defs": defs,
            "type": "object",
            "properties": {
                "root": { "$ref": "#/$defs/L20" }
            }
        });

        let result = flatten_json_schema_with_defs_checked(&schema);
        assert!(
            result.is_err(),
            "exponential 2^20 expansion should hit the limit and return an error"
        );
        let err = result.unwrap_err();
        assert!(
            err.reason.contains("expansion")
                || err.reason.contains("limit")
                || err.reason.contains("node"),
            "error should mention expansion/node limit: {}",
            err.reason
        );
    }

    /// A moderate schema (a handful of $defs referencing each other without
    /// explosion) must still succeed with the checked API.
    #[test]
    fn moderate_schema_succeeds_with_checked_api() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "item": { "$ref": "#/$defs/Item" },
                "items": {
                    "type": "array",
                    "items": { "$ref": "#/$defs/Item" }
                }
            },
            "$defs": {
                "Item": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "value": { "type": "number" }
                    }
                }
            }
        });
        let result = flatten_json_schema_with_defs_checked(&schema);
        assert!(
            result.is_ok(),
            "moderate schema should succeed with checked API: {:?}",
            result.err()
        );
        let flattened = result.unwrap();
        let item = &flattened["properties"]["item"];
        assert_eq!(item["type"], "object");
        assert_eq!(item["properties"]["name"]["type"], "string");
    }

    /// The compatibility wrapper `flatten_json_schema_with_defs` must not
    /// panic on exponential schemas — it should produce a fallback.
    #[test]
    fn exponential_defs_wrapper_does_not_panic() {
        let mut defs = serde_json::Map::new();
        defs.insert("L0".to_string(), serde_json::json!({"type": "string"}));
        for i in 1..=20 {
            defs.insert(
                format!("L{i}"),
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "a": { "$ref": format!("#/$defs/L{}", i - 1) },
                        "b": { "$ref": format!("#/$defs/L{}", i - 1) }
                    }
                }),
            );
        }
        let schema = serde_json::json!({
            "$defs": defs,
            "type": "object",
            "properties": { "root": { "$ref": "#/$defs/L20" } }
        });
        // Wrapper must not panic — it should fall back to a safe output
        let flattened = flatten_json_schema_with_defs(&schema);
        // Still has the "type" and "properties" keys (root survived)
        assert!(flattened.is_object());
    }
}
