//! `auth_sessions` — refresh-token / device sessions. Deliberately distinct
//! from `learning_sessions` (the classroom session) — see `CLAUDE.md`.
//!
//! The gateway never stores a raw refresh token; only its hash reaches this
//! module (`refresh_token_hash`). Rotation is: on `/auth/refresh`, look up
//! the presented token's hash, revoke that row, and insert a new one — never
//! update a row's hash in place, so a stolen-and-replayed old token is
//! detectable (its row is already revoked).

use chrono::{DateTime, Utc};
use dg_core::UserId;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct AuthSession {
    pub id: Uuid,
    pub user_id: UserId,
    pub refresh_token_hash: String,
    pub device_info: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub async fn create(
    pool: &PgPool,
    user_id: UserId,
    refresh_token_hash: &str,
    device_info: Option<&str>,
    ip_address: Option<std::net::IpAddr>,
    expires_at: DateTime<Utc>,
) -> Result<AuthSession> {
    sqlx::query_as!(
        AuthSession,
        r#"
        INSERT INTO auth_sessions (user_id, refresh_token_hash, device_info, ip_address, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, user_id, refresh_token_hash, device_info,
                  ip_address::text as ip_address, created_at, expires_at, revoked_at
        "#,
        user_id,
        refresh_token_hash,
        device_info,
        ip_address as Option<std::net::IpAddr>,
        expires_at
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Look up a session by its refresh token's hash. Only returns sessions that
/// are neither revoked nor expired — an expired/revoked hit is treated the
/// same as "not found" by the caller (no token-existence oracle).
pub async fn find_active_by_hash(pool: &PgPool, refresh_token_hash: &str) -> Result<Option<AuthSession>> {
    sqlx::query_as!(
        AuthSession,
        r#"
        SELECT id, user_id, refresh_token_hash, device_info,
               ip_address::text as ip_address, created_at, expires_at, revoked_at
        FROM auth_sessions
        WHERE refresh_token_hash = $1 AND revoked_at IS NULL AND expires_at > now()
        "#,
        refresh_token_hash
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn list_active_for_user(pool: &PgPool, user_id: UserId) -> Result<Vec<AuthSession>> {
    sqlx::query_as!(
        AuthSession,
        r#"
        SELECT id, user_id, refresh_token_hash, device_info,
               ip_address::text as ip_address, created_at, expires_at, revoked_at
        FROM auth_sessions
        WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > now()
        ORDER BY created_at DESC
        "#,
        user_id
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn revoke(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query!(r#"UPDATE auth_sessions SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL"#, id)
        .execute(pool)
        .await
        .map_err(Error::from_sqlx)?;
    Ok(())
}

/// Revoke a specific session, but only if it belongs to `user_id` — the
/// check `DELETE /api/v1/auth/sessions/:id` needs so a user can never revoke
/// someone else's session by guessing an id.
pub async fn revoke_for_user(pool: &PgPool, id: Uuid, user_id: UserId) -> Result<bool> {
    let result = sqlx::query!(
        r#"
        UPDATE auth_sessions SET revoked_at = now()
        WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL
        "#,
        id,
        user_id
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(result.rows_affected() > 0)
}

pub async fn revoke_all_for_user(pool: &PgPool, user_id: UserId) -> Result<()> {
    sqlx::query!(
        r#"UPDATE auth_sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL"#,
        user_id
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}
