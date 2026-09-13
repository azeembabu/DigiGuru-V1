//! Drill-down lists behind the dashboard's enrolment and safety tiles —
//! `GET /api/v1/admin/enrollments` and `GET /api/v1/admin/safety-incidents`.
//!
//! Its sibling [`crate::models::drilldown_sessions`] carries the session and
//! board-event lists; the two were split only to keep each module under the
//! file-length guidance in `.claude/rules/code-style.md`.
//!
//! # One SQL body per list, two public doors
//!
//! Same construction as [`crate::models::analytics`], for the same reason: the
//! scoped and the unscoped variant of a list must never be two hand-written
//! `SELECT`s, because the moment one is edited the two roles silently disagree
//! about what the same list means. So each query body is written once, in a
//! private function taking `(pool, ids, unscoped, filters, ...)`, and its scope
//! predicate is `($2 OR <col> = ANY($1))` — a constant `true` when `$2` is set,
//! otherwise membership of the caller's programs.
//!
//! The thin `_all` / `_scoped` wrappers still exist so the **caller's role**
//! picks the function. A role reduced to a boolean threaded through one public
//! entry point is far easier to default wrong than a missing match arm is.
//!
//! An empty scope slice is a real answer — an empty array — never a reason to
//! fall back to the platform-wide query.
//!
//! Scoping walks `student_courses -> courses -> program_id` for enrolments and
//! `safety_incidents -> students -> program_id` for incidents, exactly as
//! `analytics` does.
//!
//! Every row is denormalised (student name, roll number, course code, …) by
//! the join the query already makes: a drill-down whose rows each need a second
//! request to render is an N+1 moved into the browser.

use chrono::{DateTime, Utc};
use dg_core::{CourseId, EnrollmentStatus, ProgramId, SessionId, StudentId};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Error, Result};

/// One row of `GET /admin/enrollments`.
#[derive(Debug, Clone)]
pub struct EnrollmentRow {
    pub id: Uuid,
    pub student_id: StudentId,
    pub student_name: String,
    pub roll_number: String,
    pub course_id: CourseId,
    pub course_code: String,
    pub course_name: String,
    pub semester_number: i16,
    pub status: EnrollmentStatus,
    pub assigned_at: DateTime<Utc>,
}

/// The `GET /admin/enrollments` filters, already normalised by the handler.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnrollmentFilters<'a> {
    /// Case-insensitive substring over student name, roll number, course code
    /// and course name.
    pub q: Option<&'a str>,
    pub status: Option<EnrollmentStatus>,
    pub course_id: Option<Uuid>,
    pub student_id: Option<Uuid>,
}

/// Platform-wide enrolments. Super-admin only — [`enrollments_scoped`] is the
/// sub-admin path.
pub async fn enrollments_all(
    pool: &PgPool,
    f: EnrollmentFilters<'_>,
    limit: i64,
    offset: i64,
) -> Result<Vec<EnrollmentRow>> {
    enrollments(pool, &[], true, f, limit, offset).await
}

/// The same list, restricted to `program_ids`. An empty slice yields an empty
/// page, which is the correct answer for a sub-admin with no scopes.
pub async fn enrollments_scoped(
    pool: &PgPool,
    program_ids: &[ProgramId],
    f: EnrollmentFilters<'_>,
    limit: i64,
    offset: i64,
) -> Result<Vec<EnrollmentRow>> {
    enrollments(pool, &raw(program_ids), false, f, limit, offset).await
}

/// `X-Total-Count` for [`enrollments_all`] — the total before pagination.
pub async fn count_enrollments_all(pool: &PgPool, f: EnrollmentFilters<'_>) -> Result<i64> {
    count_enrollments(pool, &[], true, f).await
}

/// `X-Total-Count` for [`enrollments_scoped`].
pub async fn count_enrollments_scoped(
    pool: &PgPool,
    program_ids: &[ProgramId],
    f: EnrollmentFilters<'_>,
) -> Result<i64> {
    count_enrollments(pool, &raw(program_ids), false, f).await
}

async fn enrollments(
    pool: &PgPool,
    ids: &[Uuid],
    unscoped: bool,
    f: EnrollmentFilters<'_>,
    limit: i64,
    offset: i64,
) -> Result<Vec<EnrollmentRow>> {
    sqlx::query_as!(
        EnrollmentRow,
        r#"
        SELECT
            sc.id                as "id!",
            sc.student_id        as "student_id!: StudentId",
            st.full_name         as "student_name!",
            st.roll_number       as "roll_number!",
            sc.course_id         as "course_id!: CourseId",
            c.code               as "course_code!",
            c.name               as "course_name!",
            sem.semester_number  as "semester_number!",
            sc.status            as "status!: EnrollmentStatus",
            sc.assigned_at       as "assigned_at!"
        FROM student_courses sc
        JOIN students  st  ON st.id  = sc.student_id
        JOIN courses   c   ON c.id   = sc.course_id
        JOIN semesters sem ON sem.id = c.semester_id
        WHERE ($2 OR c.program_id = ANY($1))
          AND ($3::text IS NULL OR sc.status = $3::text::enrollment_status)
          AND ($4::uuid IS NULL OR sc.course_id  = $4)
          AND ($5::uuid IS NULL OR sc.student_id = $5)
          AND ($6::text IS NULL
               OR st.full_name   ILIKE '%' || $6 || '%'
               OR st.roll_number ILIKE '%' || $6 || '%'
               OR c.code         ILIKE '%' || $6 || '%'
               OR c.name         ILIKE '%' || $6 || '%')
        ORDER BY sc.assigned_at DESC, sc.id
        LIMIT $7 OFFSET $8
        "#,
        ids,
        unscoped,
        f.status.map(EnrollmentStatus::as_db_str),
        f.course_id,
        f.student_id,
        f.q,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

async fn count_enrollments(
    pool: &PgPool,
    ids: &[Uuid],
    unscoped: bool,
    f: EnrollmentFilters<'_>,
) -> Result<i64> {
    sqlx::query_scalar!(
        r#"
        SELECT count(*) as "count!"
        FROM student_courses sc
        JOIN students st ON st.id = sc.student_id
        JOIN courses  c  ON c.id  = sc.course_id
        WHERE ($2 OR c.program_id = ANY($1))
          AND ($3::text IS NULL OR sc.status = $3::text::enrollment_status)
          AND ($4::uuid IS NULL OR sc.course_id  = $4)
          AND ($5::uuid IS NULL OR sc.student_id = $5)
          AND ($6::text IS NULL
               OR st.full_name   ILIKE '%' || $6 || '%'
               OR st.roll_number ILIKE '%' || $6 || '%'
               OR c.code         ILIKE '%' || $6 || '%'
               OR c.name         ILIKE '%' || $6 || '%')
        "#,
        ids,
        unscoped,
        f.status.map(EnrollmentStatus::as_db_str),
        f.course_id,
        f.student_id,
        f.q
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// One row of `GET /admin/safety-incidents`.
///
/// `excerpt` is returned exactly as stored. It is redacted at write time per
/// `.claude/rules/security.md`; this read path neither re-redacts nor
/// un-redacts it, so an incident always reads the same wherever it is shown.
#[derive(Debug, Clone)]
pub struct SafetyIncidentRow {
    pub id: Uuid,
    pub session_id: Option<SessionId>,
    pub student_id: StudentId,
    pub student_name: String,
    pub roll_number: String,
    /// `jailbreak|toxicity|out_of_scope`
    pub kind: String,
    /// 0 = pre-LLM tap, 1 = transcript, 2 = output.
    pub tier: i16,
    pub excerpt: String,
    pub created_at: DateTime<Utc>,
}

/// The `GET /admin/safety-incidents` filters. `from`/`to` bound `created_at`
/// inclusively and are already parsed from RFC3339 by the handler.
#[derive(Debug, Clone, Copy, Default)]
pub struct SafetyIncidentFilters<'a> {
    pub tier: Option<i16>,
    pub kind: Option<&'a str>,
    pub student_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

/// Platform-wide incidents. Super-admin only.
pub async fn incidents_all(
    pool: &PgPool,
    f: SafetyIncidentFilters<'_>,
    limit: i64,
    offset: i64,
) -> Result<Vec<SafetyIncidentRow>> {
    incidents(pool, &[], true, f, limit, offset).await
}

/// The same list, restricted to the students of `program_ids`.
pub async fn incidents_scoped(
    pool: &PgPool,
    program_ids: &[ProgramId],
    f: SafetyIncidentFilters<'_>,
    limit: i64,
    offset: i64,
) -> Result<Vec<SafetyIncidentRow>> {
    incidents(pool, &raw(program_ids), false, f, limit, offset).await
}

/// `X-Total-Count` for [`incidents_all`].
pub async fn count_incidents_all(pool: &PgPool, f: SafetyIncidentFilters<'_>) -> Result<i64> {
    count_incidents(pool, &[], true, f).await
}

/// `X-Total-Count` for [`incidents_scoped`].
pub async fn count_incidents_scoped(
    pool: &PgPool,
    program_ids: &[ProgramId],
    f: SafetyIncidentFilters<'_>,
) -> Result<i64> {
    count_incidents(pool, &raw(program_ids), false, f).await
}

async fn incidents(
    pool: &PgPool,
    ids: &[Uuid],
    unscoped: bool,
    f: SafetyIncidentFilters<'_>,
    limit: i64,
    offset: i64,
) -> Result<Vec<SafetyIncidentRow>> {
    sqlx::query_as!(
        SafetyIncidentRow,
        r#"
        SELECT
            si.id          as "id!",
            si.session_id  as "session_id: SessionId",
            si.student_id  as "student_id!: StudentId",
            st.full_name   as "student_name!",
            st.roll_number as "roll_number!",
            si.kind        as "kind!",
            si.tier        as "tier!",
            si.excerpt     as "excerpt!",
            si.created_at  as "created_at!"
        FROM safety_incidents si
        JOIN students st ON st.id = si.student_id
        WHERE ($2 OR st.program_id = ANY($1))
          AND ($3::smallint    IS NULL OR si.tier       = $3)
          AND ($4::text        IS NULL OR si.kind       = $4)
          AND ($5::uuid        IS NULL OR si.student_id = $5)
          AND ($6::uuid        IS NULL OR si.session_id = $6)
          AND ($7::timestamptz IS NULL OR si.created_at >= $7)
          AND ($8::timestamptz IS NULL OR si.created_at <= $8)
        ORDER BY si.created_at DESC, si.id
        LIMIT $9 OFFSET $10
        "#,
        ids,
        unscoped,
        f.tier,
        f.kind,
        f.student_id,
        f.session_id,
        f.from,
        f.to,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

async fn count_incidents(
    pool: &PgPool,
    ids: &[Uuid],
    unscoped: bool,
    f: SafetyIncidentFilters<'_>,
) -> Result<i64> {
    sqlx::query_scalar!(
        r#"
        SELECT count(*) as "count!"
        FROM safety_incidents si
        JOIN students st ON st.id = si.student_id
        WHERE ($2 OR st.program_id = ANY($1))
          AND ($3::smallint    IS NULL OR si.tier       = $3)
          AND ($4::text        IS NULL OR si.kind       = $4)
          AND ($5::uuid        IS NULL OR si.student_id = $5)
          AND ($6::uuid        IS NULL OR si.session_id = $6)
          AND ($7::timestamptz IS NULL OR si.created_at >= $7)
          AND ($8::timestamptz IS NULL OR si.created_at <= $8)
        "#,
        ids,
        unscoped,
        f.tier,
        f.kind,
        f.student_id,
        f.session_id,
        f.from,
        f.to
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// `ProgramId` newtypes down to the raw `uuid[]` the scope predicate binds.
pub(crate) fn raw(program_ids: &[ProgramId]) -> Vec<Uuid> {
    program_ids.iter().map(|p| p.into_uuid()).collect()
}
