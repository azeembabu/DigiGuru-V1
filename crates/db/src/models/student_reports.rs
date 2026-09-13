//! Admin "Student Reports" reads — the attempt-level report list and one
//! student's academic record (contract A2.4).
//!
//! Read-only, and deliberately built on nothing new: every figure is derived
//! from `exam_attempts`, `exam_attempt_answers` and `question_pool`. In
//! particular `time_spent_seconds` is `submitted_at - started_at` computed in
//! SQL — there is no stored duration column, because a second copy of a value
//! already implied by two timestamps is a second thing to keep true.
//!
//! Scoping works the way `/admin/analytics` does: the caller's role decides
//! *which query runs*, and a `sub_admin` is restricted to its
//! `sub_admin_scopes` programs through `exam_attempts -> students ->
//! program_id`. Both functions take `scope_program_ids`; `Some(&[])` — a
//! sub-admin with no scopes — correctly yields nothing, which is the right
//! answer and not a reason to fall back to the unscoped query.
//!
//! Averages and percentages are `Option`: `null` when there is nothing to
//! average, never `0`, which would read as a real measured zero.

use chrono::{DateTime, Utc};
use dg_core::{
    AssessmentType, CourseId, ExamAttemptStatus, ExamId, ProgramId, SemesterId, StudentId,
};
use sqlx::PgPool;

use crate::error::{Error, Result};

/// One row of `GET /admin/student-reports`, denormalised far enough to render
/// without a second request per row.
#[derive(Debug, Clone)]
pub struct AttemptReportRow {
    pub attempt_id: dg_core::ExamAttemptId,
    pub student_id: StudentId,
    pub student_name: String,
    pub roll_number: String,
    pub program_id: ProgramId,
    pub program_name: String,
    pub semester_number: i16,
    pub course_id: CourseId,
    pub course_code: String,
    pub course_name: String,
    pub exam_id: ExamId,
    pub exam_title: String,
    /// `None` for an exam that is not an MCQ paper.
    pub assessment_type: Option<AssessmentType>,
    pub attempt_no: i16,
    /// Questions on the paper, counted from the attempt's own stored rows — not
    /// from `exams.question_count`, which is the *intended* size and can differ
    /// from what a historic attempt was actually served.
    pub total_questions: i64,
    pub answered_questions: i64,
    pub correct_answers: i64,
    /// `None` until graded.
    pub score: Option<f64>,
    pub max_score: f64,
    /// `100 * score / max_score`, `None` until graded.
    pub percentage: Option<f64>,
    /// `submitted_at - started_at` in whole seconds; `None` while in progress.
    pub time_spent_seconds: Option<i64>,
    pub status: ExamAttemptStatus,
    pub started_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    /// Topics of this attempt's incorrect answers, alphabetical. Empty while the
    /// attempt is unsubmitted: before grading, which questions are wrong is part
    /// of the answer key.
    pub weak_topics: Vec<String>,
}

/// Filters for [`list_attempt_reports`]. `None` means "no filter", so one struct
/// serves the unfiltered list and every drill-down the dashboard links into.
#[derive(Debug, Clone, Copy, Default)]
pub struct AttemptReportFilter<'a> {
    pub program_id: Option<ProgramId>,
    pub semester_id: Option<SemesterId>,
    pub course_id: Option<CourseId>,
    pub student_id: Option<StudentId>,
    pub exam_id: Option<ExamId>,
    pub assessment_type: Option<AssessmentType>,
    pub status: Option<ExamAttemptStatus>,
    /// Inclusive lower/upper bounds on `started_at`.
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    /// Case-insensitive substring over student name, roll number and exam title.
    pub q: Option<&'a str>,
    /// Sub-admin scope, applied in SQL before pagination so the page and
    /// `X-Total-Count` agree. `Some(&[])` matches nothing.
    pub scope_program_ids: Option<&'a [uuid::Uuid]>,
}

/// One page of attempt reports, newest first.
///
/// The per-attempt tallies are correlated aggregates rather than a `GROUP BY`
/// over a joined `exam_attempt_answers`: grouping would multiply the attempt row
/// by its answers and make every other aggregate in the select list wrong.
pub async fn list_attempt_reports(
    pool: &PgPool,
    filter: &AttemptReportFilter<'_>,
    limit: i64,
    offset: i64,
) -> Result<Vec<AttemptReportRow>> {
    sqlx::query_as!(
        AttemptReportRow,
        r#"
        SELECT
            ea.id           as "attempt_id!: dg_core::ExamAttemptId",
            ea.student_id   as "student_id!: StudentId",
            st.full_name    as "student_name!",
            st.roll_number  as "roll_number!",
            c.program_id    as "program_id!: ProgramId",
            p.name          as "program_name!",
            sm.semester_number as "semester_number!",
            c.id            as "course_id!: CourseId",
            c.code          as "course_code!",
            c.name          as "course_name!",
            e.id            as "exam_id!: ExamId",
            e.title         as "exam_title!",
            e.assessment_type as "assessment_type: AssessmentType",
            ea.attempt_no   as "attempt_no!",
            (SELECT count(*) FROM exam_attempt_answers a
              WHERE a.attempt_id = ea.id)                  as "total_questions!",
            (SELECT count(*) FROM exam_attempt_answers a
              WHERE a.attempt_id = ea.id
                AND a.selected_option_index IS NOT NULL)   as "answered_questions!",
            (SELECT count(*) FROM exam_attempt_answers a
              WHERE a.attempt_id = ea.id AND a.is_correct) as "correct_answers!",
            ea.score        as "score",
            ea.max_score    as "max_score!",
            -- NULL in, NULL out: an ungraded attempt has no percentage, and a
            -- zero here would assert a measured score of nothing.
            (100.0 * ea.score / ea.max_score)              as "percentage",
            -- Derived, never stored.
            EXTRACT(EPOCH FROM (ea.submitted_at - ea.started_at))::bigint
                                                           as "time_spent_seconds",
            ea.status       as "status!: ExamAttemptStatus",
            ea.started_at   as "started_at!",
            ea.submitted_at as "submitted_at",
            COALESCE(
              (SELECT array_agg(DISTINCT q.topic ORDER BY q.topic)
                 FROM exam_attempt_answers a
                 JOIN question_pool q ON q.id = a.question_id
                WHERE a.attempt_id = ea.id AND a.is_correct IS FALSE),
              ARRAY[]::text[]
            )                                              as "weak_topics!"
        FROM exam_attempts ea
        JOIN students  st ON st.id = ea.student_id
        JOIN exams     e  ON e.id  = ea.exam_id
        JOIN blocks    b  ON b.id  = e.block_id
        JOIN courses   c  ON c.id  = b.course_id
        JOIN semesters sm ON sm.id = c.semester_id
        JOIN programs  p  ON p.id  = c.program_id
        WHERE ($1::uuid IS NULL OR c.program_id = $1)
          AND ($2::uuid IS NULL OR c.semester_id = $2)
          AND ($3::uuid IS NULL OR c.id = $3)
          AND ($4::uuid IS NULL OR ea.student_id = $4)
          AND ($5::uuid IS NULL OR ea.exam_id = $5)
          AND ($6::text IS NULL OR e.assessment_type = $6::assessment_type)
          AND ($7::text IS NULL OR ea.status = $7::exam_attempt_status)
          AND ($8::timestamptz IS NULL OR ea.started_at >= $8)
          AND ($9::timestamptz IS NULL OR ea.started_at <= $9)
          AND ($10::text IS NULL OR st.full_name ILIKE '%' || $10 || '%'
                                 OR st.roll_number ILIKE '%' || $10 || '%'
                                 OR e.title ILIKE '%' || $10 || '%')
          -- A sub-admin's scope follows the student's own program, matching
          -- `/admin/analytics`; `= ANY('{}')` is false for every row, so a
          -- sub-admin with no scopes sees nothing.
          AND ($11::uuid[] IS NULL OR st.program_id = ANY($11))
        ORDER BY ea.started_at DESC, ea.attempt_no DESC, ea.id DESC
        LIMIT $12 OFFSET $13
        "#,
        filter.program_id.map(ProgramId::into_uuid),
        filter.semester_id.map(SemesterId::into_uuid),
        filter.course_id.map(CourseId::into_uuid),
        filter.student_id.map(StudentId::into_uuid),
        filter.exam_id.map(ExamId::into_uuid),
        filter.assessment_type.map(AssessmentType::as_db_str),
        filter.status.map(ExamAttemptStatus::as_db_str),
        filter.from,
        filter.to,
        filter.q,
        filter.scope_program_ids,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// `X-Total-Count` for [`list_attempt_reports`] — the total *before* pagination,
/// so it repeats every filter and takes no `LIMIT`.
pub async fn count_attempt_reports(pool: &PgPool, filter: &AttemptReportFilter<'_>) -> Result<i64> {
    sqlx::query_scalar!(
        r#"
        SELECT count(*) as "count!"
        FROM exam_attempts ea
        JOIN students  st ON st.id = ea.student_id
        JOIN exams     e  ON e.id  = ea.exam_id
        JOIN blocks    b  ON b.id  = e.block_id
        JOIN courses   c  ON c.id  = b.course_id
        WHERE ($1::uuid IS NULL OR c.program_id = $1)
          AND ($2::uuid IS NULL OR c.semester_id = $2)
          AND ($3::uuid IS NULL OR c.id = $3)
          AND ($4::uuid IS NULL OR ea.student_id = $4)
          AND ($5::uuid IS NULL OR ea.exam_id = $5)
          AND ($6::text IS NULL OR e.assessment_type = $6::assessment_type)
          AND ($7::text IS NULL OR ea.status = $7::exam_attempt_status)
          AND ($8::timestamptz IS NULL OR ea.started_at >= $8)
          AND ($9::timestamptz IS NULL OR ea.started_at <= $9)
          AND ($10::text IS NULL OR st.full_name ILIKE '%' || $10 || '%'
                                 OR st.roll_number ILIKE '%' || $10 || '%'
                                 OR e.title ILIKE '%' || $10 || '%')
          AND ($11::uuid[] IS NULL OR st.program_id = ANY($11))
        "#,
        filter.program_id.map(ProgramId::into_uuid),
        filter.semester_id.map(SemesterId::into_uuid),
        filter.course_id.map(CourseId::into_uuid),
        filter.student_id.map(StudentId::into_uuid),
        filter.exam_id.map(ExamId::into_uuid),
        filter.assessment_type.map(AssessmentType::as_db_str),
        filter.status.map(ExamAttemptStatus::as_db_str),
        filter.from,
        filter.to,
        filter.q,
        filter.scope_program_ids
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// The header of `GET /admin/students/{id}/report`: who the student is and their
/// totals across every attempt.
#[derive(Debug, Clone)]
pub struct StudentReportHeader {
    pub student_id: StudentId,
    pub student_name: String,
    pub roll_number: String,
    pub program_name: String,
    pub semester_number: i16,
    pub lsc_code: Option<String>,
    pub attempts_total: i64,
    pub attempts_graded: i64,
    /// `None` when nothing is graded yet.
    pub average_percentage: Option<f64>,
    pub best_percentage: Option<f64>,
    /// Summed `submitted_at - started_at`; `0` when nothing has been submitted,
    /// which here is a real measured total rather than a missing one.
    pub total_time_spent_seconds: i64,
}

/// One student's report header, **only** if they are inside the caller's scope.
///
/// The scope predicate is part of the query, so a student outside a sub-admin's
/// programs returns `None` and the handler answers `404` — never `403`, which
/// would confirm the id exists.
pub async fn report_header(
    pool: &PgPool,
    student_id: StudentId,
    scope_program_ids: Option<&[uuid::Uuid]>,
) -> Result<Option<StudentReportHeader>> {
    sqlx::query_as!(
        StudentReportHeader,
        r#"
        SELECT
            st.id          as "student_id!: StudentId",
            st.full_name   as "student_name!",
            st.roll_number as "roll_number!",
            p.name         as "program_name!",
            sm.semester_number as "semester_number!",
            l.code         as "lsc_code",
            (SELECT count(*) FROM exam_attempts ea
              WHERE ea.student_id = st.id)                       as "attempts_total!",
            (SELECT count(*) FROM exam_attempts ea
              WHERE ea.student_id = st.id AND ea.status = 'graded') as "attempts_graded!",
            (SELECT avg(100.0 * ea.score / ea.max_score) FROM exam_attempts ea
              WHERE ea.student_id = st.id AND ea.score IS NOT NULL) as "average_percentage",
            (SELECT max(100.0 * ea.score / ea.max_score) FROM exam_attempts ea
              WHERE ea.student_id = st.id AND ea.score IS NOT NULL) as "best_percentage",
            COALESCE((
              SELECT sum(EXTRACT(EPOCH FROM (ea.submitted_at - ea.started_at)))::bigint
              FROM exam_attempts ea
              WHERE ea.student_id = st.id AND ea.submitted_at IS NOT NULL
            ), 0)                                                as "total_time_spent_seconds!"
        FROM students st
        JOIN programs  p  ON p.id  = st.program_id
        JOIN semesters sm ON sm.id = st.semester_id
        LEFT JOIN lscs l  ON l.id  = st.lsc_id
        WHERE st.id = $1
          AND ($2::uuid[] IS NULL OR st.program_id = ANY($2))
        "#,
        student_id.into_uuid(),
        scope_program_ids
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// One course's line in a student's report.
#[derive(Debug, Clone)]
pub struct StudentCourseRollup {
    pub course_id: CourseId,
    pub course_code: String,
    pub course_name: String,
    pub attempts: i64,
    /// `None` when the student has no graded attempt in this course.
    pub average_percentage: Option<f64>,
}

/// Per-course rollup for one student, ordered by course code so the table reads
/// the same on every refresh.
///
/// Grouped from `exam_attempts` rather than from `student_courses`: this is a
/// record of what the student has *sat*, and a course with no attempt has no
/// line here.
pub async fn rollup_by_course(
    pool: &PgPool,
    student_id: StudentId,
) -> Result<Vec<StudentCourseRollup>> {
    sqlx::query_as!(
        StudentCourseRollup,
        r#"
        SELECT
            c.id   as "course_id!: CourseId",
            c.code as "course_code!",
            c.name as "course_name!",
            count(*) as "attempts!",
            avg(100.0 * ea.score / ea.max_score) as "average_percentage"
        FROM exam_attempts ea
        JOIN exams   e ON e.id = ea.exam_id
        JOIN blocks  b ON b.id = e.block_id
        JOIN courses c ON c.id = b.course_id
        WHERE ea.student_id = $1
        GROUP BY c.id, c.code, c.name
        ORDER BY c.code ASC
        "#,
        student_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// A topic the student keeps getting wrong, and how often.
#[derive(Debug, Clone)]
pub struct WeakTopic {
    pub topic: String,
    pub missed_count: i64,
}

/// Weak topics aggregated across every submitted attempt, most-missed first.
///
/// `is_correct IS FALSE`, not `IS NOT TRUE`: a NULL `is_correct` belongs to an
/// attempt that was never graded, and counting those as misses would invent
/// weakness from an abandoned paper. `topic` is the tie-break so equal counts
/// order stably rather than shuffling between refreshes.
pub async fn weak_topics(
    pool: &PgPool,
    student_id: StudentId,
    limit: i64,
) -> Result<Vec<WeakTopic>> {
    sqlx::query_as!(
        WeakTopic,
        r#"
        SELECT q.topic as "topic!", count(*) as "missed_count!"
        FROM exam_attempt_answers a
        JOIN exam_attempts ea ON ea.id = a.attempt_id
        JOIN question_pool  q ON q.id = a.question_id
        WHERE ea.student_id = $1 AND a.is_correct IS FALSE
        GROUP BY q.topic
        ORDER BY count(*) DESC, q.topic ASC
        LIMIT $2
        "#,
        student_id.into_uuid(),
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}
