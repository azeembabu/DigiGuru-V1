//! The single error type ever serialised to a client.
//!
//! Per `CLAUDE.md`: "Errors returned to a client are `PublicError` only. Never
//! leak SQL, stack traces, or prompts." Any internal failure detail must be
//! logged (e.g. via `tracing::error!`) at the point it occurs, then converted
//! to `PublicError::Internal` — which carries no detail in its `Display` or
//! serialized form — before it ever reaches a response body.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// The only error type returned to API clients.
///
/// `Internal` is intentionally opaque: it must never carry arbitrary error
/// detail (no `String` payload, no `#[from]` on a lower-level error) because
/// anything attached to it is a candidate for leaking SQL, stack traces, or
/// prompt content to the client. Log the real cause where it happens, then
/// return `PublicError::Internal`.
#[derive(Debug, thiserror::Error)]
pub enum PublicError {
    #[error("not found")]
    NotFound,

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    /// A validation failure safe to show verbatim to the client (field-level
    /// messages only — never raw DB constraint text).
    #[error("{0}")]
    Validation(String),

    /// Anything else. Deliberately carries no detail — log internally first.
    #[error("internal server error")]
    Internal,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for PublicError {
    fn into_response(self) -> Response {
        let status = match self {
            PublicError::NotFound => StatusCode::NOT_FOUND,
            PublicError::Unauthorized => StatusCode::UNAUTHORIZED,
            PublicError::Forbidden => StatusCode::FORBIDDEN,
            PublicError::Validation(_) => StatusCode::BAD_REQUEST,
            PublicError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };

        // Note: `self.to_string()` is safe here only because every variant's
        // Display impl above is already client-safe by construction.
        let body = ErrorBody {
            error: self.to_string(),
        };

        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_display_has_no_detail() {
        assert_eq!(PublicError::Internal.to_string(), "internal server error");
    }
}
