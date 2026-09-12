//! `password_resets` — hashed, single-use, expiring reset tokens.
//!
//! Like `auth_sessions.refresh_token_hash`, the gateway only ever persists a
//! hash of the reset token; the raw token is emailed to the user and never
//! stored.

use chrono::{DateTime, Utc};
use dg_core::UserId;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct PasswordReset {
    pub id: Uuid,
    pub user_id: UserId,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub async fn create(
    pool: &PgPool,
    user_id: UserId,
    token_hash: &str,
    expires_at: DateTime<Utc>,
) -> Result<PasswordReset> {
    sqlx::query_as!(
        PasswordReset,
        r#"
        INSERT INTO password_resets (user_id, token_hash, expires_at)
        VALUES ($1, $2, $3)
        RETURNING id, user_id, token_hash, expires_at, used_at, created_at
        "#,
        user_id,
        token_hash,
        expires_at
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// A still-valid (unused, unexpired) reset row for this token hash.
pub async fn find_valid_by_hash(pool: &PgPool, token_hash: &str) -> Result<Option<PasswordReset>> {
    sqlx::query_as!(
        PasswordReset,
        r#"
        SELECT id, user_id, token_hash, expires_at, used_at, created_at
        FROM password_resets
        WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now()
        "#,
        token_hash
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn mark_used(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query!(r#"UPDATE password_resets SET used_at = now() WHERE id = $1"#, id)
        .execute(pool)
        .await
        .map_err(Error::from_sqlx)?;
    Ok(())
}

/// Invalidate any outstanding reset tokens for a user — called right after a
/// successful reset, so an older, still-unused email link can't also be
/// redeemed.
pub async fn invalidate_all_for_user(pool: &PgPool, user_id: UserId) -> Result<()> {
    sqlx::query!(
        r#"UPDATE password_resets SET used_at = now() WHERE user_id = $1 AND used_at IS NULL"#,
        user_id
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}
