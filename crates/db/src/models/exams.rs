//! `exams` and `exam_attempts` — block-level assessments and a student's
//! record of sitting them (`migrations/0005_exams.sql`).
//!
//! An exam has no `program_id` of its own; scope is resolved by walking
//! `exams -> blocks -> courses -> program_id`, one level deeper than
//! [`crate::models::blocks`] does it. [`program_id_for_exam`] is that walk as
//! a single round trip, so an admin handler can make its capability check
//! before it fetches anything.
//!
//! Two attempt row shapes exist deliberately. [`ExamAttemptRow`] is the admin
//! list for one exam and denormalises the *student*; [`StudentAttemptRow`] is
//! the student's own dashboard card and denormalises the *exam and its
//! ancestry* instead. Neither is a superset of the other, and a student row
//! must never carry another student's name — keeping them apart is what makes
//! that a type-level property rather than a review comment.
//!
//! Every student-facing query takes the `StudentId` as a mandatory `WHERE`
//! predicate, not as a filter the caller may omit: there is no function in
//! this module that can return one student's attempts to another.

use chrono::{DateTime, Utc};
use dg_core::{
    BlockId, CourseId, ExamAttemptId, ExamAttemptStatus, ExamId, ExamStatus, ProgramId, SemesterId,
    StudentId, UserId,
};
use sqlx::PgPool;

use crate::error::{Error, Result};

/// An exam, carrying the flat ancestry of its block for the same reason
/// [`crate::models::blocks::Block`] does: a console rendering a breadcrumb
/// should not need a request per level to name the exam's own parents.
#[derive(Debug, Clone)]
pub struct Exam {
    pub id: ExamId,
    pub block_id: BlockId,
    pub block_no: i16,
    pub block_title: String,
    pub course_id: CourseId,
    pub course_code: String,
    pub semester_id: SemesterId,
    pub program_id: ProgramId,
    pub title: String,
    pub description: Option<String>,
    pub max_score: f64,
    /// `None` = untimed.
    pub duration_minutes: Option<i16>,
    pub status: ExamStatus,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    pool: &PgPool,
    block_id: BlockId,
    title: &str,
    description: Option<&str>,
    max_score: f64,
    duration_minutes: Option<i16>,
    status: ExamStatus,
    created_by: UserId,
) -> Result<Exam> {
    sqlx::query_as!(
        Exam,
        r#"
        WITH inserted AS (
            INSERT INTO exams (block_id, title, description, max_score, duration_minutes,
                               status, created_by)
            VALUES ($1, $2, $3, $4, $5, $6::text::exam_status, $7)
            RETURNING id, block_id, title, description, max_score, duration_minutes,
                      status, created_by, created_at
        )
        SELECT
            e.id           as "id!: ExamId",
            e.block_id     as "block_id!: BlockId",
            b.block_no     as "block_no!",
            b.title        as "block_title!",
            b.course_id    as "course_id!: CourseId",
            c.code         as "course_code!",
            c.semester_id  as "semester_id!: SemesterId",
            c.program_id   as "program_id!: ProgramId",
            e.title        as "title!",
            e.description  as "description",
            e.max_score    as "max_score!",
            e.duration_minutes as "duration_minutes",
            e.status       as "status!: ExamStatus",
            e.created_by   as "created_by!: UserId",
            e.created_at   as "created_at!"
        FROM inserted e
        JOIN blocks  b ON b.id = e.block_id
        JOIN courses c ON c.id = b.course_id
        "#,
        block_id.into_uuid(),
        title,
        description,
        max_score,
        duration_minutes,
        status.as_db_str(),
        created_by.into_uuid()
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn find_by_id(pool: &PgPool, id: ExamId) -> Result<Option<Exam>> {
    sqlx::query_as!(
        Exam,
        r#"
        SELECT
            e.id           as "id!: ExamId",
            e.block_id     as "block_id!: BlockId",
            b.block_no     as "block_no!",
            b.title        as "block_title!",
            b.course_id    as "course_id!: CourseId",
            c.code         as "course_code!",
            c.semester_id  as "semester_id!: SemesterId",
            c.program_id   as "program_id!: ProgramId",
            e.title        as "title!",
            e.description  as "description",
            e.max_score    as "max_score!",
            e.duration_minutes as "duration_minutes",
            e.status       as "status!: ExamStatus",
            e.created_by   as "created_by!: UserId",
            e.created_at   as "created_at!"
        FROM exams e
        JOIN blocks  b ON b.id = e.block_id
        JOIN courses c ON c.id = b.course_id
        WHERE e.id = $1
        "#,
        id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// One page of a block's exams, ordered by title so the list is stable across
/// pages (`created_at` alone ties for two exams seeded in one transaction).
pub async fn list_by_block(
    pool: &PgPool,
    block_id: BlockId,
    q: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Exam>> {
    sqlx::query_as!(
        Exam,
        r#"
        SELECT
            e.id           as "id!: ExamId",
            e.block_id     as "block_id!: BlockId",
            b.block_no     as "block_no!",
            b.title        as "block_title!",
            b.course_id    as "course_id!: CourseId",
            c.code         as "course_code!",
            c.semester_id  as "semester_id!: SemesterId",
            c.program_id   as "program_id!: ProgramId",
            e.title        as "title!",
            e.description  as "description",
            e.max_score    as "max_score!",
            e.duration_minutes as "duration_minutes",
            e.status       as "status!: ExamStatus",
            e.created_by   as "created_by!: UserId",
            e.created_at   as "created_at!"
        FROM exams e
        JOIN blocks  b ON b.id = e.block_id
        JOIN courses c ON c.id = b.course_id
        WHERE e.block_id = $1
          AND ($2::text IS NULL OR e.title ILIKE '%' || $2 || '%')
        ORDER BY e.title ASC, e.id ASC
        LIMIT $3 OFFSET $4
        "#,
        block_id.into_uuid(),
        q,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// `X-Total-Count` for [`list_by_block`] — the total *before* pagination, so
/// it must apply the same `q` filter and no `LIMIT`.
pub async fn count_by_block(pool: &PgPool, block_id: BlockId, q: Option<&str>) -> Result<i64> {
    sqlx::query_scalar!(
        r#"
        SELECT count(*) as "count!"
        FROM exams e
        WHERE e.block_id = $1
          AND ($2::text IS NULL OR e.title ILIKE '%' || $2 || '%')
        "#,
        block_id.into_uuid(),
        q
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// The `program_id` that owns `exam_id`, via `exams -> blocks -> courses`.
/// `None` if the exam does not exist.
///
/// Mirrors [`crate::models::blocks::program_id_for_block`], and exists for
/// the same reason: the admin handler resolves the owning program *before*
/// its capability check, so a sub-admin outside the scope cannot tell an
/// existing exam from a missing one.
pub async fn program_id_for_exam(pool: &PgPool, exam_id: ExamId) -> Result<Option<ProgramId>> {
    let rec = sqlx::query!(
        r#"
        SELECT c.program_id as "program_id: ProgramId"
        FROM exams e
        JOIN blocks  b ON b.id = e.block_id
        JOIN courses c ON c.id = b.course_id
        WHERE e.id = $1
        "#,
        exam_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)?;

    Ok(rec.map(|r| r.program_id))
}

/// One row of the admin list `GET /admin/exams/{exam_id}/attempts`,
/// denormalised across the student who sat it.
#[derive(Debug, Clone)]
pub struct ExamAttemptRow {
    pub id: ExamAttemptId,
    pub exam_id: ExamId,
    pub student_id: StudentId,
    pub student_name: String,
    pub roll_number: String,
    pub attempt_no: i16,
    /// `None` until the attempt is marked.
    pub score: Option<f64>,
    pub max_score: f64,
    pub status: ExamAttemptStatus,
    pub started_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
}

/// One page of the attempts at a single exam.
///
/// Ordered newest-first with `attempt_no` and `id` as tie-breaks, so the
/// ordering is total: two attempts sharing a timestamp still have one
/// deterministic position each, which is what makes paging over this list
/// safe.
pub async fn attempts_for_exam(
    pool: &PgPool,
    exam_id: ExamId,
    limit: i64,
    offset: i64,
) -> Result<Vec<ExamAttemptRow>> {
    sqlx::query_as!(
        ExamAttemptRow,
        r#"
        SELECT
            ea.id           as "id!: ExamAttemptId",
            ea.exam_id      as "exam_id!: ExamId",
            ea.student_id   as "student_id!: StudentId",
            s.full_name     as "student_name!",
            s.roll_number   as "roll_number!",
            ea.attempt_no   as "attempt_no!",
            ea.score        as "score",
            ea.max_score    as "max_score!",
            ea.status       as "status!: ExamAttemptStatus",
            ea.started_at   as "started_at!",
            ea.submitted_at as "submitted_at"
        FROM exam_attempts ea
        JOIN students s ON s.id = ea.student_id
        WHERE ea.exam_id = $1
        ORDER BY ea.submitted_at DESC NULLS LAST, ea.started_at DESC,
                 ea.attempt_no DESC, ea.id DESC
        LIMIT $2 OFFSET $3
        "#,
        exam_id.into_uuid(),
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// `X-Total-Count` for [`attempts_for_exam`].
pub async fn count_attempts_for_exam(pool: &PgPool, exam_id: ExamId) -> Result<i64> {
    sqlx::query_scalar!(
        r#"SELECT count(*) as "count!" FROM exam_attempts WHERE exam_id = $1"#,
        exam_id.into_uuid()
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// One row of the student's own dashboard list — everything a score card
/// renders, denormalised by the join the query already makes, so a page of
/// cards is one request rather than one request per card.
#[derive(Debug, Clone)]
pub struct StudentAttemptRow {
    pub id: ExamAttemptId,
    pub exam_id: ExamId,
    pub exam_title: String,
    pub block_id: BlockId,
    pub block_no: i16,
    pub block_title: String,
    pub course_id: CourseId,
    pub course_code: String,
    pub course_name: String,
    pub attempt_no: i16,
    pub score: Option<f64>,
    pub max_score: f64,
    pub status: ExamAttemptStatus,
    pub started_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
}

/// One page of *this* student's attempts, newest first.
///
/// `student_id` is a mandatory argument bound into the `WHERE` clause, never
/// an optional filter: there is deliberately no way to call this without
/// naming exactly one student, and the caller sources that id from the
/// authenticated token (`.claude/rules/security.md` — self-only).
pub async fn attempts_for_student(
    pool: &PgPool,
    student_id: StudentId,
    limit: i64,
    offset: i64,
) -> Result<Vec<StudentAttemptRow>> {
    sqlx::query_as!(
        StudentAttemptRow,
        r#"
        SELECT
            ea.id           as "id!: ExamAttemptId",
            ea.exam_id      as "exam_id!: ExamId",
            e.title         as "exam_title!",
            e.block_id      as "block_id!: BlockId",
            b.block_no      as "block_no!",
            b.title         as "block_title!",
            b.course_id     as "course_id!: CourseId",
            c.code          as "course_code!",
            c.name          as "course_name!",
            ea.attempt_no   as "attempt_no!",
            ea.score        as "score",
            ea.max_score    as "max_score!",
            ea.status       as "status!: ExamAttemptStatus",
            ea.started_at   as "started_at!",
            ea.submitted_at as "submitted_at"
        FROM exam_attempts ea
        JOIN exams   e ON e.id = ea.exam_id
        JOIN blocks  b ON b.id = e.block_id
        JOIN courses c ON c.id = b.course_id
        WHERE ea.student_id = $1
        ORDER BY ea.submitted_at DESC NULLS LAST, ea.started_at DESC,
                 ea.attempt_no DESC, ea.id DESC
        LIMIT $2 OFFSET $3
        "#,
        student_id.into_uuid(),
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// `X-Total-Count` for [`attempts_for_student`].
pub async fn count_attempts_for_student(pool: &PgPool, student_id: StudentId) -> Result<i64> {
    sqlx::query_scalar!(
        r#"SELECT count(*) as "count!" FROM exam_attempts WHERE student_id = $1"#,
        student_id.into_uuid()
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// A single attempt, **only** if it belongs to `student_id`.
///
/// The ownership predicate is part of the query rather than a check the
/// handler performs on the row afterwards. That is the whole security
/// property of this function: another student's attempt id simply selects no
/// row, so the caller cannot accidentally return data it then decides to
/// reject, and the answer for "not yours" is indistinguishable from "does not
/// exist".
pub async fn attempt_for_student(
    pool: &PgPool,
    student_id: StudentId,
    attempt_id: ExamAttemptId,
) -> Result<Option<StudentAttemptRow>> {
    sqlx::query_as!(
        StudentAttemptRow,
        r#"
        SELECT
            ea.id           as "id!: ExamAttemptId",
            ea.exam_id      as "exam_id!: ExamId",
            e.title         as "exam_title!",
            e.block_id      as "block_id!: BlockId",
            b.block_no      as "block_no!",
            b.title         as "block_title!",
            b.course_id     as "course_id!: CourseId",
            c.code          as "course_code!",
            c.name          as "course_name!",
            ea.attempt_no   as "attempt_no!",
            ea.score        as "score",
            ea.max_score    as "max_score!",
            ea.status       as "status!: ExamAttemptStatus",
            ea.started_at   as "started_at!",
            ea.submitted_at as "submitted_at"
        FROM exam_attempts ea
        JOIN exams   e ON e.id = ea.exam_id
        JOIN blocks  b ON b.id = e.block_id
        JOIN courses c ON c.id = b.course_id
        WHERE ea.id = $2 AND ea.student_id = $1
        "#,
        student_id.into_uuid(),
        attempt_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}
