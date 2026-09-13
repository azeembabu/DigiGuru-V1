//! `/api/v1/student/exam-attempts` — the student's own exam record: the
//! "recent exams" score cards on the dashboard, and the single attempt a card
//! links to for review.
//!
//! # Self-only
//!
//! There is no `student_id` anywhere in either route's path, query, or body.
//! The subject is resolved from the caller's own access token
//! (`students::find_by_user_id(actor.user_id)`) and bound into the query's
//! `WHERE` clause by `dg_db::models::exams`, which exposes no function able
//! to return attempts for an unspecified student. So "read another student's
//! attempts" is not a request a client can express — it is not a check that
//! could be forgotten, which is the point (`.claude/rules/security.md`).
//!
//! An attempt id belonging to someone else therefore selects no row and comes
//! back `404 NOT_FOUND`, not `403`: a `403` would confirm that the id exists,
//! which is itself a small leak about another student's record. That is
//! asserted end-to-end against a real Postgres in
//! `crates/db/tests/exam_attempts.rs`, which is the only place it can be
//! proved — the guarantee lives in a `WHERE` clause, not in a branch here.

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::{Capability, ExamAttemptId, ExamAttemptStatus, PublicError, StudentId};
use dg_db::models::{exams, students};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

/// One score card. Carries the exam, block and course labels denormalised by
/// the join the query already makes — a page of cards is one request, not one
/// request per card.
///
/// Deliberately *not* the admin `ExamAttemptResponse`: this shape has no
/// student name or roll number on it at all, so there is no field here that
/// could ever carry another student's identity.
#[derive(Debug, Serialize)]
pub struct MyExamAttemptResponse {
    pub id: Uuid,
    pub exam_id: Uuid,
    pub exam_title: String,
    pub block_id: Uuid,
    pub block_no: i16,
    pub block_title: String,
    pub course_id: Uuid,
    pub course_code: String,
    pub course_name: String,
    /// Which sitting this was, 1-based. Two attempts at the same exam differ
    /// here, so a card can say "attempt 2" rather than showing two rows a
    /// student cannot tell apart.
    pub attempt_no: i16,
    /// `null` until the attempt is marked. A number, never a rendered
    /// "18/20" — the card formats `score`/`max_score` itself.
    pub score: Option<f64>,
    pub max_score: f64,
    pub status: ExamAttemptStatus,
    pub started_at: DateTime<Utc>,
    /// `null` while the attempt is still open.
    pub submitted_at: Option<DateTime<Utc>>,
}

impl From<exams::StudentAttemptRow> for MyExamAttemptResponse {
    fn from(a: exams::StudentAttemptRow) -> Self {
        Self {
            id: a.id.into_uuid(),
            exam_id: a.exam_id.into_uuid(),
            exam_title: a.exam_title,
            block_id: a.block_id.into_uuid(),
            block_no: a.block_no,
            block_title: a.block_title,
            course_id: a.course_id.into_uuid(),
            course_code: a.course_code,
            course_name: a.course_name,
            attempt_no: a.attempt_no,
            score: a.score,
            max_score: a.max_score,
            status: a.status,
            started_at: a.started_at,
            submitted_at: a.submitted_at,
        }
    }
}

/// Resolve the caller's own `students` row.
///
/// The one place a `StudentId` enters either handler, and it comes from the
/// token. An admin has no `students` row and so gets `404` here rather than
/// another student's data — the capability check above it has already
/// rejected them, this is the second lock.
async fn own_student_id(state: &AppState, actor: &dg_core::Actor) -> Result<StudentId, PublicError> {
    let student = students::find_by_user_id(&state.pool, actor.user_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;
    Ok(student.id)
}

#[derive(Debug, Deserialize)]
pub struct ListMyAttemptsQuery {
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

/// `GET /api/v1/student/exam-attempts` — this student's attempts, newest
/// first, as a paginated bare array with `X-Total-Count`, like every other
/// list in this API.
pub async fn list_my_attempts(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<ListMyAttemptsQuery>,
) -> Result<(HeaderMap, Json<Vec<MyExamAttemptResponse>>), PublicError> {
    actor.require(Capability::ViewOwnExams)?;

    let student_id = own_student_id(&state, &actor).await?;
    let page = crate::admin::page(query.limit, query.offset, 50)?;

    let total = exams::count_attempts_for_student(&state.pool, student_id)
        .await
        .map_err(PublicError::from)?;
    let rows = exams::attempts_for_student(&state.pool, student_id, page.limit, page.offset)
        .await
        .map_err(PublicError::from)?;

    Ok((
        crate::admin::total_count(total),
        Json(rows.into_iter().map(MyExamAttemptResponse::from).collect()),
    ))
}

/// `GET /api/v1/student/exam-attempts/{id}` — one attempt, in the same shape
/// the list returns, so a review screen codes against one card type.
pub async fn get_my_attempt(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<MyExamAttemptResponse>, PublicError> {
    actor.require(Capability::ViewOwnExams)?;

    let student_id = own_student_id(&state, &actor).await?;

    // Ownership is a predicate inside the query, not a comparison made on a
    // row already fetched: another student's id selects nothing.
    let attempt = exams::attempt_for_student(&state.pool, student_id, ExamAttemptId::from(id))
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    Ok(Json(attempt.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dg_core::{Actor, ProgramId, Role, UserId};

    #[test]
    fn a_student_may_read_its_own_exam_attempts() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert!(actor.require(Capability::ViewOwnExams).is_ok());
    }

    /// The security property of this module, asserted at the level it is
    /// actually enforced: the request shape carries no student selector, so
    /// the only `StudentId` reaching the query is the caller's own.
    ///
    /// `ListMyAttemptsQuery` is exhaustively destructured on purpose — adding
    /// a `student_id` field to it would stop this test compiling, which is
    /// the failure mode worth catching at review time.
    #[test]
    fn the_list_query_cannot_name_another_student() {
        let ListMyAttemptsQuery { limit, offset } = ListMyAttemptsQuery {
            limit: Some(10),
            offset: Some(0),
        };
        assert_eq!(limit, Some(10));
        assert_eq!(offset, Some(0));
    }

    #[test]
    fn an_admin_holds_no_self_only_exam_capability() {
        // A sub-admin has no `students` row to be self-only about; its view
        // of attempts is the scoped `/admin/exams/{id}/attempts` list.
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        let err = actor
            .require(Capability::ViewOwnExams)
            .expect_err("no own exams for an admin");
        assert_eq!(err.code(), "FORBIDDEN");
    }

    #[test]
    fn my_attempts_reject_an_out_of_range_limit_rather_than_clamping() {
        let err = crate::admin::page(Some(201), None, 50).expect_err("over the ceiling");
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn a_card_carries_no_student_identity() {
        // Serialising a row must not produce a name or roll number: the
        // student-facing shape has no such field, and this pins that.
        let json = serde_json::to_value(MyExamAttemptResponse {
            id: Uuid::nil(),
            exam_id: Uuid::nil(),
            exam_title: "Unit 3 Test".into(),
            block_id: Uuid::nil(),
            block_no: 3,
            block_title: "Unit 3".into(),
            course_id: Uuid::nil(),
            course_code: "ML101".into(),
            course_name: "Malayalam Literature".into(),
            attempt_no: 2,
            score: Some(18.0),
            max_score: 20.0,
            status: ExamAttemptStatus::Graded,
            started_at: Utc::now(),
            submitted_at: Some(Utc::now()),
        })
        .expect("serialises");

        assert!(json.get("student_name").is_none());
        assert!(json.get("roll_number").is_none());
        assert!(json.get("student_id").is_none());
        // Scores stay numbers on the wire; "18/20" is the UI's job.
        assert!(json["score"].is_number());
        assert!(json["max_score"].is_number());
    }
}
