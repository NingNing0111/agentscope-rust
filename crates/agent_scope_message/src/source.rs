//! Data source types for binary content blocks.

use serde::{Deserialize, Serialize};

/// Base64-encoded data source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Base64Source {
    /// Base64-encoded data string.
    pub data: String,
    /// MIME type (e.g. "image/png", "application/pdf").
    pub media_type: String,
}

/// URL-referenced data source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct URLSource {
    /// RFC 3986 compliant URI.
    pub url: String,
    /// MIME type of the resource.
    pub media_type: String,
}
