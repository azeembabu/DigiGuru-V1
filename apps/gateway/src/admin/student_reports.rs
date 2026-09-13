//! `/api/v1/admin/student-reports` and
//! `/api/v1/admin/students/{student_id}/report` — the admin's view of what
//! students have actually sat.
//!
//! There is **no new store behind this**. Both routes read the same
//! `exam_attempts` and `exam_attempt_answers` rows the student's own submit
//! writes (`student/exam_module.rs`), which is the whole reason the contract has
//! no `sync_targets`: a queue or a mirrored gradebook would only create a way
//! for the admin's numbers and the student's to disagree.
//!
//! `time_spent_seconds` is likewise **derived** — `submitted_at - started_at` —
//! rather than stored. A duration column would have to be kept true by every
//! write path that touches either timestamp, and would silently drift the first
//! time one of them was corrected.
//!
//! Scoping follows `/admin/analytics` exactly: the caller's role decides *which
//! query runs*, never a filter applied to a platform-wide result. A sub-admin
//! sees only its `sub_admin_scopes` programs, reached by
//! `exam_attempts -> students -> program_id`, and a sub-admin with no scopes
//! gets an empty array — the correct answer, not a reason to fall back to the
//! unscoped query. A student outside the scope is `404`, never `403`, because a
//! `403` would confirm the id exists.

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::{
    AssessmentType, Capability, CourseId, ExamAttemptStatus, ExamId, ProgramId, PublicError, Role,
    SemesterId, StudentId,
};
use dg_db::models::student_reports as reports;

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

/// How many weak topics the per-student report returns. The screen shows a
/// badge list, not a histogram of every topic ever missed.
const WEAK_TOPIC_LIMIT: i64 = 20;

/// The sub-admin scope for a report query, or `None` for the unscoped one.
///
/// Identical in shape and reasoning to `questions::list_scope`; kept separate
/// rather than shared because these two modules' `Role::Student` arms answer
/// for different routes and collapsing them would hide that.
fn report_scope(actor: &dg_core::Actor) -> Result<Option<Vec<Uuid>>, PublicError> {
    match actor.role {
        Role::SuperAdmin => Ok(None),
        Role::SubAdmin => Ok(Some(actor.scopes.iter().map(|p| p.into_uuid()).collect())),
        // Unreachable via the capability check above every caller; matched
        // explicitly so a future capability change cannot silently hand a
        // student the unscoped query.
        Role::Student => Err(PublicError::Forbidden),
    }
}

// ---------------------------------------------------------------------------
// GET /admin/student-reports
// ---------------------------------------------------------------------------

/// One attempt, denormalised across student, programme, course and exam so a
/// row renders without a second request.
#[derive(Debug, Serialize)]
pub struct AttemptReportResponse {
    pub attempt_id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    pub roll_number: String,
    pub program_id: Uuid,
    pub program_name: String,
    pub semester_number: i16,
    pub course_id: Uuid,
    pub course_code: String,
    pub course_name: String,
    pub exam_id: Uuid,
    pub exam_title: String,
    /// `null` for an exam that is not an MCQ paper.
    pub assessment_type: Option<AssessmentType>,
    pub attempt_no: i16,
    /// Counted from the attempt's own stored paper, not from
    /// `exams.question_count`: that is the *intended* size, and a historic
    /// attempt may have been served a different one.
    pub total_questions: i64,
    pub answered_questions: i64,
    pub correct_answers: i64,
    /// `null` until graded. Never `0` for an ungraded attempt — a zero is a real
    /// measured score, which is a different claim.
    pub score: Option<f64>,
    pub max_score: f64,
    /// `null` whenever `score` is.
    pub percentage: Option<f64>,
    /// `submitted_at - started_at` in whole seconds; `null` while in progress.
    pub time_spent_seconds: Option<i64>,
    pub status: ExamAttemptStatus,
    pub started_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    /// Topics of this attempt's incorrect answers. Empty while the attempt is
    /// unsubmitted: before grading, which questions are wrong is part of the
    /// answer key, and an admin list is not a place to leak it either.
    pub weak_topics: Vec<String>,
}

impl From<reports::AttemptReportRow> for AttemptReportResponse {
    fn from(r: reports::AttemptReportRow) -> Self {
        Self {
            attempt_id: r.attempt_id.into_uuid(),
            student_id: r.student_id.into_uuid(),
            student_name: r.student_name,
            roll_number: r.roll_number,
            program_id: r.program_id.into_uuid(),
            program_name: r.program_name,
            semester_number: r.semester_number,
            course_id: r.course_id.into_uuid(),
            course_code: r.course_code,
            course_name: r.course_name,
            exam_id: r.exam_id.into_uuid(),
            exam_title: r.exam_title,
            assessment_type: r.assessment_type,
            attempt_no: r.attempt_no,
            total_questions: r.total_questions,
            answered_questions: r.answered_questions,
            correct_answers: r.correct_answers,
            score: r.score,
            max_score: r.max_score,
            percentage: r.percentage,
            time_spent_seconds: r.time_spent_seconds,
            status: r.status,
            started_at: r.started_at,
            submitted_at: r.submitted_at,
            weak_topics: r.weak_topics,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListAttemptReportsQuery {
    #[serde(default)]
    pub program_id: Option<Uuid>,
    #[serde(default)]
    pub semester_id: Option<Uuid>,
    #[serde(default)]
    pub course_id: Option<Uuid>,
    #[serde(default)]
    pub student_id: Option<Uuid>,
    #[serde(default)]
    pub exam_id: Option<Uuid>,
    #[serde(default)]
    pub assessment_type: Option<AssessmentType>,
    #[serde(default)]
    pub status: Option<ExamAttemptStatus>,
    /// RFC3339, filtering `started_at`.
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    /// Case-insensitive substring over student name, roll number and exam title.
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

pub async fn list_student_reports(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<ListAttemptReportsQuery>,
) -> Result<(HeaderMap, Json<Vec<AttemptReportResponse>>), PublicError> {
    actor.require(Capability::ManagePrograms)?;

    let page = super::page(query.limit, query.offset, 50)?;
    let scope = report_scope(&actor)?;
    let q = super::search_term(query.q.as_deref());

    let filter = reports::AttemptReportFilter {
        program_id: query.program_id.map(ProgramId::from),
        semester_id: query.semester_id.map(SemesterId::from),
        course_id: query.course_id.map(CourseId::from),
        student_id: query.student_id.map(StudentId::from),
        exam_id: query.exam_id.map(ExamId::from),
        assessment_type: query.assessment_type,
        status: query.status,
        from: super::sessions::timestamp("from", query.from.as_deref())?,
        to: super::sessions::timestamp("to", query.to.as_deref())?,
        q,
        scope_program_ids: scope.as_deref(),
    };

    let total = reports::count_attempt_reports(&state.pool, &filter)
        .await
        .map_err(PublicError::from)?;
    let rows = reports::list_attempt_reports(&state.pool, &filter, page.limit, page.offset)
        .await
        .map_err(PublicError::from)?;

    Ok((
        super::total_count(total),
        Json(rows.into_iter().map(AttemptReportResponse::from).collect()),
    ))
}

// ---------------------------------------------------------------------------
// GET /admin/students/{student_id}/report
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct CourseRollupResponse {
    pub course_id: Uuid,
    pub course_code: String,
    pub course_name: String,
    pub attempts: i64,
    /// `null` when nothing in this course is graded yet.
    pub average_percentage: Option<f64>,
}

impl From<reports::StudentCourseRollup> for CourseRollupResponse {
    fn from(r: reports::StudentCourseRollup) -> Self {
        Self {
            course_id: r.course_id.into_uuid(),
            course_code: r.course_code,
            course_name: r.course_name,
            attempts: r.attempts,
            average_percentage: r.average_percentage,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct WeakTopicResponse {
    pub topic: String,
    /// How many times this student has answered a question on the topic
    /// incorrectly, across every graded attempt.
    pub missed_count: i64,
}

impl From<reports::WeakTopic> for WeakTopicResponse {
    fn from(w: reports::WeakTopic) -> Self {
        Self {
            topic: w.topic,
            missed_count: w.missed_count,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StudentReportResponse {
    pub student_id: Uuid,
    pub student_name: String,
    pub roll_number: String,
    pub program_name: String,
    pub semester_number: i16,
    pub lsc_code: Option<String>,
    pub attempts_total: i64,
    pub attempts_graded: i64,
    /// `null` with nothing to average, per the existing analytics rule: a `0`
    /// would read as a measured zero, which is a different claim.
    pub average_percentage: Option<f64>,
    pub best_percentage: Option<f64>,
    /// Summed derived durations; `0` for a student who has submitted nothing,
    /// which here is a real count rather than a missing measurement.
    pub total_time_spent_seconds: i64,
    pub by_course: Vec<CourseRollupResponse>,
    /// Descending by `missed_count` — the topics to revise first.
    pub weak_topics: Vec<WeakTopicResponse>,
}

pub async fn get_student_report(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(student_id): Path<Uuid>,
) -> Result<Json<StudentReportResponse>, PublicError> {
    actor.require(Capability::ManagePrograms)?;

    let student_id = StudentId::from(student_id);
    let scope = report_scope(&actor)?;

    // The scope predicate is inside the query, so a student in another
    // programme selects no row: "not in your scope" and "does not exist" are
    // the same `404`, and nothing here has to decide between them.
    let header = reports::report_header(&state.pool, student_id, scope.as_deref())
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    let by_course = reports::rollup_by_course(&state.pool, student_id)
        .await
        .map_err(PublicError::from)?;
    let weak_topics = reports::weak_topics(&state.pool, student_id, WEAK_TOPIC_LIMIT)
        .await
        .map_err(PublicError::from)?;

    Ok(Json(StudentReportResponse {
        student_id: header.student_id.into_uuid(),
        student_name: header.student_name,
        roll_number: header.roll_number,
        program_name: header.program_name,
        semester_number: header.semester_number,
        lsc_code: header.lsc_code,
        attempts_total: header.attempts_total,
        attempts_graded: header.attempts_graded,
        average_percentage: header.average_percentage,
        best_percentage: header.best_percentage,
        total_time_spent_seconds: header.total_time_spent_seconds,
        by_course: by_course
            .into_iter()
            .map(CourseRollupResponse::from)
            .collect(),
        weak_topics: weak_topics
            .into_iter()
            .map(WeakTopicResponse::from)
            .collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dg_core::{Actor, UserId};

    // -- RBAC matrix -------------------------------------------------------

    #[test]
    fn a_super_admin_may_read_every_student_report() {
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert!(actor.require(Capability::ManagePrograms).is_ok());
        assert_eq!(report_scope(&actor).expect("unscoped"), None);
    }

    #[test]
    fn a_sub_admin_reads_reports_only_for_its_own_programs() {
        let scoped = ProgramId::new();
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![scoped]);
        assert!(actor.require(Capability::ManagePrograms).is_ok());
        assert_eq!(
            report_scope(&actor).expect("scoped"),
            Some(vec![scoped.into_uuid()]),
            "the scope is passed into SQL, not applied to the result"
        );
    }

    /// The rule that matters most here: no scopes means no rows, not every row.
    #[test]
    fn a_sub_admin_with_no_scopes_gets_an_empty_report_list() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![]);
        assert_eq!(
            report_scope(&actor).expect("a sub-admin may list"),
            Some(Vec::new()),
            "an empty scope vector is the filter, never a reason to drop it"
        );
    }

    #[test]
    fn a_student_is_forbidden_both_report_routes() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![ProgramId::new()]);
        assert_eq!(
            actor
                .require(Capability::ManagePrograms)
                .expect_err("reports are admin-only")
                .code(),
            "FORBIDDEN"
        );
        assert_eq!(
            report_scope(&actor).expect_err("never reachable").code(),
            "FORBIDDEN"
        );
    }

    #[test]
    fn report_lists_reject_an_out_of_range_limit_rather_than_clamping() {
        assert_eq!(
            crate::admin::page(Some(201), None, 50)
                .expect_err("over the ceiling")
                .code(),
            "VALIDATION_ERROR"
        );
        assert_eq!(
            crate::admin::page(None, Some(-1), 50)
                .expect_err("negative offset")
                .code(),
            "VALIDATION_ERROR"
        );
    }

    #[test]
    fn a_malformed_from_timestamp_is_a_field_error() {
        let err =
            super::super::sessions::timestamp("from", Some("yesterday")).expect_err("not RFC3339");
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    // -- Response shape ----------------------------------------------------

    /// `score`/`percentage`/`time_spent_seconds` must be `null` — not `0` — for
    /// an attempt still in progress. A zero there is a measured claim, and the
    /// dashboard would render it as a student who scored nothing.
    #[test]
    fn an_in_progress_attempt_reports_nulls_not_zeroes() {
        let json = serde_json::to_value(AttemptReportResponse {
            attempt_id: Uuid::nil(),
            student_id: Uuid::nil(),
            student_name: "Asha".into(),
            roll_number: "BA-ML-0001".into(),
            program_id: Uuid::nil(),
            program_name: "BA Malayalam".into(),
            semester_number: 1,
            course_id: Uuid::nil(),
            course_code: "ML101".into(),
            course_name: "Malayalam Literature".into(),
            exam_id: Uuid::nil(),
            exam_title: "Unit 3 Test".into(),
            assessment_type: Some(AssessmentType::MidTermQuiz),
            attempt_no: 1,
            total_questions: 10,
            answered_questions: 4,
            correct_answers: 0,
            score: None,
            max_score: 10.0,
            percentage: None,
            time_spent_seconds: None,
            status: ExamAttemptStatus::InProgress,
            started_at: Utc::now(),
            submitted_at: None,
            weak_topics: Vec::new(),
        })
        .expect("serialises");

        assert!(json["score"].is_null());
        assert!(json["percentage"].is_null());
        assert!(
            json["time_spent_seconds"].is_null(),
            "a duration is only known once the attempt is submitted"
        );
        assert!(
            json["weak_topics"]
                .as_array()
                .is_some_and(|topics| topics.is_empty()),
            "weak topics before grading would leak which answers are wrong"
        );
    }

    #[test]
    fn a_graded_attempt_reports_numbers_and_a_derived_duration() {
        let started = Utc::now();
        let json = serde_json::to_value(AttemptReportResponse {
            attempt_id: Uuid::nil(),
            student_id: Uuid::nil(),
            student_name: "Asha".into(),
            roll_number: "BA-ML-0001".into(),
            program_id: Uuid::nil(),
            program_name: "BA Malayalam".into(),
            semester_number: 1,
            course_id: Uuid::nil(),
            course_code: "ML101".into(),
            course_name: "Malayalam Literature".into(),
            exam_id: Uuid::nil(),
            exam_title: "Unit 3 Test".into(),
            assessment_type: Some(AssessmentType::SemesterExam),
            attempt_no: 2,
            total_questions: 10,
            answered_questions: 10,
            correct_answers: 8,
            score: Some(8.0),
            max_score: 10.0,
            percentage: Some(80.0),
            time_spent_seconds: Some(412),
            status: ExamAttemptStatus::Graded,
            started_at: started,
            submitted_at: Some(started),
            weak_topics: vec!["Phonology".into()],
        })
        .expect("serialises");

        assert!(json["score"].is_number() && json["percentage"].is_number());
        assert_eq!(json["time_spent_seconds"], 412);
        // Scores stay numbers on the wire; "8/10" is the console's rendering.
        assert!(json["max_score"].is_number());
    }

    #[test]
    fn a_student_with_nothing_graded_reports_null_averages_and_empty_series() {
        let json = serde_json::to_value(StudentReportResponse {
            student_id: Uuid::nil(),
            student_name: "Asha".into(),
            roll_number: "BA-ML-0001".into(),
            program_name: "BA Malayalam".into(),
            semester_number: 1,
            lsc_code: None,
            attempts_total: 0,
            attempts_graded: 0,
            average_percentage: None,
            best_percentage: None,
            total_time_spent_seconds: 0,
            by_course: Vec::new(),
            weak_topics: Vec::new(),
        })
        .expect("serialises");

        assert!(json["average_percentage"].is_null());
        assert!(json["best_percentage"].is_null());
        // A count, unlike an average, is genuinely zero.
        assert_eq!(json["total_time_spent_seconds"], 0);
        assert!(json["by_course"].is_array());
        assert!(json["weak_topics"].is_array());
        assert!(json["lsc_code"].is_null());
    }

    #[test]
    fn a_weak_topic_carries_the_count_the_badge_orders_by() {
        let json = serde_json::to_value(WeakTopicResponse::from(reports::WeakTopic {
            topic: "Phonology".into(),
            missed_count: 7,
        }))
        .expect("serialises");
        assert_eq!(json["topic"], "Phonology");
        assert_eq!(json["missed_count"], 7);
    }
}
