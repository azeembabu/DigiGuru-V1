//! `db`'s own error type (per `.claude/rules/code-style.md`: "every crate
//! exposes its own `Error` enum"), plus the conversion into `core::Error`
//! and directly into `core::PublicError` for handler ergonomics.
//!
//! Nothing here ever carries raw SQL text or a full `sqlx::Error` debug dump
//! into a `PublicError` — `sqlx::Error`'s `Display` (used for `#[from]`
//! logging via `Error::Internal` below) is logged with `tracing`, never
//! serialised to a client.

use dg_core::error::PublicError;

/// Internal error type for every `db::*` query module.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("resource not found")]
    NotFound,

    /// A `UNIQUE` constraint was violated. `constraint` is the Postgres
    /// constraint name — logged, never shown to a client verbatim; callers
    /// that need a client-safe message (e.g. "email already registered")
    /// should match on this variant themselves before it reaches `?`.
    #[error("unique constraint violated: {constraint}")]
    UniqueViolation { constraint: String },

    /// A `FOREIGN KEY` constraint was violated — e.g. a `program_id` that
    /// does not resolve to a real row.
    #[error("foreign key violated: {constraint}")]
    ForeignKeyViolation { constraint: String },

    #[error("database error")]
    Sqlx(#[from] sqlx::Error),
}

impl Error {
    /// Classify a raw `sqlx::Error` into one of the structured variants
    /// above where possible, falling back to the opaque `Sqlx` variant.
    pub fn from_sqlx(err: sqlx::Error) -> Self {
        match &err {
            sqlx::Error::RowNotFound => Error::NotFound,
            sqlx::Error::Database(db_err) => {
                let constraint = db_err.constraint().map(str::to_owned);
                match (db_err.code().as_deref(), constraint) {
                    // Postgres SQLSTATE 23505 = unique_violation
                    (Some("23505"), Some(constraint)) => Error::UniqueViolation { constraint },
                    // Postgres SQLSTATE 23503 = foreign_key_violation
                    (Some("23503"), Some(constraint)) => {
                        Error::ForeignKeyViolation { constraint }
                    }
                    _ => Error::Sqlx(err),
                }
            }
            _ => Error::Sqlx(err),
        }
    }
}

/// Local-type conversion (`Error` is defined in this crate), legal under the
/// orphan rule even though `core::Error` is foreign. Routes every DB failure
/// through the single internal `core::Error` funnel described in
/// `crates/core/src/error.rs`.
impl From<Error> for dg_core::error::Error {
    fn from(err: Error) -> Self {
        match err {
            Error::NotFound => dg_core::error::Error::NotFound,
            Error::UniqueViolation { constraint } => {
                dg_core::error::Error::Internal(format!("unique violation: {constraint}"))
            }
            Error::ForeignKeyViolation { constraint } => {
                tracing::warn!(
                    constraint = %constraint,
                    "foreign key violation mapped to INVALID_REFERENCE"
                );
                dg_core::error::Error::Unprocessable {
                    code: "INVALID_REFERENCE",
                    message: "One or more referenced records do not exist.".into(),
                }
            }
            Error::Sqlx(inner) => dg_core::error::Error::Internal(inner.to_string()),
        }
    }
}

/// Convenience: let handlers `?` a `db::Error` straight into `PublicError`
/// without an explicit intermediate `core::Error` step.
impl From<Error> for PublicError {
    fn from(err: Error) -> Self {
        dg_core::error::Error::from(err).into()
    }
}

pub type Result<T> = std::result::Result<T, Error>;
