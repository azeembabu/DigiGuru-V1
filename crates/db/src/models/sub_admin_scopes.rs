//! `sub_admin_scopes` — which programs a sub-admin may touch. This table is
//! the enforcement point for "a sub-admin for BA Malayalam cannot touch BCom
//! content" (`IMPLEMENTATION_PLAN.md` §4.1 item 2). Every admin handler that
//! accepts a sub-admin must load this via `list_for_user` and hand the
//! result to `dg_core::Actor::scopes`.

use dg_core::{ProgramId, UserId};
use sqlx::PgPool;

use crate::error::{Error, Result};

pub async fn list_for_user(pool: &PgPool, user_id: UserId) -> Result<Vec<ProgramId>> {
    let rows = sqlx::query!(
        r#"SELECT program_id FROM sub_admin_scopes WHERE user_id = $1"#,
        user_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(rows.into_iter().map(|r| ProgramId::from(r.program_id)).collect())
}

/// Executor-generic so initial scopes can be granted inside the same
/// transaction that creates the sub-admin — see `users::create`.
pub async fn add_scope<'e, E: sqlx::PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
    program_id: ProgramId,
) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO sub_admin_scopes (user_id, program_id)
        VALUES ($1, $2)
        ON CONFLICT (user_id, program_id) DO NOTHING
        "#,
        user_id.into_uuid(),
        program_id.into_uuid()
    )
    .execute(executor)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}

pub async fn remove_scope(pool: &PgPool, user_id: UserId, program_id: ProgramId) -> Result<()> {
    sqlx::query!(
        r#"DELETE FROM sub_admin_scopes WHERE user_id = $1 AND program_id = $2"#,
        user_id.into_uuid(),
        program_id.into_uuid()
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}

pub async fn is_in_scope(pool: &PgPool, user_id: UserId, program_id: ProgramId) -> Result<bool> {
    let row = sqlx::query!(
        r#"SELECT 1 as present FROM sub_admin_scopes WHERE user_id = $1 AND program_id = $2"#,
        user_id.into_uuid(),
        program_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(row.is_some())
}

/// One `sub_admin_scopes` row resolved against `programs`.
#[derive(Debug, Clone)]
pub struct ScopedProgram {
    pub program_id: ProgramId,
    pub code: String,
    pub name: String,
}

/// The caller's own scopes with the program `code`/`name` joined in, for
/// `GET /api/v1/me` — an admin shell renders "scoped to: BA Malayalam", and
/// a bare `Vec<ProgramId>` would force it into a second round trip per id.
pub async fn list_with_programs(pool: &PgPool, user_id: UserId) -> Result<Vec<ScopedProgram>> {
    sqlx::query_as!(
        ScopedProgram,
        r#"
        SELECT s.program_id as "program_id: ProgramId", p.code, p.name
        FROM sub_admin_scopes s
        JOIN programs p ON p.id = s.program_id
        WHERE s.user_id = $1
        ORDER BY p.name
        "#,
        user_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}
