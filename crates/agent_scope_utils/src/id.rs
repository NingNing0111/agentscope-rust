use uuid::Uuid;

/// Generate a 32-character hex UUID (matching Python `uuid4().hex`).
pub fn generate_id() -> String {
    Uuid::new_v4().as_simple().to_string()
}

/// Generate a hyphenated UUID string.
pub fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}

/// Generate an ISO 8601 / RFC 3339 timestamp.
pub fn generate_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id_is_32_hex_chars() {
        let id = generate_id();
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_generate_id_is_unique() {
        let id1 = generate_id();
        let id2 = generate_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_generate_uuid_is_hyphenated() {
        let id = generate_uuid();
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
        assert!(Uuid::parse_str(&id).is_ok());
    }

    #[test]
    fn test_generate_timestamp_is_rfc3339() {
        let ts = generate_timestamp();
        // RFC 3339 contains 'T' separator and either 'Z' or timezone offset
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z') || ts.contains('+') || ts.contains('-'));
        // Must parse as valid chrono DateTime
        assert!(chrono::DateTime::parse_from_rfc3339(&ts).is_ok());
    }
}
