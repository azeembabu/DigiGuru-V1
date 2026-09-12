//! `courses` — belongs to a `program` + `semester`.

use dg_core::{CourseId, ProgramId, SemesterId};
use sqlx::PgPool;

use crate::error::{Error, Result};

/// A course carries its semester's number and name denormalised from
/// `semesters` by every query in this module.
///
/// Same reasoning as `students::StudentDetail`: the admin console shows a
/// semester label on each course row, and `semesters` is otherwise reachable
/// only per program, so labelling rows client-side would mean a fan-out
/// across the catalogue for one page of courses. The join is free here —
/// `courses.semester_id` is a foreign key, always present.
///
/// This changes nothing about the semester data model: `semesters` and
/// `courses.semester_id` are untouched, and this struct only reads them.
#[derive(Debug, Clone)]
pub struct Course {
    pub id: CourseId,
    pub program_id: ProgramId,
    pub semester_id: SemesterId,
    pub semester_number: i16,
    pub semester_name: String,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}

pub async fn create(
    pool: &PgPool,
    program_id: ProgramId,
    semester_id: SemesterId,
    code: &str,
    name: &str,
    description: Option<&str>,
) -> Result<Course> {
    sqlx::query_as!(
        Course,
        r#"
        WITH inserted AS (
            INSERT INTO courses (program_id, semester_id, code, name, description)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, program_id, semester_id, code, name, description
        )
        SELECT c.id as "id!: CourseId", c.program_id as "program_id!: ProgramId",
               c.semester_id as "semester_id!: SemesterId",
               sem.semester_number as "semester_number!", sem.name as "semester_name!",
               c.code as "code!", c.name as "name!", c.description
        FROM inserted c
        JOIN semesters sem ON sem.id = c.semester_id
        "#,
        program_id.into_uuid(),
        semester_id.into_uuid(),
        code,
        name,
        description
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn find_by_id(pool: &PgPool, id: CourseId) -> Result<Option<Course>> {
    sqlx::query_as!(
        Course,
        r#"
        SELECT c.id as "id!: CourseId", c.program_id as "program_id!: ProgramId",
               c.semester_id as "semester_id!: SemesterId",
               sem.semester_number as "semester_number!", sem.name as "semester_name!",
               c.code as "code!", c.name as "name!", c.description
        FROM courses c
        JOIN semesters sem ON sem.id = c.semester_id
        WHERE c.id = $1
        "#,
        id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn list_by_semester(pool: &PgPool, semester_id: SemesterId) -> Result<Vec<Course>> {
    sqlx::query_as!(
        Course,
        r#"
        SELECT c.id as "id!: CourseId", c.program_id as "program_id!: ProgramId",
               c.semester_id as "semester_id!: SemesterId",
               sem.semester_number as "semester_number!", sem.name as "semester_name!",
               c.code as "code!", c.name as "name!", c.description
        FROM courses c
        JOIN semesters sem ON sem.id = c.semester_id
        WHERE c.semester_id = $1
        ORDER BY c.code
        "#,
        semester_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Every course in a program, across all of its semesters.
///
/// Ordered by `semester_number` then `code` so a console that groups rows by
/// semester can render straight down the list without re-sorting. `q` is a
/// case-insensitive substring match over course code and name.
///
/// Complements `list_by_semester` rather than replacing it — the per-semester
/// list is still the right query when a semester is what the caller has.
pub async fn list_by_program(
    pool: &PgPool,
    program_id: ProgramId,
    q: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Course>> {
    sqlx::query_as!(
        Course,
        r#"
        SELECT c.id as "id!: CourseId", c.program_id as "program_id!: ProgramId",
               c.semester_id as "semester_id!: SemesterId",
               sem.semester_number as "semester_number!", sem.name as "semester_name!",
               c.code as "code!", c.name as "name!", c.description
        FROM courses c
        JOIN semesters sem ON sem.id = c.semester_id
        WHERE c.program_id = $1
          AND ($2::text IS NULL OR c.code ILIKE '%' || $2 || '%' OR c.name ILIKE '%' || $2 || '%')
        ORDER BY sem.semester_number ASC, c.code ASC
        LIMIT $3 OFFSET $4
        "#,
        program_id.into_uuid(),
        q,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Total matching `list_by_program`'s filter, ignoring `limit`/`offset` —
/// the `X-Total-Count` header of `GET /api/v1/admin/programs/{id}/courses`.
pub async fn count_by_program(
    pool: &PgPool,
    program_id: ProgramId,
    q: Option<&str>,
) -> Result<i64> {
    sqlx::query_scalar!(
        r#"
        SELECT count(*) as "count!"
        FROM courses c
        WHERE c.program_id = $1
          AND ($2::text IS NULL OR c.code ILIKE '%' || $2 || '%' OR c.name ILIKE '%' || $2 || '%')
        "#,
        program_id.into_uuid(),
        q
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn update(
    pool: &PgPool,
    id: CourseId,
    name: &str,
    description: Option<&str>,
) -> Result<()> {
    sqlx::query!(
        r#"UPDATE courses SET name = $2, description = $3 WHERE id = $1"#,
        id.into_uuid(),
        name,
        description
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}
