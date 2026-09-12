//! `courses` — belongs to a `program` + `semester`.

use dg_core::{CourseId, ProgramId, SemesterId};
use sqlx::PgPool;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Course {
    pub id: CourseId,
    pub program_id: ProgramId,
    pub semester_id: SemesterId,
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
        INSERT INTO courses (program_id, semester_id, code, name, description)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, program_id, semester_id, code, name, description
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
        SELECT id, program_id, semester_id, code, name, description
        FROM courses WHERE id = $1
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
        SELECT id, program_id, semester_id, code, name, description
        FROM courses WHERE semester_id = $1 ORDER BY code
        "#,
        semester_id.into_uuid()
    )
    .fetch_all(pool)
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
