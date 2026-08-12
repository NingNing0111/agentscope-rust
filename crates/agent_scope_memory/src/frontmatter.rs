//! YAML frontmatter parser and serializer for memory Markdown files.
//! Handles the `---` delimited metadata block at the top of each `.md` memory file.

use std::collections::HashMap;

use crate::MemoryEntry;

pub fn parse_frontmatter_fields(content: &str) -> HashMap<String, String> {
    agent_scope_utils::frontmatter::parse_frontmatter_fields(content)
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
            // YAML double-quoted scalars require tab to be escaped; a literal
            // tab produces invalid YAML for strict parsers (round-5 M2).
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

pub(crate) fn body_after_frontmatter(content: &str) -> Option<String> {
    agent_scope_utils::frontmatter::body_after_frontmatter(content)
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
    fn rejects_delimiters_with_suffix() {
        assert!(parse_frontmatter_fields("---suffix\nname: n\n---\nbody").is_empty());
        assert!(parse_frontmatter_fields("---\nname: n\n---suffix\nbody").is_empty());
        assert!(body_after_frontmatter("---\nname: n\n---suffix\nbody").is_none());
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
