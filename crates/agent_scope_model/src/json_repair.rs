//! JSON repair — fixes common LLM-generated JSON formatting issues.

pub fn json_repair(input: &str) -> String {
    let mut result = input.trim().to_string();
    let mut brace_count: i32 = 0;
    let mut bracket_count: i32 = 0;
    let mut in_string = false;
    let mut prev_char = '\0';

    for ch in result.chars() {
        if prev_char != '\\' {
            match ch {
                '"' => in_string = !in_string,
                '{' if !in_string => brace_count += 1,
                '}' if !in_string => brace_count -= 1,
                '[' if !in_string => bracket_count += 1,
                ']' if !in_string => bracket_count -= 1,
                _ => {}
            }
        }
        prev_char = ch;
    }

    // Close unterminated string first, then close brackets/braces
    if in_string {
        result.push('"');
    }

    for _ in 0..bracket_count.max(0) {
        result.push(']');
    }
    for _ in 0..brace_count.max(0) {
        result.push('}');
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_closing_brace() {
        let input = r#"{"name": "test", "value": 42"#;
        let repaired = json_repair(input);
        assert!(serde_json::from_str::<serde_json::Value>(&repaired).is_ok());
    }

    #[test]
    fn test_missing_closing_bracket() {
        let input = r#"["a", "b", "c"#;
        let repaired = json_repair(input);
        assert!(serde_json::from_str::<serde_json::Value>(&repaired).is_ok());
    }

    #[test]
    fn test_nested_unbalanced() {
        let input = r#"{"items": [{"name": "a"}, {"name": "b"}"#;
        let repaired = json_repair(input);
        assert!(serde_json::from_str::<serde_json::Value>(&repaired).is_ok());
    }

    #[test]
    fn test_valid_json_unchanged() {
        let input = r#"{"key": "value"}"#;
        let repaired = json_repair(input);
        let v: serde_json::Value = serde_json::from_str(&repaired).unwrap();
        assert_eq!(v["key"], "value");
    }

    #[test]
    fn test_truncated_string() {
        let input = r#"{"text": "hello"#;
        let repaired = json_repair(input);
        let _: serde_json::Value = serde_json::from_str(&repaired).unwrap();
    }
}
