//! `POST /api/v1/auth/login` — argon2id verify, issues the access-token
//! cookie plus a rotating refresh token hashed into `auth_sessions`.

use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};
use axum_extra::extract::CookieJar;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::PublicError;
use dg_db::models::{auth_sessions, users};

use super::cookies::{access_cookie, refresh_cookie, REFRESH_TOKEN_TTL_DAYS};
use super::jwt::issue_access_token;
use super::password::verify_password;
use super::tokens::{generate_token, hash_token};
use crate::extractors::ClientIp;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub user_id: Uuid,
    pub role: dg_core::Role,
    pub is_first_login: Option<bool>,
}

pub async fn login(
    State(state): State<AppState>,
    ClientIp(ip_address): ClientIp,
    headers: HeaderMap,
    jar: CookieJar,
    Json(payload): Json<LoginRequest>,
) -> Result<(CookieJar, Json<LoginResponse>), PublicError> {
    // Deliberately the same error for "no such email" and "wrong password"
    // — an auth endpoint must not leak which one it was.
    let invalid_credentials = || PublicError::Unauthorized;

    let user = users::find_by_email(&state.pool, &payload.email)
        .await
        .map_err(PublicError::from)?
        .ok_or_else(invalid_credentials)?;

    verify_password(&payload.password, &user.password_hash).map_err(|_| invalid_credentials())?;

    if !matches!(user.status, dg_core::UserStatus::Active) {
        return Err(PublicError::Forbidden);
    }

    let access_token = issue_access_token(user.id.into_uuid(), user.role, &state.config.jwt_access_secret)
        .map_err(|err| {
            tracing::error!(error = %err, "access token issuance failed");
            PublicError::Internal
        })?;

    let refresh_raw = generate_token();
    let refresh_hash = hash_token(&state.config.jwt_refresh_secret, &refresh_raw);
    let device_info = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    auth_sessions::create(
        &state.pool,
        user.id,
        &refresh_hash,
        device_info.as_deref(),
        ip_address,
        Utc::now() + Duration::days(REFRESH_TOKEN_TTL_DAYS),
    )
    .await
    .map_err(PublicError::from)?;

    users::touch_last_login(&state.pool, user.id).await.map_err(PublicError::from)?;

    let is_first_login = if user.role == dg_core::Role::Student {
        dg_db::models::students::find_by_user_id(&state.pool, user.id)
            .await
            .map_err(PublicError::from)?
            .map(|s| s.is_first_login)
    } else {
        None
    };

    crate::audit::log(&state, Some(user.id), "auth.login", ip_address, device_info.as_deref(), None)
        .await;

    let jar = jar
        .add(access_cookie(access_token))
        .add(refresh_cookie(refresh_raw));

    Ok((
        jar,
        Json(LoginResponse {
            user_id: user.id.into_uuid(),
            role: user.role,
            is_first_login,
        }),
    ))
}
