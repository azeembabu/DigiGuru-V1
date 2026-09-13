//! `/api/v1/admin/exams` — the admin side of block assessments: create and
//! read exams, and read the attempts at one of them.
//!
//! An exam has no `program_id` of its own, so scope is resolved one level
//! deeper than `admin/blocks.rs` does it — `exam -> block -> course ->
//! program_id` — and, exactly as there, the owning program is resolved
//! *before* the capability check so a sub-admin outside the scope cannot
//! distinguish an existing exam from a missing one.
//!
//! Both lists here can grow without bound (a block accumulates exams; a
//! popular exam accumulates an attempt per student per sitting), so both
//! follow the shared paginated-list contract: bare array body, `X-Total-Count`,
//! validated — never clamped — `limit`/`offset`.

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use dg_core::{BlockId, Capability, ExamAttemptStatus, ExamId, ExamStatus, PublicError};
use dg_db::models::{blocks, exams};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

/// An exam plus the flat ancestry of its block, so a console renders a
/// breadcrumb from one response — the same shape choice `BlockResponse`
/// makes, and for the same reason.
#[derive(Debug, Serialize)]
pub struct ExamResponse {
    pub id: Uuid,
    pub block_id: Uuid,
    pub block_no: i16,
    pub block_title: String,
    pub course_id: Uuid,
    pub course_code: String,
    pub semester_id: Uuid,
    pub program_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    /// A number, never a rendered "18/20" — formatting belongs to the UI.
    pub max_score: f64,
    /// `null` = untimed.
    pub duration_minutes: Option<i16>,
    pub status: ExamStatus,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

impl From<exams::Exam> for ExamResponse {
    fn from(e: exams::Exam) -> Self {
        Self {
            id: e.id.into_uuid(),
            block_id: e.block_id.into_uuid(),
            block_no: e.block_no,
            block_title: e.block_title,
            course_id: e.course_id.into_uuid(),
            course_code: e.course_code,
            semester_id: e.semester_id.into_uuid(),
            program_id: e.program_id.into_uuid(),
            title: e.title,
            description: e.description,
            max_score: e.max_score,
            duration_minutes: e.duration_minutes,
            status: e.status,
            created_by: e.created_by.into_uuid(),
            created_at: e.created_at,
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateExamRequest {
    pub block_id: Uuid,
    #[validate(length(min = 2, max = 200))]
    pub title: String,
    pub description: Option<String>,
    /// `UNIQUE (block_id, title)` in the schema, so a duplicate title is a
    /// DB-level conflict rather than something validated here.
    #[validate(range(min = 0.01, max = 10000.0))]
    pub max_score: f64,
    #[validate(range(min = 1, max = 600))]
    pub duration_minutes: Option<i16>,
    /// Omitted means `draft` — an exam is not visible to students until an
    /// admin deliberately publishes it.
    pub status: Option<ExamStatus>,
}

/// An omitted `status` means `draft`: a new exam is never live to students
/// until an admin publishes it deliberately.
fn requested_status(status: Option<ExamStatus>) -> ExamStatus {
    status.unwrap_or(ExamStatus::Draft)
}

pub async fn create_exam(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Json(payload): Json<CreateExamRequest>,
) -> Result<Json<ExamResponse>, PublicError> {
    payload
        .validate()
        .map_err(|_| PublicError::validation("title", "invalid exam fields"))?;

    let block_id = BlockId::from(payload.block_id);
    let program_id = blocks::program_id_for_block(&state.pool, block_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManagePrograms, program_id)?;

    let exam = exams::create(
        &state.pool,
        block_id,
        &payload.title,
        payload.description.as_deref(),
        payload.max_score,
        payload.duration_minutes,
        requested_status(payload.status),
        actor.user_id,
    )
    .await
    .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.exam_created",
        None,
        None,
        Some(serde_json::json!({ "exam_id": exam.id, "block_id": block_id })),
    )
    .await;

    Ok(Json(exam.into()))
}

#[derive(Debug, Deserialize)]
pub struct ListExamsQuery {
    /// Case-insensitive substring over the exam title; blank means no filter.
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

pub async fn list_exams_for_block(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(block_id): Path<Uuid>,
    Query(query): Query<ListExamsQuery>,
) -> Result<(HeaderMap, Json<Vec<ExamResponse>>), PublicError> {
    let block_id = BlockId::from(block_id);
    let program_id = blocks::program_id_for_block(&state.pool, block_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManagePrograms, program_id)?;

    let page = super::page(query.limit, query.offset, 50)?;
    let q = super::search_term(query.q.as_deref());

    let total = exams::count_by_block(&state.pool, block_id, q)
        .await
        .map_err(PublicError::from)?;
    let rows = exams::list_by_block(&state.pool, block_id, q, page.limit, page.offset)
        .await
        .map_err(PublicError::from)?;

    Ok((
        super::total_count(total),
        Json(rows.into_iter().map(ExamResponse::from).collect()),
    ))
}

pub async fn get_exam(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<ExamResponse>, PublicError> {
    let exam_id = ExamId::from(id);
    let program_id = exams::program_id_for_exam(&state.pool, exam_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManagePrograms, program_id)?;

    let exam = exams::find_by_id(&state.pool, exam_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    Ok(Json(exam.into()))
}

/// One attempt at an exam, denormalised across the student who sat it. This
/// is the **admin** view; the student's own card is a different shape (see
/// `student/exams.rs`) and deliberately never carries another student's
/// identity.
#[derive(Debug, Serialize)]
pub struct ExamAttemptResponse {
    pub id: Uuid,
    pub exam_id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub roll_number: String,
    pub attempt_no: i16,
    /// `null` until the attempt is marked.
    pub score: Option<f64>,
    pub max_score: f64,
    pub status: ExamAttemptStatus,
    pub started_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
}

impl From<exams::ExamAttemptRow> for ExamAttemptResponse {
    fn from(a: exams::ExamAttemptRow) -> Self {
        Self {
            id: a.id.into_uuid(),
            exam_id: a.exam_id.into_uuid(),
            student_id: a.student_id.into_uuid(),
            student_name: a.student_name,
            roll_number: a.roll_number,
            attempt_no: a.attempt_no,
            score: a.score,
            max_score: a.max_score,
            status: a.status,
            started_at: a.started_at,
            submitted_at: a.submitted_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListAttemptsQuery {
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

pub async fn list_attempts_for_exam(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(exam_id): Path<Uuid>,
    Query(query): Query<ListAttemptsQuery>,
) -> Result<(HeaderMap, Json<Vec<ExamAttemptResponse>>), PublicError> {
    let exam_id = ExamId::from(exam_id);
    let program_id = exams::program_id_for_exam(&state.pool, exam_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManagePrograms, program_id)?;

    let page = super::page(query.limit, query.offset, 50)?;

    let total = exams::count_attempts_for_exam(&state.pool, exam_id)
        .await
        .map_err(PublicError::from)?;
    let rows = exams::attempts_for_exam(&state.pool, exam_id, page.limit, page.offset)
        .await
        .map_err(PublicError::from)?;

    Ok((
        super::total_count(total),
        Json(rows.into_iter().map(ExamAttemptResponse::from).collect()),
    ))
}

#[cfg(test)]
mod tests {
    use dg_core::{Actor, Capability, ExamStatus, ProgramId, Role, UserId};

    /// The single authorization rule every route in this module applies once
    /// `exam -> block -> course -> program_id` has been resolved. Asserting it
    /// directly is what makes the matrix below a test of *this* module rather
    /// than of `dg_core::Actor` in the abstract.
    fn authorize(actor: &Actor, program_id: ProgramId) -> Result<(), dg_core::PublicError> {
        actor.require_scoped(Capability::ManagePrograms, program_id)
    }

    #[test]
    fn super_admin_may_manage_exams_in_any_program() {
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert!(authorize(&actor, ProgramId::new()).is_ok());
    }

    #[test]
    fn sub_admin_may_manage_exams_only_in_scoped_programs() {
        let scoped = ProgramId::new();
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![scoped]);
        assert!(authorize(&actor, scoped).is_ok());
    }

    #[test]
    fn sub_admin_is_forbidden_exams_outside_its_scope() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        let err = authorize(&actor, ProgramId::new()).expect_err("out of scope");
        assert_eq!(err.code(), "FORBIDDEN");
    }

    #[test]
    fn sub_admin_with_no_scopes_is_forbidden_every_exam() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![]);
        assert!(authorize(&actor, ProgramId::new()).is_err());
    }

    #[test]
    fn student_is_forbidden_every_admin_exam_route() {
        let program_id = ProgramId::new();
        // A student holding the program in `scopes` still fails: `in_scope`
        // is false for `Role::Student` regardless of what the vector says.
        let actor = Actor::new(UserId::new(), Role::Student, vec![program_id]);
        let err = authorize(&actor, program_id).expect_err("students never manage exams");
        assert_eq!(err.code(), "FORBIDDEN");
    }

    #[test]
    fn student_is_forbidden_reading_attempts_of_an_exam_they_sat() {
        // The admin attempts list is scoped by program, not by whose attempts
        // they are: a student must not reach it even for their own exam.
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert!(actor.require(Capability::ManagePrograms).is_err());
    }

    #[test]
    fn an_exam_defaults_to_draft_when_status_is_omitted() {
        assert_eq!(super::requested_status(None), ExamStatus::Draft);
        assert_eq!(
            super::requested_status(Some(ExamStatus::Published)),
            ExamStatus::Published,
            "an explicit status is still honoured"
        );
    }
}
