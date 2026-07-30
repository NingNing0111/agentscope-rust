//! Session events serialization and round-trip tests.

use agent_scope_event::{
    EventBase, SessionClosedEvent, SessionCreatedEvent, SessionLoadedEvent, SessionSavedEvent,
    SessionTrimmedEvent,
};

#[test]
fn test_session_created_event_serialization() {
    let base = EventBase::new();
    let event = SessionCreatedEvent {
        base: base.clone(),
        session_id: "s-001".into(),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""session_id":"s-001""#));
    assert!(!json.contains(r#""type""#)); // base event has no type tag
}

#[test]
fn test_session_closed_event_serialization() {
    let base = EventBase::new();
    let event = SessionClosedEvent {
        base,
        session_id: "s-001".into(),
        reason: "explicit_close".into(),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""session_id":"s-001""#));
    assert!(json.contains(r#""reason":"explicit_close""#));
}

#[test]
fn test_session_saved_event_serialization() {
    let base = EventBase::new();
    let event = SessionSavedEvent {
        base,
        session_id: "s-001".into(),
        message_count: 42,
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""message_count":42"#));
}

#[test]
fn test_session_loaded_event_serialization() {
    let base = EventBase::new();
    let event = SessionLoadedEvent {
        base,
        session_id: "s-001".into(),
        message_count: 42,
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""message_count":42"#));
}

#[test]
fn test_session_trimmed_event_serialization() {
    let base = EventBase::new();
    let event = SessionTrimmedEvent {
        base,
        session_id: "s-trim".into(),
        messages_before: 100,
        messages_after: 50,
        tokens_before: Some(5000),
        tokens_after: Some(2400),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""messages_before":100"#));
    assert!(json.contains(r#""messages_after":50"#));

    // tokens_before/after should be skipped when None
    let event_no_tokens = SessionTrimmedEvent {
        base: EventBase::new(),
        session_id: "s-trim".into(),
        messages_before: 30,
        messages_after: 15,
        tokens_before: None,
        tokens_after: None,
    };
    let json2 = serde_json::to_string(&event_no_tokens).unwrap();
    assert!(!json2.contains("tokens_before"));
    assert!(!json2.contains("tokens_after"));
}

#[test]
fn test_session_events_roundtrip() {
    let base = EventBase::new();

    // SessionCreated
    let created = SessionCreatedEvent {
        base: base.clone(),
        session_id: "s-roundtrip".into(),
    };
    let json = serde_json::to_string(&created).unwrap();
    let restored: SessionCreatedEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.session_id, "s-roundtrip");

    // SessionClosed
    let closed = SessionClosedEvent {
        base: base.clone(),
        session_id: "s-roundtrip".into(),
        reason: "explicit_close".into(),
    };
    let json = serde_json::to_string(&closed).unwrap();
    let restored: SessionClosedEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.session_id, "s-roundtrip");
    assert_eq!(restored.reason, "explicit_close");

    // SessionSaved
    let saved = SessionSavedEvent {
        base: base.clone(),
        session_id: "s-roundtrip".into(),
        message_count: 42,
    };
    let json = serde_json::to_string(&saved).unwrap();
    let restored: SessionSavedEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.message_count, 42);

    // SessionLoaded
    let loaded = SessionLoadedEvent {
        base: base.clone(),
        session_id: "s-roundtrip".into(),
        message_count: 42,
    };
    let json = serde_json::to_string(&loaded).unwrap();
    let restored: SessionLoadedEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.message_count, 42);

    // SessionTrimmed
    let trimmed = SessionTrimmedEvent {
        base,
        session_id: "s-roundtrip".into(),
        messages_before: 100,
        messages_after: 50,
        tokens_before: Some(5000),
        tokens_after: Some(2400),
    };
    let json = serde_json::to_string(&trimmed).unwrap();
    let restored: SessionTrimmedEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.messages_before, 100);
    assert_eq!(restored.messages_after, 50);
    assert_eq!(restored.tokens_before, Some(5000));
    assert_eq!(restored.tokens_after, Some(2400));
}
