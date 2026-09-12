//! `semesters` — belongs to a `program`, numbered 1..=12.

use dg_core::{EntityStatus, ProgramId, SemesterId};
use sqlx::PgPool;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Semester {
    pub id: SemesterId,
    pub program_id: ProgramId,
    pub semester_number: i16,
    pub name: String,
    pub status: EntityStatus,
}

pub async fn create(
    pool: &PgPool,
    program_id: ProgramId,
    semester_number: i16,
    name: &str,
) -> Result<Semester> {
    sqlx::query_as!(
        Semester,
        r#"
        INSERT INTO semesters (program_id, semester_number, name)
        VALUES ($1, $2, $3)
        RETURNING id, program_id, semester_number, name, status as "status: EntityStatus"
        "#,
        program_id.into_uuid(),
        semester_number,
        name
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn find_by_id(pool: &PgPool, id: SemesterId) -> Result<Option<Semester>> {
    sqlx::query_as!(
        Semester,
        r#"SELECT id, program_id, semester_number, name, status as "status: EntityStatus" FROM semesters WHERE id = $1"#,
        id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Existence check for signup validation, additionally verifying the
/// semester actually belongs to the claimed program.
pub async fn belongs_to_program(pool: &PgPool, id: SemesterId, program_id: ProgramId) -> Result<bool> {
    let row = sqlx::query!(
        r#"SELECT 1 as present FROM semesters WHERE id = $1 AND program_id = $2 AND status = 'active'"#,
        id.into_uuid(),
        program_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(row.is_some())
}

pub async fn list_by_program(pool: &PgPool, program_id: ProgramId) -> Result<Vec<Semester>> {
    sqlx::query_as!(
        Semester,
        r#"
        SELECT id, program_id, semester_number, name, status as "status: EntityStatus"
        FROM semesters
        WHERE program_id = $1
        ORDER BY semester_number
        "#,
        program_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn update(pool: &PgPool, id: SemesterId, name: &str, status: EntityStatus) -> Result<()> {
    sqlx::query!(
        r#"UPDATE semesters SET name = $2, status = $3::text::entity_status WHERE id = $1"#,
        id.into_uuid(),
        name,
        status.as_db_str()
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}
