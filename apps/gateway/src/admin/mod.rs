//! `/api/v1/admin/*` — CRUD for `programs`, `semesters`, `courses`, `lscs`,
//! `students`, and `users`. Every handler is behind a capability check
//! (`dg_core::Actor::require`/`require_scoped`); sub-admin routes
//! additionally filter through `sub_admin_scopes`.
//!
//! `blocks` has no admin CRUD routes in this pass: neither
//! `IMPLEMENTATION_PLAN.md` §4.2's deliverable list nor this task's endpoint
//! list mentions one, even though the `blocks` table itself is fully
//! modelled in `dg_db::models::blocks` (needed for `/me/context` and for
//! `students.current_block_id`). Noted as a gap, not silently added.

pub mod courses;
pub mod lscs;
pub mod programs;
pub mod semesters;
pub mod students;
pub mod users;

use axum::{
    routing::{get, patch, post},
    Router,
};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/programs", post(programs::create_program).get(programs::list_programs))
        .route("/programs/{id}", get(programs::get_program).patch(programs::update_program))
        .route("/semesters", post(semesters::create_semester))
        .route("/semesters/{id}", patch(semesters::update_semester))
        .route("/programs/{program_id}/semesters", get(semesters::list_semesters_for_program))
        .route("/courses", post(courses::create_course))
        .route("/courses/{id}", patch(courses::update_course))
        .route("/semesters/{semester_id}/courses", get(courses::list_courses_for_semester))
        .route("/lscs", post(lscs::create_lsc).get(lscs::list_lscs))
        .route("/lscs/{id}", patch(lscs::update_lsc))
        .route("/students", get(students::list_students))
        .route("/students/{id}", get(students::get_student))
        .route("/students/{id}/academic", patch(students::update_student_academic))
        .route("/students/{id}/current-block", patch(students::set_current_block))
        .route("/users", get(users::list_users).post(users::create_admin))
        .route("/users/{id}/status", patch(users::set_user_status))
        .route("/users/{id}/scopes", get(users::list_scopes).post(users::add_scope))
        .route("/users/{id}/scopes/{program_id}", axum::routing::delete(users::remove_scope))
}

/// Whether a student has a live `learning_sessions` row.
///
/// **Stub**: see the identical note in `me/profile.rs` — `learning_sessions`
/// writes don't exist until Phase 3. Shared here so both the self-service
/// and admin academic-update paths return the same `409 SESSION_ACTIVE`
/// contract once a real check lands; today it always returns `false`.
pub(crate) async fn session_active_stub(
    _state: &AppState,
    _student_id: dg_core::StudentId,
) -> Result<bool, dg_core::PublicError> {
    Ok(false)
}
