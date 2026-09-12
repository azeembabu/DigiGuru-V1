//! `POST /api/v1/auth/reset-password` — redeems a reset token issued by
//! `forgot_password`. On success, every outstanding `auth_sessions` row for
//! the user is revoked, so a compromised session cannot survive a password
//! reset.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use validator::Validate;

use dg_core::PublicError;
use dg_db::models::{auth_sessions, password_resets, users};

use super::password::hash_password;
use super::tokens::hash_token;
use crate::state::AppState;

#[derive(Debug, Deserialize, Validate)]
pub struct ResetPasswordRequest {
    pub token: String,
    #[validate(length(min = 8, max = 128, message = "must be 8-128 characters"))]
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct ResetPasswordResponse {
    pub message: &'static str,
}

pub async fn reset_password(
    State(state): State<AppState>,
    Json(payload): Json<ResetPasswordRequest>,
) -> Result<Json<ResetPasswordResponse>, PublicError> {
    payload
        .validate()
        .map_err(|_| PublicError::validation("new_password", "must be 8-128 characters"))?;

    let token_hash = hash_token(&state.config.jwt_refresh_secret, &payload.token);

    let reset = password_resets::find_valid_by_hash(&state.pool, &token_hash)
        .await
        .map_err(PublicError::from)?
        .ok_or_else(|| PublicError::validation("token", "invalid or expired"))?;

    let user = users::find_by_id(&state.pool, reset.user_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::Unauthorized)?;

    let password_hash = hash_password(&payload.new_password).map_err(|err| {
        tracing::error!(error = %err, "password hashing failed");
        PublicError::Internal
    })?;

    users::update_password_hash(&state.pool, user.id, &password_hash)
        .await
        .map_err(PublicError::from)?;

    password_resets::mark_used(&state.pool, reset.id).await.map_err(PublicError::from)?;
    password_resets::invalidate_all_for_user(&state.pool, user.id)
        .await
        .map_err(PublicError::from)?;

    // A password reset invalidates every existing device session.
    auth_sessions::revoke_all_for_user(&state.pool, user.id)
        .await
        .map_err(PublicError::from)?;

    crate::audit::log(&state, Some(user.id), "auth.reset_password", None, None, None).await;

    Ok(Json(ResetPasswordResponse { message: "Password updated. Please log in again." }))
}
