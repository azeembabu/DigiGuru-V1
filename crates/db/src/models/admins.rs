//! `admins` — the `super_admin`/`sub_admin` profile row, 1:1 with `users`.

use dg_core::UserId;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Admin {
    pub id: Uuid,
    pub user_id: UserId,
    pub full_name: String,
}

pub async fn create(pool: &PgPool, user_id: UserId, full_name: &str) -> Result<Admin> {
    sqlx::query_as!(
        Admin,
        r#"
        INSERT INTO admins (user_id, full_name)
        VALUES ($1, $2)
        RETURNING id, user_id, full_name
        "#,
        user_id.into_uuid(),
        full_name
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn find_by_user_id(pool: &PgPool, user_id: UserId) -> Result<Option<Admin>> {
    sqlx::query_as!(
        Admin,
        r#"SELECT id, user_id, full_name FROM admins WHERE user_id = $1"#,
        user_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn update_full_name(pool: &PgPool, user_id: UserId, full_name: &str) -> Result<()> {
    sqlx::query!(
        r#"UPDATE admins SET full_name = $2 WHERE user_id = $1"#,
        user_id.into_uuid(),
        full_name
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}
