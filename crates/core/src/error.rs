//! Internal (`Error`) and client-facing (`PublicError`) error types.
//!
//! `Error` is what crate-internal code returns and propagates with `?`. It
//! may carry detail (a DB constraint name, an upstream message) because it
//! never reaches a client directly. `PublicError` is the *only* type ever
//! serialised to a client (see `.claude/rules/api-conventions.md`): the
//! `From<Error> for PublicError` impl below is the one deliberate place
//! internal detail is dropped, after being logged with `tracing::error!`.
//!
//! No SQL text, stack trace, or debug-format of a DB error ever reaches
//! `PublicError` — that is the whole point of the two-type split.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// A single field-level validation failure, safe to show verbatim.
#[derive(Debug, Clone)]
pub struct FieldError {
    pub field: &'static str,
    pub message: String,
}

impl FieldError {
    pub fn new(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }
}

/// Crate-internal error type. Carries enough detail for logs; never
/// serialised directly to a client — convert to `PublicError` first (`?`
/// does this automatically wherever a handler returns `Result<_, PublicError>`).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("resource not found")]
    NotFound,
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("validation failed: {0:?}")]
    Validation(Vec<FieldError>),
    /// A state conflict safe to name to the client, e.g. `SESSION_ACTIVE`.
    #[error("conflict ({code}): {message}")]
    Conflict { code: &'static str, message: String },
    /// Syntactically valid but semantically invalid input, e.g. a
    /// `program_id` that does not resolve to a real row.
    #[error("unprocessable ({code}): {message}")]
    Unprocessable { code: &'static str, message: String },
    #[error("rate limited")]
    RateLimited,
    /// An upstream dependency (Gemini, Qdrant, Redis) is unavailable. The
    /// `String` is internal detail only — logged, never shown to a client.
    #[error("upstream unavailable: {0}")]
    Unavailable(String),
    /// Anything else. The `String` is internal detail only — logged, never
    /// shown to a client.
    #[error("internal: {0}")]
    Internal(String),
}

impl From<Error> for PublicError {
    fn from(err: Error) -> Self {
        match err {
            Error::NotFound => PublicError::NotFound,
            Error::Unauthorized => PublicError::Unauthorized,
            Error::Forbidden => PublicError::Forbidden,
            Error::Validation(fields) => PublicError::Validation(fields),
            Error::Conflict { code, message } => PublicError::Conflict { code, message },
            Error::Unprocessable { code, message } => {
                PublicError::Unprocessable { code, message }
            }
            Error::RateLimited => PublicError::RateLimited,
            Error::Unavailable(detail) => {
                tracing::error!(detail = %detail, "upstream unavailable");
                PublicError::Unavailable
            }
            Error::Internal(detail) => {
                tracing::error!(detail = %detail, "internal error");
                PublicError::Internal
            }
        }
    }
}

/// The only error type ever serialised to a client. Every variant maps to a
/// stable `code()` string and a `public_message()` safe to display verbatim.
/// Per `.claude/rules/api-conventions.md` the JSON body is always exactly:
///
/// ```json
/// { "error": { "code": "SESSION_ACTIVE", "message": "..." } }
/// ```
///
/// `Internal` and `Unavailable` are intentionally opaque: they carry no
/// payload, so there is nothing on them a caller could accidentally leak.
#[derive(Debug)]
pub enum PublicError {
    NotFound,
    Unauthorized,
    Forbidden,
    Validation(Vec<FieldError>),
    Conflict { code: &'static str, message: String },
    Unprocessable { code: &'static str, message: String },
    /// The request body exceeded the route's configured size cap. Carries
    /// the limit in its message so a client can tell the user what to do
    /// about it rather than just that something failed.
    PayloadTooLarge { message: String },
    RateLimited,
    Unavailable,
    Internal,
}

impl PublicError {
    pub const SESSION_ACTIVE: &'static str = "SESSION_ACTIVE";

    /// The `409 SESSION_ACTIVE` contract required by `IMPLEMENTATION_PLAN.md`
    /// §4.1 item 6: changing academic context while a session is live.
    pub fn session_active() -> Self {
        PublicError::Conflict {
            code: Self::SESSION_ACTIVE,
            message: "Cannot change program, semester, or block during an active session.".into(),
        }
    }

    /// `413 PAYLOAD_TOO_LARGE`, naming the actual limit in MiB — a message
    /// that says only "too large" leaves the user guessing how much to cut.
    pub fn payload_too_large(max_bytes: usize) -> Self {
        let max_mib = max_bytes / (1024 * 1024);
        PublicError::PayloadTooLarge {
            message: format!("The uploaded file is larger than the {max_mib} MB limit."),
        }
    }

    /// A single-field validation error, for the common case.
    pub fn validation(field: &'static str, message: impl Into<String>) -> Self {
        PublicError::Validation(vec![FieldError::new(field, message)])
    }

    pub fn code(&self) -> &str {
        match self {
            PublicError::NotFound => "NOT_FOUND",
            PublicError::Unauthorized => "UNAUTHORIZED",
            PublicError::Forbidden => "FORBIDDEN",
            PublicError::Validation(_) => "VALIDATION_ERROR",
            PublicError::Conflict { code, .. } => code,
            PublicError::Unprocessable { code, .. } => code,
            PublicError::PayloadTooLarge { .. } => "PAYLOAD_TOO_LARGE",
            PublicError::RateLimited => "RATE_LIMITED",
            PublicError::Unavailable => "UPSTREAM_UNAVAILABLE",
            PublicError::Internal => "INTERNAL_ERROR",
        }
    }

    pub fn public_message(&self) -> String {
        match self {
            PublicError::NotFound => "The requested resource was not found.".to_string(),
            PublicError::Unauthorized => "Authentication is required.".to_string(),
            PublicError::Forbidden => {
                "You do not have permission to perform this action.".to_string()
            }
            PublicError::Validation(fields) => fields
                .iter()
                .map(|f| format!("{}: {}", f.field, f.message))
                .collect::<Vec<_>>()
                .join("; "),
            PublicError::Conflict { message, .. } => message.clone(),
            PublicError::Unprocessable { message, .. } => message.clone(),
            PublicError::PayloadTooLarge { message } => message.clone(),
            PublicError::RateLimited => "Too many requests. Please try again later.".to_string(),
            PublicError::Unavailable => {
                "The service is temporarily unavailable. Please try again shortly.".to_string()
            }
            PublicError::Internal => "Something went wrong. Please try again later.".to_string(),
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            PublicError::NotFound => StatusCode::NOT_FOUND,
            PublicError::Unauthorized => StatusCode::UNAUTHORIZED,
            PublicError::Forbidden => StatusCode::FORBIDDEN,
            PublicError::Validation(_) => StatusCode::BAD_REQUEST,
            PublicError::Conflict { .. } => StatusCode::CONFLICT,
            PublicError::Unprocessable { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            PublicError::PayloadTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
            PublicError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            PublicError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            PublicError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl std::fmt::Display for PublicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code(), self.public_message())
    }
}

impl std::error::Error for PublicError {}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: String,
}

impl IntoResponse for PublicError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: self.code(),
                message: self.public_message(),
            },
        };
        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_message_has_no_detail() {
        assert_eq!(
            PublicError::Internal.public_message(),
            "Something went wrong. Please try again later."
        );
    }

    #[test]
    fn session_active_matches_documented_contract() {
        let err = PublicError::session_active();
        assert_eq!(err.code(), "SESSION_ACTIVE");
    }

    #[test]
    fn payload_too_large_is_413_and_names_the_limit() {
        let err = PublicError::payload_too_large(64 * 1024 * 1024);
        assert_eq!(err.code(), "PAYLOAD_TOO_LARGE");
        assert_eq!(err.status(), StatusCode::PAYLOAD_TOO_LARGE);
        // The number is the point: "too large" alone tells an admin nothing
        // about how much to cut.
        assert!(err.public_message().contains("64 MB"));
    }

    #[test]
    fn validation_joins_field_messages() {
        let err = PublicError::Validation(vec![
            FieldError::new("email", "invalid format"),
            FieldError::new("roll_number", "already taken"),
        ]);
        assert_eq!(err.public_message(), "email: invalid format; roll_number: already taken");
    }
}
