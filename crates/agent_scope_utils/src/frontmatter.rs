//! Frontmatter parsing helpers with AgentScope compatibility behavior.
//!
//! The raw parser dependency is intentionally kept behind this module so callers
//! depend on project-owned fallback, delimiter, and scalar compatibility rules.

use std::collections::HashMap;

use gray_matter::{Matter, engine::YAML};
use serde_json::Value;

/// Parsed `SKILL.md` metadata and body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    pub body: String,
}

/// Parse a `SKILL.md` file while preserving the legacy parser contract.
///
/// Missing or malformed frontmatter returns empty metadata and the original
/// content as the body.
pub fn parse_skill_frontmatter(content: &str) -> SkillFrontmatter {
    let Some((frontmatter, body)) = split_legacy_frontmatter(content.trim()) else {
        return SkillFrontmatter {
            name: String::new(),
            description: String::new(),
            body: content.to_string(),
        };
    };

    let fields =
        parse_yaml_fields(&frontmatter).unwrap_or_else(|| parse_legacy_fields(&frontmatter));

    SkillFrontmatter {
        name: fields.get("name").cloned().unwrap_or_default(),
        description: fields
            .get("description")
            .map(|description| description.trim_end_matches('\n').to_string())
            .unwrap_or_default(),
        body: body.trim().to_string(),
    }
}

/// Parse scalar-like frontmatter fields from markdown content.
///
/// Returns an empty map when frontmatter is absent or malformed.
pub fn parse_frontmatter_fields(content: &str) -> HashMap<String, String> {
    split_legacy_frontmatter(content)
        .and_then(|(frontmatter, _)| parse_yaml_fields(&frontmatter))
        .unwrap_or_default()
}

/// Return the body after a frontmatter block, preserving the legacy newline trim.
pub fn body_after_frontmatter(content: &str) -> Option<String> {
    split_legacy_frontmatter(content).map(|(_, body)| body.trim_start_matches('\n').to_string())
}

fn split_legacy_frontmatter(content: &str) -> Option<(String, String)> {
    let normalized = content.replace("\r\n", "\n");
    let rest = normalized.strip_prefix("---\n")?;
    if let Some(end) = rest.find("\n---\n") {
        let frontmatter = rest[..end].to_string();
        let body = rest[end + "\n---\n".len()..].to_string();
        return Some((frontmatter, body));
    }
    if let Some(frontmatter) = rest.strip_suffix("\n---") {
        return Some((frontmatter.to_string(), String::new()));
    }
    None
}

fn parse_yaml_fields(frontmatter: &str) -> Option<HashMap<String, String>> {
    let matter = Matter::<YAML>::new();
    let input = format!("---\n{frontmatter}\n---\n");
    let parsed = matter.parse::<Value>(&input).ok()?;
    let Value::Object(object) = parsed.data? else {
        return None;
    };

    let mut fields = HashMap::new();
    for (key, value) in object {
        if let Some(value) = value_to_legacy_string(value) {
            fields.insert(key, value);
        }
    }
    Some(fields)
}

fn value_to_legacy_string(value: Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null => Some(String::new()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn parse_legacy_fields(frontmatter: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    let lines: Vec<&str> = frontmatter.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();
        if let Some(value) = line.strip_prefix("name:") {
            fields.insert("name".to_string(), unquote(value.trim()).to_string());
        } else if let Some(value) = line.strip_prefix("description:") {
            let value = value.trim();
            if value == "|" || value == "|-" || value == ">" || value == ">-" {
                let folded = value.starts_with('>');
                let mut block_lines = Vec::new();
                i += 1;
                while i < lines.len() {
                    let raw = lines[i];
                    if raw.trim().is_empty() {
                        block_lines.push(String::new());
                        i += 1;
                        continue;
                    }
                    let indent = raw.chars().take_while(|c| c.is_whitespace()).count();
                    if indent == 0 {
                        i = i.saturating_sub(1);
                        break;
                    }
                    block_lines.push(raw.trim().to_string());
                    i += 1;
                }
                let description = if folded {
                    block_lines.join(" ")
                } else {
                    block_lines.join("\n")
                };
                fields.insert("description".to_string(), description);
            } else {
                fields.insert("description".to_string(), unquote(value).to_string());
            }
        }
        i += 1;
    }

    fields
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_skill_frontmatter_falls_back_to_original_body() {
        let parsed = parse_skill_frontmatter("body only");
        assert_eq!(parsed.name, "");
        assert_eq!(parsed.description, "");
        assert_eq!(parsed.body, "body only");
    }

    #[test]
    fn parses_skill_frontmatter() {
        let parsed = parse_skill_frontmatter("---\nname: test\ndescription: hello\n---\nbody");
        assert_eq!(parsed.name, "test");
        assert_eq!(parsed.description, "hello");
        assert_eq!(parsed.body, "body");
    }
}
