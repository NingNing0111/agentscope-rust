//! YAML frontmatter parser and serializer for memory Markdown files.
//! Handles the `---` delimited metadata block at the top of each `.md` memory file.

use std::collections::HashMap;

use regex::Regex;

use crate::MemoryEntry;

pub fn parse_frontmatter_fields(content: &str) -> HashMap<String, String> {
    let Ok(block_re) = Regex::new(r"(?s)\A---\r?\n(.*?)\r?\n---") else {
        return HashMap::new();
    };
    let Some(captures) = block_re.captures(content) else {
        return HashMap::new();
    };
    let Some(block) = captures.get(1) else {
        return HashMap::new();
    };

    let Ok(field_re) = Regex::new(r"^([A-Za-z_][A-Za-z0-9_-]*):\s*(.*)$") else {
        return HashMap::new();
    };

    block
        .as_str()
        .lines()
        .filter_map(|line| {
            let captures = field_re.captures(line.trim())?;
            let key = captures.get(1)?.as_str().to_string();
            let raw_value = captures.get(2)?.as_str().trim();
            let value = if raw_value.starts_with('"') && raw_value.ends_with('"') {
                yaml_unescape(&raw_value[1..raw_value.len() - 1])
            } else {
                // Legacy unquoted value (simple scalar written by older code).
                raw_value.trim_matches('"').to_string()
            };
            Some((key, value))
        })
        .collect()
}

pub fn serialize_frontmatter(entry: &MemoryEntry) -> String {
    let tags = entry.metadata.tags.as_ref().map(|tags| tags.join(", "));
    let mut lines = vec![
        "---".to_string(),
        format!("name: {}", entry.name),
        // description/tags are user-supplied and may contain newlines, quotes
        // or backslashes; quote them so they round-trip losslessly.
        format!("description: {}", yaml_quote(&entry.description)),
        format!("type: {}", entry.metadata.mem_type.as_str()),
        format!("created_at: {}", entry.metadata.created_at),
        format!("updated_at: {}", entry.metadata.updated_at),
    ];
    if let Some(tags) = tags {
        lines.push(format!("tags: {}", yaml_quote(&tags)));
    }
    lines.push("---".to_string());
    lines.push(String::new());
    lines.push(entry.content.clone());
    lines.join("\n")
}

/// Quote a string as a single-line YAML double-quoted scalar.
fn yaml_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Unescape a double-quoted YAML scalar (inverse of [`yaml_quote`]).
fn yaml_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub(crate) fn body_after_frontmatter(content: &str) -> Option<String> {
    let normalized = content.replace("\r\n", "\n");
    if !normalized.starts_with("---\n") {
        return None;
    }
    let rest = &normalized[4..];
    let end = rest.find("\n---")?;
    let after = &rest[end + 4..];
    Some(after.trim_start_matches('\n').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryEntry, MemoryType};

    #[test]
    fn parses_valid_frontmatter() {
        let fields = parse_frontmatter_fields(
            "---\nname: user-role\ndescription: desc\ntype: user\n---\nbody",
        );
        assert_eq!(fields.get("name").unwrap(), "user-role");
        assert_eq!(fields.get("description").unwrap(), "desc");
        assert_eq!(fields.get("type").unwrap(), "user");
    }

    #[test]
    fn missing_delimiters_returns_empty_fields() {
        assert!(parse_frontmatter_fields("name: nope\n---").is_empty());
    }

    #[test]
    fn empty_fields_are_preserved() {
        let fields = parse_frontmatter_fields("---\ndescription: \n---\n");
        assert_eq!(fields.get("description").unwrap(), "");
    }

    #[test]
    fn serializes_roundtrip_fields() {
        let entry = MemoryEntry::new("user-role", "Role hint", MemoryType::User, "body");
        let serialized = serialize_frontmatter(&entry);
        let fields = parse_frontmatter_fields(&serialized);
        assert_eq!(fields.get("name").unwrap(), "user-role");
        assert_eq!(body_after_frontmatter(&serialized).unwrap(), "body");
    }

    #[test]
    fn supports_multiline_body_after_frontmatter() {
        let body = body_after_frontmatter("---\nname: n\n---\nline1\nline2").unwrap();
        assert_eq!(body, "line1\nline2");
    }
}
