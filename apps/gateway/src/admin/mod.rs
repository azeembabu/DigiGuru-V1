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

pub mod analytics;
pub mod blocks;
pub mod board_events;
pub mod courses;
pub mod documents;
pub mod enrollments;
pub mod exams;
pub mod lscs;
pub mod performance;
pub mod programs;
pub mod question_import;
pub mod removal;
pub mod questions;
pub mod safety_incidents;
pub mod semesters;
pub mod sessions;
pub mod stats;
pub mod student_reports;
pub mod students;
pub mod users;

use axum::{
    extract::DefaultBodyLimit,
    http::{header::HeaderName, HeaderMap, HeaderValue},
    routing::{get, patch, post},
    Router,
};

use dg_core::PublicError;

use crate::state::AppState;

/// `max_upload_bytes` is applied as a `DefaultBodyLimit` to the document
/// upload route **only**. Raising axum's global 2 MiB default instead would
/// let every JSON endpoint — `/auth/login` included — buffer a body that
/// large, which is a DoS surface, not a fix.
pub fn router(max_upload_bytes: usize) -> Router<AppState> {
    Router::new()
        .route(
            "/programs",
            post(programs::create_program).get(programs::list_programs),
        )
        .route(
            "/programs/{id}",
            get(programs::get_program)
                .patch(programs::update_program)
                .delete(removal::delete_program),
        )
        // Permanent removal, and what it would destroy. The preview is not a
        // lock on the delete — a script can skip it — it is what lets the
        // console show an admin the cost before they type the name back.
        .route(
            "/programs/{id}/deletion-impact",
            get(removal::program_impact),
        )
        .route("/courses/{id}/deletion-impact", get(removal::course_impact))
        .route("/blocks/{id}/deletion-impact", get(removal::block_impact))
        .route("/documents/{id}/deletion-impact", get(removal::unit_impact))
        .route("/semesters", post(semesters::create_semester))
        .route(
            "/semesters/{id}",
            get(semesters::get_semester).patch(semesters::update_semester),
        )
        .route(
            "/programs/{program_id}/semesters",
            get(semesters::list_semesters_for_program),
        )
        .route("/courses", post(courses::create_course))
        .route(
            "/courses/{id}",
            get(courses::get_course)
                .patch(courses::update_course)
                .delete(removal::delete_course),
        )
        .route(
            "/semesters/{semester_id}/courses",
            get(courses::list_courses_for_semester),
        )
        .route(
            "/programs/{program_id}/courses",
            get(courses::list_courses_for_program),
        )
        .route("/lscs", post(lscs::create_lsc).get(lscs::list_lscs))
        .route("/lscs/{id}", patch(lscs::update_lsc))
        .route("/students", get(students::list_students))
        .route("/students/{id}", get(students::get_student))
        .route(
            "/students/{id}/academic",
            patch(students::update_student_academic),
        )
        .route(
            "/students/{id}/current-block",
            patch(students::set_current_block),
        )
        .route("/blocks", post(blocks::create_block))
        .route(
            "/blocks/{id}",
            get(blocks::get_block)
                .patch(blocks::update_block)
                .delete(removal::delete_block),
        )
        .route(
            "/courses/{course_id}/blocks",
            get(blocks::list_blocks_for_course),
        )
        .route(
            "/blocks/{block_id}/documents",
            post(documents::upload)
                .get(documents::list_documents_for_block)
                // Applies to the GET too, harmlessly: it has no body.
                .layer(DefaultBodyLimit::max(max_upload_bytes)),
        )
        .route(
            "/documents/{id}",
            get(documents::get_document).delete(removal::delete_unit),
        )
        // A separate path segment rather than a `PATCH /documents/{id}`: the
        // rest of a document row is written by the ingestion pipeline, not by
        // an admin, so the one editable field gets its own route instead of a
        // general document patch that would invite the others to be added to it.
        .route("/documents/{id}/video", patch(documents::set_video))
        // Exams hang off a block, like documents do; scope is resolved
        // `exam -> block -> course -> program_id`.
        .route("/exams", post(exams::create_exam))
        .route("/exams/{id}", get(exams::get_exam))
        .route(
            "/exams/{exam_id}/attempts",
            get(exams::list_attempts_for_exam),
        )
        .route("/blocks/{block_id}/exams", get(exams::list_exams_for_block))
        // The MCQ bank the exam module samples from. Scoped
        // `question -> block -> course -> program_id`, like exams.
        .route(
            "/question-pool",
            post(questions::create_question).get(questions::list_questions),
        )
        .route(
            "/question-pool/bulk",
            // Its own body cap, like the document upload's: a 500-row import is
            // far larger than axum's 2 MiB default allows, and raising that
            // default globally would widen the DoS surface on `/auth/*`.
            post(questions::bulk_import_questions)
                .layer(DefaultBodyLimit::max(questions::MAX_BULK_IMPORT_BYTES)),
        )
        .route("/question-pool/{id}", patch(questions::update_question))
        // Per-course active counts for one program, empty courses included, so
        // the Program -> Semester -> Course navigation can show which pools are
        // still empty in one request instead of one per course.
        .route(
            "/programs/{program_id}/question-pool-counts",
            get(questions::list_pool_counts),
        )
        // Student Reports: the same `exam_attempts` rows the student's own
        // submit writes, scoped like `/analytics`. No second store.
        .route(
            "/student-reports",
            get(student_reports::list_student_reports),
        )
        .route(
            "/students/{student_id}/report",
            get(student_reports::get_student_report),
        )
        // Performance: one student's record, and the best-performers board
        // the students page shows. Both derive from the same attempts the
        // student's own assessment page reads, so the two cannot disagree.
        .route(
            "/students/{student_id}/performance",
            get(performance::get_student_performance),
        )
        .route(
            "/students/{student_id}/unit-assessments",
            get(performance::get_student_unit_assessments),
        )
        .route("/top-performers", get(performance::list_top_performers))
        .route("/stats", get(stats::get_stats))
        .route("/analytics", get(analytics::get_analytics))
        // Drill-downs behind the dashboard tiles: each opens the records
        // behind one number. All four are plain paginated admin lists.
        .route("/enrollments", get(enrollments::list_enrollments))
        .route("/sessions", get(sessions::list_sessions))
        .route("/board-events", get(board_events::list_board_events))
        .route(
            "/safety-incidents",
            get(safety_incidents::list_safety_incidents),
        )
        .route("/users", get(users::list_users).post(users::create_admin))
        .route("/users/{id}/status", patch(users::set_user_status))
        .route(
            "/users/{id}/scopes",
            get(users::list_scopes).post(users::add_scope),
        )
        .route(
            "/users/{id}/scopes/{program_id}",
            axum::routing::delete(users::remove_scope),
        )
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
