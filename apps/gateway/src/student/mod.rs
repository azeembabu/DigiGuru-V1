//! `/api/v1/student/*` — routes that serve a student their *own* records.
//!
//! Distinct from `/me/*`, which answers "who and where am I" (identity,
//! academic context, profile). This namespace is for a student's accumulated
//! records — exam attempts, the dashboard roll-up, the flashcard notebook, the
//! revision queue — which are lists and aggregates rather than a single
//! self-describing payload, and so need the pagination contract the `/me`
//! routes have no use for.
//!
//! Every route here is **self-only**: the subject is resolved from the caller's
//! own access token by [`own_student`], never from a path or query parameter, so
//! no request shape exists that could name another student. See `exams.rs` for
//! the long-form argument, and `exam_module.rs` for why the answer key cannot
//! leak through a response type.

pub mod dashboard;
pub mod exam_module;
pub mod exams;
pub mod flashcards;
pub mod queries;
pub mod revisions;

use axum::{
    routing::{get, patch, post},
    Router,
};

use dg_core::PublicError;
use dg_db::models::students;

use crate::state::AppState;

/// Resolve the caller's own `students` row.
///
/// The single place a `StudentId` enters any handler in this module, and it
/// comes from the validated access token — not from the request. The row is
/// returned whole rather than just its id because the self-only routes need
/// `timezone` (NN-3 day boundaries, flashcard and revision due dates) and
/// `program_id`/`semester_id` (exam sampling) from the same lookup.
///
/// An admin has no `students` row and so gets `404` here. That is the second
/// lock: the capability check in the handler has already rejected them.
pub(crate) async fn own_student(
    state: &AppState,
    actor: &dg_core::Actor,
) -> Result<students::Student, PublicError> {
    students::find_by_user_id(&state.pool, actor.user_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/dashboard", get(dashboard::get_dashboard))
        .route("/revisions", get(revisions::list_revisions))
        .route("/flashcards", get(flashcards::list_flashcards))
        .route(
            "/flashcards/{id}/review",
            post(flashcards::review_flashcard),
        )
        // The exam runner. `/exams/attempts/...` rather than
        // `/exams/{exam_id}/attempts/{id}`: an attempt id is unique on its own
        // and already carries its exam, so repeating the exam in the path would
        // create a second, un-checkable way to address the same row.
        .route("/exams", get(exam_module::list_available_exams))
        .route(
            "/exams/{exam_id}/attempts",
            post(exam_module::start_attempt),
        )
        .route("/exams/attempts/{id}", get(exam_module::resume_attempt))
        .route(
            "/exams/attempts/{id}/answers",
            patch(exam_module::save_answers),
        )
        .route(
            "/exams/attempts/{id}/submit",
            post(exam_module::submit_attempt),
        )
        // The historical record, as distinct from sitting a paper.
        .route("/exam-attempts", get(exams::list_my_attempts))
        .route("/exam-attempts/{id}", get(exams::get_my_attempt))
}
