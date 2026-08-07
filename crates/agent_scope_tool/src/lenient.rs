//! Lenient deserialization for LLM-produced tool arguments.
//!
//! LLM providers frequently emit well-formed JSON where a numeric field is
//! serialized as a string (`"max_results": "30"`) or a boolean as `"true"`.
//! Strict serde deserialization rejects these, turning an otherwise valid tool
//! call into a spurious error and forcing the agent into an error/recovery
//! loop. This module retries a failed strict deserialization with a targeted
//! string→number / string→bool coercion so such calls succeed instead.

use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;

/// Deserialize `value` into `T`, retrying with lenient string coercion when
/// strict serde deserialization fails.
///
/// The strict attempt runs first and its result is returned untouched when it
/// succeeds, so this helper never rewrites input that already deserializes.
/// Only when strict deserialization has already failed does the retry coerce
/// string-encoded numbers (`"30"` → `30`) and booleans (`"true"` → `true`) —
/// the dominant failure mode for LLM tool calls. If the retry also fails, the
/// error is returned exactly as before (the call is no worse off than a strict
/// `serde_json::from_value`).
pub fn deserialize_lenient<T: DeserializeOwned>(value: JsonValue) -> Result<T, serde_json::Error> {
    match serde_json::from_value::<T>(value.clone()) {
        Ok(typed) => Ok(typed),
        Err(_) => serde_json::from_value::<T>(coerce_strings(value)),
    }
}

/// Recursively convert string-encoded numbers and booleans to typed JSON
/// values. Only applied on the retry path (see [`deserialize_lenient`]), so a
/// string field whose value happens to be numeric (e.g. a `pattern` of
/// `"2026"`) is never mangled on the path that already deserializes.
fn coerce_strings(value: JsonValue) -> JsonValue {
    match value {
        JsonValue::String(s) => coerce_string(s),
        JsonValue::Array(items) => {
            JsonValue::Array(items.into_iter().map(coerce_strings).collect())
        }
        JsonValue::Object(map) => JsonValue::Object(
            map.into_iter()
                .map(|(k, v)| (k, coerce_strings(v)))
                .collect(),
        ),
        other => other,
    }
}

/// Coerce one string to a typed JSON value when it encodes a number or a
/// boolean; otherwise return it unchanged.
fn coerce_string(s: String) -> JsonValue {
    if let Ok(n) = s.parse::<i64>() {
        JsonValue::Number(n.into())
    } else if let Ok(n) = s.parse::<u64>() {
        JsonValue::Number(n.into())
    } else if let Ok(f) = s.parse::<f64>()
        && let Some(n) = serde_json::Number::from_f64(f)
    {
        // `from_f64` rejects NaN/±infinity, which `parse::<f64>` accepts; those
        // stay strings and surface as a normal deserialization error.
        JsonValue::Number(n)
    } else if s == "true" {
        JsonValue::Bool(true)
    } else if s == "false" {
        JsonValue::Bool(false)
    } else {
        JsonValue::String(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Deserialize)]
    struct TestInput {
        name: String,
        count: usize,
        ratio: Option<f64>,
        enabled: Option<bool>,
    }

    fn lenient<T: DeserializeOwned>(json: serde_json::Value) -> Result<T, serde_json::Error> {
        deserialize_lenient(json)
    }

    #[test]
    fn strict_path_is_untouched() {
        // Already well-typed input deserializes exactly as before.
        let input: TestInput = lenient(serde_json::json!({
            "name": "skill", "count": 30, "ratio": 0.5, "enabled": true
        }))
        .unwrap();
        assert_eq!(
            input,
            TestInput {
                name: "skill".into(),
                count: 30,
                ratio: Some(0.5),
                enabled: Some(true),
            }
        );
    }

    #[test]
    fn string_numbers_are_coerced() {
        // The reported failure mode: the LLM serialized numbers as strings.
        let input: TestInput = lenient(serde_json::json!({
            "name": "grep", "count": "30", "ratio": "0.5", "enabled": "true"
        }))
        .unwrap();
        assert_eq!(
            input,
            TestInput {
                name: "grep".into(),
                count: 30,
                ratio: Some(0.5),
                enabled: Some(true),
            }
        );
    }

    #[test]
    fn nested_string_numbers_are_coerced() {
        #[derive(Debug, PartialEq, Deserialize)]
        struct Outer {
            items: Vec<Inner>,
            flags: std::collections::HashMap<String, bool>,
        }
        #[derive(Debug, PartialEq, Deserialize)]
        struct Inner {
            max: usize,
            min: i64,
        }

        let input: Outer = lenient(serde_json::json!({
            "items": [{"max": "10", "min": "-5"}],
            "flags": {"a": "false"}
        }))
        .unwrap();
        assert_eq!(
            input,
            Outer {
                items: vec![Inner { max: 10, min: -5 }],
                flags: std::collections::HashMap::from([("a".into(), false)]),
            }
        );
    }

    #[test]
    fn string_field_with_numeric_value_is_not_mangled_on_strict_path() {
        // `pattern: "2026"` deserializes successfully on the strict pass, so
        // coercion never runs and the value stays a string.
        #[derive(Debug, PartialEq, Deserialize)]
        struct S {
            pattern: String,
        }
        let input: S = lenient(serde_json::json!({"pattern": "2026"})).unwrap();
        assert_eq!(
            input,
            S {
                pattern: "2026".into()
            }
        );
    }

    #[test]
    fn uncoercible_string_stays_a_string() {
        let input: TestInput = lenient(serde_json::json!({
            "name": "alpha-beta", "count": "42"
        }))
        .unwrap();
        assert_eq!(input.name, "alpha-beta");
        assert_eq!(input.count, 42);
    }

    #[test]
    fn genuinely_invalid_input_still_errors() {
        // A non-numeric string in a numeric field can't be salvaged — the
        // result must be an error, not a silently wrong value.
        let result: Result<TestInput, _> = lenient(serde_json::json!({
            "name": "x", "count": "not-a-number"
        }));
        assert!(result.is_err());
    }

    #[test]
    fn non_finite_floats_are_not_accepted() {
        // `"NaN"` parses as f64 but must not become a JSON number.
        let coerced = coerce_string("NaN".to_string());
        assert_eq!(coerced, JsonValue::String("NaN".to_string()));
    }
}
