//! Conservative repair of malformed tool-argument JSON.
//!
//! LLM providers streaming `tool_call.arguments` occasionally emit a JSON
//! object that is very close to valid but structurally off — most commonly a
//! trailing extra closing brace (e.g. `{"a": 1}}`) or a truncated string /
//! object at stream end. This module repairs the unambiguous, common cases
//! without ever altering input that already parses as valid JSON.

use serde_json::Value;

/// Outcome of [`repair_tool_input`].
#[derive(Debug)]
pub enum RepairOutcome {
    /// The input already parsed as valid JSON; returned unchanged.
    Valid(Value),
    /// The input was repaired and then parsed successfully.
    Repaired {
        original: String,
        repaired: String,
        value: Value,
    },
    /// No safe repair path existed; carries the original parse error.
    Failed { error: serde_json::Error },
}

/// Try to parse `input` as a JSON value, repairing only the unambiguous
/// malformations an LLM tool-call stream can produce.
///
/// Guarantees:
/// - Input that already parses is returned untouched (never rewritten).
/// - Every repair candidate must re-parse successfully **and** be a top-level
///   JSON object (all agent tool arguments are objects) before it is accepted,
///   so a malformed candidate can never be substituted for valid input.
pub fn repair_tool_input(input: &str) -> RepairOutcome {
    // Phase 0: valid JSON passes through unchanged.
    if let Ok(value) = serde_json::from_str::<Value>(input) {
        return RepairOutcome::Valid(value);
    }

    // Phase 1: generate a bounded set of candidates, each verified by a fresh
    // parse. Only a candidate that re-parses as a top-level object is accepted.
    for candidate in repair_candidates(input) {
        if let Ok(value) = serde_json::from_str::<Value>(&candidate)
            && value.is_object()
        {
            return RepairOutcome::Repaired {
                original: input.to_string(),
                repaired: candidate,
                value,
            };
        }
    }

    // No safe repair path — preserve the original parse error.
    RepairOutcome::Failed {
        error: serde_json::from_str::<Value>(input).unwrap_err(),
    }
}

/// Generate an ordered, bounded list of repair candidates for malformed input.
///
/// Candidates are cheap string transformations; the caller re-parses each one
/// and only accepts those that validate. Order matters: the most likely /
/// least invasive fix (trailing extra delimiters) comes first.
fn repair_candidates(input: &str) -> Vec<String> {
    let (depth, excess_close, in_string) = scan_top_level(input);
    let mut out = Vec::new();

    // Candidate A: trailing extra closing delimiters — the reported `"}}` bug.
    // Strip up to `excess_close` (bounded at 3) trailing `}`/`]`, skipping
    // whitespace.
    if excess_close > 0 {
        for n in 1..=excess_close.min(3) {
            if let Some(stripped) = strip_trailing_delims(input, n) {
                out.push(stripped);
            }
        }
    }

    // Candidate B: a trailing comma before the closing delimiter (`{"a":1,}`).
    if let Some(no_comma) = remove_trailing_comma(input) {
        out.push(no_comma);
    }

    // Candidate C: EOF inside an unterminated string at shallow nesting.
    if in_string && depth <= 2 {
        out.push(format!("{input}\"{}", "}".repeat(depth)));
    }

    // Candidate D: EOF inside an unclosed object/array at shallow nesting.
    if !in_string && depth > 0 && depth <= 2 {
        out.push(format!("{input}{}", "}".repeat(depth)));
    }

    // Candidate E: UTF-8 BOM prefix.
    if let Some(trimmed) = input.strip_prefix('\u{feff}') {
        out.push(trimmed.to_string());
    }

    out
}

/// Strip `n` trailing `}`/`]` characters from `input`, skipping whitespace.
/// Returns `None` if fewer than `n` trailing delimiters exist.
fn strip_trailing_delims(input: &str, n: usize) -> Option<String> {
    let mut chars: Vec<char> = input.chars().collect();
    let mut removed = 0;
    let mut i = chars.len();
    while i > 0 && removed < n {
        i -= 1;
        if chars[i].is_whitespace() {
            continue;
        }
        if matches!(chars[i], '}' | ']') {
            chars.remove(i);
            removed += 1;
        } else {
            return None;
        }
    }
    (removed == n).then(|| chars.into_iter().collect())
}

/// Remove a comma sitting just before the final closing delimiter.
fn remove_trailing_comma(input: &str) -> Option<String> {
    let mut chars: Vec<char> = input.trim_end().chars().collect();
    if !matches!(chars.last().copied(), Some('}' | ']')) {
        return None;
    }
    let mut i = chars.len() - 1; // index of the final closing delimiter
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    if i > 0 && chars[i - 1] == ',' {
        chars.remove(i - 1);
        Some(chars.into_iter().collect())
    } else {
        None
    }
}

/// Scan `input` for top-level bracket imbalance, skipping string contents.
///
/// Returns `(depth, excess_close, in_string)` where:
/// - `depth` — the net nesting depth at EOF (unclosed `{`/`[`);
/// - `excess_close` — the count of `}`/`]` that closed at depth zero;
/// - `in_string` — whether EOF falls inside an unterminated `"..."`.
fn scan_top_level(input: &str) -> (usize, usize, bool) {
    let mut depth = 0usize;
    let mut excess_close = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for ch in input.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else {
            match ch {
                '"' => in_string = true,
                '{' | '[' => depth += 1,
                '}' | ']' => {
                    if depth > 0 {
                        depth -= 1;
                    } else {
                        excess_close += 1;
                    }
                }
                _ => {}
            }
        }
    }

    (depth, excess_close, in_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repaired_value(input: &str) -> Value {
        match repair_tool_input(input) {
            RepairOutcome::Valid(v) | RepairOutcome::Repaired { value: v, .. } => v,
            RepairOutcome::Failed { .. } => panic!("expected repair for {input:?}"),
        }
    }

    fn is_failed(input: &str) {
        assert!(
            matches!(repair_tool_input(input), RepairOutcome::Failed { .. }),
            "expected Failed for {input:?}"
        );
    }

    #[test]
    fn valid_json_is_untouched() {
        let input = r#"{"a":1,"b":[1,2]}"#;
        match repair_tool_input(input) {
            RepairOutcome::Valid(v) => assert_eq!(v, serde_json::json!({"a":1,"b":[1,2]})),
            other => panic!("expected Valid, got {other:?}"),
        }
    }

    #[test]
    fn valid_json_with_nested_braces_is_untouched() {
        // Legal nested braces must never be "repaired".
        let input = r#"{"a":{"b":1}}"#;
        assert!(matches!(repair_tool_input(input), RepairOutcome::Valid(_)));
    }

    #[test]
    fn trailing_extra_brace_is_repaired() {
        // Regression for the reported `"}}` bug: a trailing `}` after a
        // complete object.
        assert_eq!(
            repaired_value(r#"{"a":"</html>"}}"#),
            serde_json::json!({"a":"</html>"})
        );
    }

    #[test]
    fn multiple_excess_braces_are_repaired() {
        assert_eq!(repaired_value(r#"{"a":1}}}"#), serde_json::json!({"a":1}));
    }

    #[test]
    fn trailing_comma_is_repaired() {
        assert_eq!(repaired_value(r#"{"a":1,}"#), serde_json::json!({"a":1}));
    }

    #[test]
    fn truncated_string_is_repaired() {
        assert_eq!(repaired_value(r#"{"a":"b"#), serde_json::json!({"a":"b"}));
    }

    #[test]
    fn unclosed_object_is_repaired() {
        assert_eq!(repaired_value(r#"{"a":1"#), serde_json::json!({"a":1}));
    }

    #[test]
    fn bom_prefixed_input_is_repaired() {
        assert_eq!(
            repaired_value("\u{feff}{\"a\":1}"),
            serde_json::json!({"a":1})
        );
    }

    #[test]
    fn non_object_top_level_is_rejected() {
        // `[1,2]]` trivially repairs to `[1,2]`, but tool arguments are
        // objects — accepting an array here would mask a wrong-shaped call.
        is_failed("[1,2]]");
    }

    #[test]
    fn unfixable_input_returns_original_error() {
        is_failed("not valid json");
        is_failed("");
        is_failed(r#"{"a": 1, "b": }"#);
    }

    #[test]
    fn scanner_skips_braces_inside_strings() {
        // `}` inside a string must not count as an excess close.
        let input = r#"{"a":"}","b":1}"#;
        assert!(matches!(repair_tool_input(input), RepairOutcome::Valid(_)));
    }

    #[test]
    fn whitespace_around_trailing_delim_is_handled() {
        assert_eq!(repaired_value(r#"{"a":1}  }"#), serde_json::json!({"a":1}));
    }
}
