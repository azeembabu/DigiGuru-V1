//! `users` — the base identity row shared by every role.
//!
//! Password hashing (argon2id) happens in `apps/gateway`; this module only
//! stores/reads the resulting hash and never logs it.

use chrono::{DateTime, Utc};
use dg_core::{Role, UserId, UserStatus};
use sqlx::PgPool;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct User {
    pub id: UserId,
    pub role: Role,
    pub status: UserStatus,
    pub email: String,
    pub password_hash: String,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Create the base `users` row. Role-specific profile rows (`admins`,
/// `students`) are created by the caller in the same transaction.
pub async fn create(
    pool: &PgPool,
    role: Role,
    email: &str,
    password_hash: &str,
) -> Result<User> {
    sqlx::query_as!(
        User,
        r#"
        INSERT INTO users (role, email, password_hash)
        VALUES ($1::text::user_role, $2, $3)
        RETURNING id, role as "role: Role", status as "status: UserStatus", email, password_hash, last_login_at, created_at, updated_at
        "#,
        role.as_db_str(),
        email,
        password_hash
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<Option<User>> {
    sqlx::query_as!(
        User,
        r#"
        SELECT id, role as "role: Role", status as "status: UserStatus", email, password_hash, last_login_at, created_at, updated_at
        FROM users
        WHERE email = $1
        "#,
        email
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn find_by_id(pool: &PgPool, id: UserId) -> Result<Option<User>> {
    sqlx::query_as!(
        User,
        r#"
        SELECT id, role as "role: Role", status as "status: UserStatus", email, password_hash, last_login_at, created_at, updated_at
        FROM users
        WHERE id = $1
        "#,
        id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Admin console listing — every user, newest first. Callers filter by role
/// or status in the handler layer; this scaffold keeps the query simple and
/// paginates via `limit`/`offset`.
pub async fn list(pool: &PgPool, limit: i64, offset: i64) -> Result<Vec<User>> {
    sqlx::query_as!(
        User,
        r#"
        SELECT id, role as "role: Role", status as "status: UserStatus", email, password_hash, last_login_at, created_at, updated_at
        FROM users
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn touch_last_login(pool: &PgPool, id: UserId) -> Result<()> {
    sqlx::query!(
        r#"UPDATE users SET last_login_at = now(), updated_at = now() WHERE id = $1"#,
        id.into_uuid()
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}

pub async fn update_password_hash(pool: &PgPool, id: UserId, password_hash: &str) -> Result<()> {
    sqlx::query!(
        r#"UPDATE users SET password_hash = $2, updated_at = now() WHERE id = $1"#,
        id.into_uuid(),
        password_hash
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}

pub async fn set_status(pool: &PgPool, id: UserId, status: UserStatus) -> Result<()> {
    sqlx::query!(
        r#"UPDATE users SET status = $2::text::user_status, updated_at = now() WHERE id = $1"#,
        id.into_uuid(),
        status.as_db_str()
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}
