//! `programs` — e.g. "BA Malayalam". Top of the academic hierarchy.

use dg_core::{EntityStatus, ProgramId};
use sqlx::PgPool;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Program {
    pub id: ProgramId,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub status: EntityStatus,
}

pub async fn create(
    pool: &PgPool,
    code: &str,
    name: &str,
    description: Option<&str>,
) -> Result<Program> {
    sqlx::query_as!(
        Program,
        r#"
        INSERT INTO programs (code, name, description)
        VALUES ($1, $2, $3)
        RETURNING id, code, name, description, status as "status: EntityStatus"
        "#,
        code,
        name,
        description
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn find_by_id(pool: &PgPool, id: ProgramId) -> Result<Option<Program>> {
    sqlx::query_as!(
        Program,
        r#"SELECT id, code, name, description, status as "status: EntityStatus" FROM programs WHERE id = $1"#,
        id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Existence check used to validate a signup payload's `program_id` before
/// it is trusted (`IMPLEMENTATION_PLAN.md` §4.1 item 3: "must resolve to
/// existing rows, not free text").
pub async fn exists(pool: &PgPool, id: ProgramId) -> Result<bool> {
    let row = sqlx::query!(r#"SELECT 1 as present FROM programs WHERE id = $1 AND status = 'active'"#, id.into_uuid())
        .fetch_optional(pool)
        .await
        .map_err(Error::from_sqlx)?;
    Ok(row.is_some())
}

pub async fn list(pool: &PgPool) -> Result<Vec<Program>> {
    sqlx::query_as!(
        Program,
        r#"SELECT id, code, name, description, status as "status: EntityStatus" FROM programs ORDER BY name"#
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn update(
    pool: &PgPool,
    id: ProgramId,
    name: &str,
    description: Option<&str>,
    status: EntityStatus,
) -> Result<()> {
    sqlx::query!(
        r#"UPDATE programs SET name = $2, description = $3, status = $4::text::entity_status WHERE id = $1"#,
        id.into_uuid(),
        name,
        description,
        status.as_db_str()
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}
