use agent_scope_utils::frontmatter::{
    body_after_frontmatter, parse_frontmatter_fields, parse_skill_frontmatter,
};

#[test]
fn parses_inline_skill_frontmatter_and_trims_body_like_legacy_callers() {
    let parsed = parse_skill_frontmatter(
        "---\nname: test-skill\ndescription: A test skill\n---\n\n# Body\n",
    );

    assert_eq!(parsed.name, "test-skill");
    assert_eq!(parsed.description, "A test skill");
    assert_eq!(parsed.body, "# Body");
}

#[test]
fn parses_quoted_skill_scalars() {
    let parsed = parse_skill_frontmatter(
        "---\nname: \"quoted-skill\"\ndescription: 'Quoted description'\n---\nBody",
    );

    assert_eq!(parsed.name, "quoted-skill");
    assert_eq!(parsed.description, "Quoted description");
    assert_eq!(parsed.body, "Body");
}

#[test]
fn parses_literal_description_block_scalar() {
    let parsed = parse_skill_frontmatter(
        "---\nname: claude-api\ndescription: |-\n  Reference for the Claude API.\n  Second line of description.\nlicense: Proprietary\n---\n\nBody.",
    );

    assert_eq!(parsed.name, "claude-api");
    assert_eq!(
        parsed.description,
        "Reference for the Claude API.\nSecond line of description."
    );
    assert_eq!(parsed.body, "Body.");
}

#[test]
fn parses_folded_description_block_scalar() {
    let parsed = parse_skill_frontmatter(
        "---\nname: claude-api\ndescription: >-\n  Reference for the Claude API.\n  Second line of description.\n---\n\nBody.",
    );

    assert_eq!(parsed.name, "claude-api");
    assert_eq!(
        parsed.description,
        "Reference for the Claude API. Second line of description."
    );
    assert_eq!(parsed.body, "Body.");
}

#[test]
fn malformed_skill_frontmatter_falls_back_to_original_content() {
    let content = "---\nname: missing-close\ndescription: bad\n# Body";
    let parsed = parse_skill_frontmatter(content);

    assert_eq!(parsed.name, "");
    assert_eq!(parsed.description, "");
    assert_eq!(parsed.body, content);
}

#[test]
fn parses_memory_style_fields_with_crlf_and_eof_delimiter() {
    let content = "---\r\nname: mem\r\ndescription: \"hello\\nworld\"\r\ntype: user\r\n---";
    let fields = parse_frontmatter_fields(content);

    assert_eq!(fields.get("name").map(String::as_str), Some("mem"));
    assert_eq!(
        fields.get("description").map(String::as_str),
        Some("hello\nworld")
    );
    assert_eq!(fields.get("type").map(String::as_str), Some("user"));
    assert_eq!(body_after_frontmatter(content), Some(String::new()));
}
