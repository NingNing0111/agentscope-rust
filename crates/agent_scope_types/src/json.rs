//! JSON type aliases — Rust equivalents of Python's JSONPrimitive and JSONSerializableObject.

use serde_json::Value;

/// JSON-compatible value type.
/// Replaces Python's `JSONPrimitive` (str | int | float | bool | None)
/// and `JSONSerializableObject` (recursive dict/list/primitive).
pub type JsonValue = Value;
