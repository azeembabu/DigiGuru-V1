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

pub async fn add_scope(pool: &PgPool, user_id: UserId, program_id: ProgramId) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO sub_admin_scopes (user_id, program_id)
        VALUES ($1, $2)
        ON CONFLICT (user_id, program_id) DO NOTHING
        "#,
        user_id.into_uuid(),
        program_id.into_uuid()
    )
    .execute(pool)
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
