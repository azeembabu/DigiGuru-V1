//! `lscs` — Learner Support Centre, an entity (not a free-text field).

use dg_core::{EntityStatus, LscId};
use sqlx::PgPool;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Lsc {
    pub id: LscId,
    pub code: String,
    pub name: String,
    pub location: Option<String>,
    pub status: EntityStatus,
}

pub async fn create(
    pool: &PgPool,
    code: &str,
    name: &str,
    location: Option<&str>,
) -> Result<Lsc> {
    sqlx::query_as!(
        Lsc,
        r#"
        INSERT INTO lscs (code, name, location)
        VALUES ($1, $2, $3)
        RETURNING id, code, name, location, status as "status: EntityStatus"
        "#,
        code,
        name,
        location
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn find_by_id(pool: &PgPool, id: LscId) -> Result<Option<Lsc>> {
    sqlx::query_as!(
        Lsc,
        r#"SELECT id, code, name, location, status as "status: EntityStatus" FROM lscs WHERE id = $1"#,
        id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn list(pool: &PgPool) -> Result<Vec<Lsc>> {
    sqlx::query_as!(Lsc, r#"SELECT id, code, name, location, status as "status: EntityStatus" FROM lscs ORDER BY name"#)
        .fetch_all(pool)
        .await
        .map_err(Error::from_sqlx)
}

pub async fn update(
    pool: &PgPool,
    id: LscId,
    name: &str,
    location: Option<&str>,
    status: EntityStatus,
) -> Result<()> {
    sqlx::query!(
        r#"UPDATE lscs SET name = $2, location = $3, status = $4::text::entity_status WHERE id = $1"#,
        id.into_uuid(),
        name,
        location,
        status.as_db_str()
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}
