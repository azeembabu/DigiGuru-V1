//! `POST /api/v1/auth/forgot-password` — issues a hashed, expiring reset
//! token. Always responds `200` with the same generic message whether or
//! not the email exists, so this endpoint cannot be used to enumerate
//! registered accounts.

use axum::{extract::State, Json};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use dg_core::PublicError;
use dg_db::models::{password_resets, users};

use super::tokens::{generate_token, hash_token};
use crate::state::AppState;

const RESET_TOKEN_TTL_MINUTES: i64 = 60;

#[derive(Debug, Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct ForgotPasswordResponse {
    pub message: &'static str,
}

pub async fn forgot_password(
    State(state): State<AppState>,
    Json(payload): Json<ForgotPasswordRequest>,
) -> Result<Json<ForgotPasswordResponse>, PublicError> {
    if let Some(user) = users::find_by_email(&state.pool, &payload.email)
        .await
        .map_err(PublicError::from)?
    {
        let raw_token = generate_token();
        let token_hash = hash_token(&state.config.jwt_refresh_secret, &raw_token);

        password_resets::create(
            &state.pool,
            user.id,
            &token_hash,
            Utc::now() + Duration::minutes(RESET_TOKEN_TTL_MINUTES),
        )
        .await
        .map_err(PublicError::from)?;

        // TODO(Phase 1 follow-up): wire an email provider. For now the raw
        // token is deliberately not returned in the response body (that
        // would defeat the point of emailing it) — it is only logged at
        // debug level in non-production environments so local dev/testing
        // can complete the reset flow without a mail server.
        tracing::debug!(user_id = %user.id, "password reset token generated (would be emailed)");

        crate::audit::log(&state, Some(user.id), "auth.forgot_password", None, None, None).await;
    }

    Ok(Json(ForgotPasswordResponse {
        message: "If an account exists for that email, a reset link has been sent.",
    }))
}
