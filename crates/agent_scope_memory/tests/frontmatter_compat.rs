use agent_scope_memory::{
    MemoryEntry, MemoryType, parse_frontmatter_fields, serialize_frontmatter,
};

#[test]
fn serialized_memory_frontmatter_round_trips_quoted_scalars() {
    let mut entry = MemoryEntry::new(
        "quoted-memory",
        "line one\nline \"two\" with \\ slash",
        MemoryType::Project,
        "# Body\ncontent",
    );
    entry.metadata.created_at = "2026-08-11T00:00:00Z".into();
    entry.metadata.updated_at = "2026-08-11T00:01:00Z".into();
    entry.metadata.tags = Some(vec!["alpha".into(), "beta gamma".into()]);

    let serialized = serialize_frontmatter(&entry);
    let fields = parse_frontmatter_fields(&serialized);

    assert_eq!(
        fields.get("name").map(String::as_str),
        Some("quoted-memory")
    );
    assert_eq!(
        fields.get("description").map(String::as_str),
        Some("line one\nline \"two\" with \\ slash")
    );
    assert_eq!(fields.get("type").map(String::as_str), Some("project"));
    assert_eq!(
        fields.get("created_at").map(String::as_str),
        Some("2026-08-11T00:00:00Z")
    );
    assert_eq!(
        fields.get("updated_at").map(String::as_str),
        Some("2026-08-11T00:01:00Z")
    );
    assert_eq!(
        fields.get("tags").map(String::as_str),
        Some("alpha, beta gamma")
    );
    assert!(
        serialized.contains("---\n\n# Body\ncontent"),
        "{serialized}"
    );
}

#[test]
fn reads_legacy_crlf_frontmatter() {
    let content = "---\r\nname: legacy\r\ndescription: \"legacy description\"\r\ntype: feedback\r\ncreated_at: 2026-08-11T00:00:00Z\r\nupdated_at: 2026-08-11T00:01:00Z\r\ntags: \"one, two\"\r\n---\r\n\r\nLegacy body";

    let fields = parse_frontmatter_fields(content);

    assert_eq!(fields.get("name").map(String::as_str), Some("legacy"));
    assert_eq!(
        fields.get("description").map(String::as_str),
        Some("legacy description")
    );
    assert_eq!(fields.get("type").map(String::as_str), Some("feedback"));
    assert_eq!(fields.get("tags").map(String::as_str), Some("one, two"));
}

#[test]
fn rejects_malformed_delimiters_without_partial_fields() {
    let content = "---\nname: bad\ndescription: bad\n---suffix\nbody";

    assert!(parse_frontmatter_fields(content).is_empty());
}
