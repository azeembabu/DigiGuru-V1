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

/// Admin console listing, ordered by name.
///
/// `program_ids = None` means unrestricted (super-admin); a sub-admin passes
/// its `sub_admin_scopes`. The scope filter is applied **in SQL, before**
/// `LIMIT`/`OFFSET` — filtering a page after fetching it would hand a
/// sub-admin short or empty pages that do not correspond to any real offset
/// into their own visible set.
///
/// `q` is a case-insensitive substring match over `code` and `name`.
pub async fn list(
    pool: &PgPool,
    program_ids: Option<&[ProgramId]>,
    q: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Program>> {
    let raw_ids: Option<Vec<uuid::Uuid>> =
        program_ids.map(|ids| ids.iter().map(|p| p.into_uuid()).collect());

    sqlx::query_as!(
        Program,
        r#"
        SELECT id, code, name, description, status as "status: EntityStatus"
        FROM programs
        WHERE ($1::uuid[] IS NULL OR id = ANY($1))
          AND ($2::text IS NULL OR code ILIKE '%' || $2 || '%' OR name ILIKE '%' || $2 || '%')
        ORDER BY name
        LIMIT $3 OFFSET $4
        "#,
        raw_ids.as_deref(),
        q,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Every program, unpaginated — the reference-data catalogue behind
/// `GET /api/v1/reference/programs`, which is a signup form's dropdown and
/// has no page controls to drive an offset with.
pub async fn list_all(pool: &PgPool) -> Result<Vec<Program>> {
    sqlx::query_as!(
        Program,
        r#"SELECT id, code, name, description, status as "status: EntityStatus" FROM programs ORDER BY name"#
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Total matching `list`'s filters, ignoring `limit`/`offset` — the
/// `X-Total-Count` header of `GET /api/v1/admin/programs`.
pub async fn count(
    pool: &PgPool,
    program_ids: Option<&[ProgramId]>,
    q: Option<&str>,
) -> Result<i64> {
    let raw_ids: Option<Vec<uuid::Uuid>> =
        program_ids.map(|ids| ids.iter().map(|p| p.into_uuid()).collect());

    sqlx::query_scalar!(
        r#"
        SELECT count(*) as "count!"
        FROM programs
        WHERE ($1::uuid[] IS NULL OR id = ANY($1))
          AND ($2::text IS NULL OR code ILIKE '%' || $2 || '%' OR name ILIKE '%' || $2 || '%')
        "#,
        raw_ids.as_deref(),
        q
    )
    .fetch_one(pool)
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
