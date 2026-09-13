//! `/api/v1/student/*` — routes that serve a student their *own* records.
//!
//! Distinct from `/me/*`, which answers "who and where am I" (identity,
//! academic context, profile). This namespace is for a student's accumulated
//! records — exam attempts today, progress and note reminders later — which
//! are lists rather than a single self-describing payload and so need the
//! pagination contract the `/me` routes have no use for.
//!
//! Every route here is **self-only**: the subject is resolved from the
//! caller's own access token, never from a path or query parameter, so no
//! request shape exists that could name another student. See `exams.rs`.

pub mod exams;

use axum::{routing::get, Router};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/exam-attempts", get(exams::list_my_attempts))
        .route("/exam-attempts/{id}", get(exams::get_my_attempt))
}
