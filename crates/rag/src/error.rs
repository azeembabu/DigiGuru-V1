//! `rag`'s own error type (per `.claude/rules/code-style.md`: "every crate
//! exposes its own `Error` enum"), plus the conversion into
//! `dg_core::PublicError`.
//!
//! `Display` retains detail for internal logging (parse-library messages,
//! Qdrant client errors) — none of that is ever safe to show a student or a
//! sub-admin verbatim, so the `From` impl below collapses every variant to a
//! single opaque `PublicError::Internal`/`PublicError::Unavailable`.

#[derive(Debug, thiserror::Error)]
pub enum RagError {
    #[error("pdf parse failed: {0}")]
    Parse(String),

    #[error("chunking failed: {0}")]
    Chunk(String),

    #[error("embedding failed: {0}")]
    Embed(String),

    #[error("qdrant operation failed: {0}")]
    Qdrant(String),

    #[error("database error: {0}")]
    Db(#[from] dg_db::Error),
}

/// Never leak parse-library internals, Qdrant client errors, or SQL detail
/// to a client (`CLAUDE.md` working agreements). `Qdrant` failures map to
/// `Unavailable` since Qdrant is an upstream dependency; everything else
/// maps to a generic internal error.
impl From<RagError> for dg_core::PublicError {
    fn from(err: RagError) -> Self {
        match err {
            RagError::Qdrant(detail) => {
                tracing::error!(detail = %detail, "qdrant operation failed");
                dg_core::PublicError::Unavailable
            }
            other => {
                tracing::error!(detail = %other, "rag pipeline error");
                dg_core::PublicError::Internal
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, RagError>;
