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
///
/// Generic over the executor rather than taking `&PgPool` so that caller
/// *can* actually be a transaction: `&PgPool` still passes unchanged, and a
/// `&mut PgConnection` from `pool.begin()` now passes too.
pub async fn create<'e, E: sqlx::PgExecutor<'e>>(
    executor: E,
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
    .fetch_one(executor)
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

/// Admin console listing, newest first.
///
/// `q` is a case-insensitive substring match on `email` — the only
/// identifying field on a `users` row (display names live on the
/// role-specific `admins`/`students` rows). `role` and `status` are exact
/// filters so an admin-users screen can ask for "all sub_admins" or "all
/// suspended accounts" server-side instead of paging the whole table and
/// filtering client-side.
pub async fn list(
    pool: &PgPool,
    q: Option<&str>,
    role: Option<Role>,
    status: Option<UserStatus>,
    limit: i64,
    offset: i64,
) -> Result<Vec<User>> {
    sqlx::query_as!(
        User,
        r#"
        SELECT id, role as "role: Role", status as "status: UserStatus", email, password_hash, last_login_at, created_at, updated_at
        FROM users
        WHERE ($1::text IS NULL OR email ILIKE '%' || $1 || '%')
          AND ($2::text IS NULL OR role = $2::text::user_role)
          AND ($3::text IS NULL OR status = $3::text::user_status)
        ORDER BY created_at DESC
        LIMIT $4 OFFSET $5
        "#,
        q,
        role.map(Role::as_db_str),
        status.map(UserStatus::as_db_str),
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Total matching `list`'s filters, ignoring `limit`/`offset` — the
/// `X-Total-Count` header of `GET /api/v1/admin/users`.
pub async fn count(
    pool: &PgPool,
    q: Option<&str>,
    role: Option<Role>,
    status: Option<UserStatus>,
) -> Result<i64> {
    sqlx::query_scalar!(
        r#"
        SELECT count(*) as "count!"
        FROM users
        WHERE ($1::text IS NULL OR email ILIKE '%' || $1 || '%')
          AND ($2::text IS NULL OR role = $2::text::user_role)
          AND ($3::text IS NULL OR status = $3::text::user_status)
        "#,
        q,
        role.map(Role::as_db_str),
        status.map(UserStatus::as_db_str)
    )
    .fetch_one(pool)
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
