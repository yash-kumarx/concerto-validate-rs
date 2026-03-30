//! Shared error type for loader and validator code.
//!
//! Keeping parse, lookup, and instance-shape failures separate saved me a lot
//! of time while debugging. "bad model" and "bad payload" are not the same job.

/// Top-level error type used across `concerto-core`.
#[derive(Debug, thiserror::Error)]
pub enum ConcertoError {
    /// Raised when model JSON or future CTO input cannot be parsed.
    #[error("parse error: {0}")]
    Parse(String),

    /// Raised when a requested type cannot be found.
    #[error("type not found: {0}")]
    TypeNotFound(String),

    /// Raised when a namespace lookup fails.
    #[error("namespace not found: {0}")]
    NamespaceNotFound(String),

    /// Raised when inheritance loops back on itself.
    #[error("circular dependency: {0}")]
    CircularDependency(String),

    /// Raised for semantically invalid model definitions.
    #[error("semantic error: {0}")]
    Semantic(String),

    /// Transparent wrapper for `serde_json` parse failures.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Raised when the incoming instance is the wrong JSON shape to validate.
    #[error("invalid instance: {0}")]
    InvalidInstance(String),
}
