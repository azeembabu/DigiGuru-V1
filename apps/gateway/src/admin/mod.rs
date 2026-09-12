//! `/api/v1/admin/*` — CRUD for `programs`, `semesters`, `courses`, `lscs`,
//! `students`, and `users`. Every handler is behind a capability check
//! (`dg_core::Actor::require`/`require_scoped`); sub-admin routes
//! additionally filter through `sub_admin_scopes`.
//!
//! Paginated list routes (`programs`, `lscs`, `students`, `users`) share the
//! `page` helper below and report their unfiltered total in an
//! `X-Total-Count` response header — see `.claude/rules/api-conventions.md`.
//! The header, rather than an envelope object, is deliberate: these routes
//! have already shipped a bare JSON array body, and reshaping that body
//! would break every existing caller.

pub mod blocks;
pub mod courses;
pub mod documents;
pub mod lscs;
pub mod programs;
pub mod semesters;
pub mod stats;
pub mod students;
pub mod users;

use axum::{
    http::{header::HeaderName, HeaderMap, HeaderValue},
    routing::{get, patch, post},
    Router,
};

use dg_core::PublicError;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/programs", post(programs::create_program).get(programs::list_programs))
        .route("/programs/{id}", get(programs::get_program).patch(programs::update_program))
        .route("/semesters", post(semesters::create_semester))
        .route("/semesters/{id}", get(semesters::get_semester).patch(semesters::update_semester))
        .route("/programs/{program_id}/semesters", get(semesters::list_semesters_for_program))
        .route("/courses", post(courses::create_course))
        .route("/courses/{id}", get(courses::get_course).patch(courses::update_course))
        .route("/semesters/{semester_id}/courses", get(courses::list_courses_for_semester))
        .route("/programs/{program_id}/courses", get(courses::list_courses_for_program))
        .route("/lscs", post(lscs::create_lsc).get(lscs::list_lscs))
        .route("/lscs/{id}", patch(lscs::update_lsc))
        .route("/students", get(students::list_students))
        .route("/students/{id}", get(students::get_student))
        .route("/students/{id}/academic", patch(students::update_student_academic))
        .route("/students/{id}/current-block", patch(students::set_current_block))
        .route("/blocks", post(blocks::create_block))
        .route("/blocks/{id}", get(blocks::get_block).patch(blocks::update_block))
        .route("/courses/{course_id}/blocks", get(blocks::list_blocks_for_course))
        .route(
            "/blocks/{block_id}/documents",
            post(documents::upload).get(documents::list_documents_for_block),
        )
        .route("/documents/{id}", get(documents::get_document))
        .route("/stats", get(stats::get_stats))
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

/// The ceiling every paginated admin list shares.
pub(crate) const MAX_PAGE_LIMIT: i64 = 200;

/// A validated `limit`/`offset` pair.
#[derive(Debug)]
pub(crate) struct Page {
    pub limit: i64,
    pub offset: i64,
}

/// Validate `limit`/`offset` from a list query.
///
/// Out-of-range values are rejected as `400 VALIDATION_ERROR` rather than
/// clamped: a silent clamp hands the caller a page it did not ask for and
/// a `X-Total-Count` it cannot reconcile with the rows it got back, which
/// reads as a pagination bug on the client side rather than as a rejected
/// request.
pub(crate) fn page(
    limit: Option<i64>,
    offset: Option<i64>,
    default_limit: i64,
) -> Result<Page, PublicError> {
    let limit = limit.unwrap_or(default_limit);
    if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
        return Err(PublicError::validation(
            "limit",
            format!("must be between 1 and {MAX_PAGE_LIMIT}"),
        ));
    }

    let offset = offset.unwrap_or(0);
    if offset < 0 {
        return Err(PublicError::validation("offset", "must be zero or greater"));
    }

    Ok(Page { limit, offset })
}

/// Normalise a free-text search parameter: a blank or whitespace-only `q`
/// means "no filter", not "match rows containing an empty string".
pub(crate) fn search_term(q: Option<&str>) -> Option<&str> {
    q.map(str::trim).filter(|s| !s.is_empty())
}

/// The `X-Total-Count` header carrying a list's total before pagination.
pub(crate) fn total_count(total: i64) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-total-count"),
        HeaderValue::from(total),
    );
    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_defaults_when_unspecified() {
        let p = page(None, None, 50).expect("defaults are valid");
        assert_eq!(p.limit, 50);
        assert_eq!(p.offset, 0);
    }

    #[test]
    fn page_rejects_limit_above_the_ceiling() {
        let err = page(Some(MAX_PAGE_LIMIT + 1), None, 50).expect_err("over the ceiling");
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn page_rejects_zero_and_negative_limit() {
        assert!(page(Some(0), None, 50).is_err());
        assert!(page(Some(-1), None, 50).is_err());
    }

    #[test]
    fn page_rejects_negative_offset() {
        let err = page(None, Some(-1), 50).expect_err("negative offset");
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn blank_search_term_is_no_filter() {
        assert_eq!(search_term(Some("   ")), None);
        assert_eq!(search_term(Some("")), None);
        assert_eq!(search_term(None), None);
        assert_eq!(search_term(Some("  malayalam ")), Some("malayalam"));
    }
}
