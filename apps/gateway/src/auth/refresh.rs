//! `POST /api/v1/auth/refresh` — rotates the refresh token and issues a new
//! access token. The old `auth_sessions` row is revoked (never reused), and
//! a new one is inserted, so a replayed old refresh token is detectable.

use axum::extract::{ConnectInfo, State};
use axum_extra::extract::CookieJar;
use chrono::{Duration, Utc};

use dg_core::PublicError;
use dg_db::models::{auth_sessions, users};

use super::cookies::{access_cookie, refresh_cookie, REFRESH_COOKIE, REFRESH_TOKEN_TTL_DAYS};
use super::jwt::issue_access_token;
use super::tokens::{generate_token, hash_token};
use crate::extractors::ClientIp;
use crate::state::AppState;

pub async fn refresh(
    State(state): State<AppState>,
    ClientIp(ip_address): ClientIp,
    jar: CookieJar,
) -> Result<CookieJar, PublicError> {
    let raw_refresh = jar
        .get(REFRESH_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or(PublicError::Unauthorized)?;

    let refresh_hash = hash_token(&state.config.jwt_refresh_secret, &raw_refresh);

    let session = auth_sessions::find_active_by_hash(&state.pool, &refresh_hash)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::Unauthorized)?;

    let user = users::find_by_id(&state.pool, session.user_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::Unauthorized)?;

    if !matches!(user.status, dg_core::UserStatus::Active) {
        return Err(PublicError::Forbidden);
    }

    // Rotate: revoke the presented session, mint a fresh one.
    auth_sessions::revoke(&state.pool, session.id).await.map_err(PublicError::from)?;

    let new_raw_refresh = generate_token();
    let new_refresh_hash = hash_token(&state.config.jwt_refresh_secret, &new_raw_refresh);

    auth_sessions::create(
        &state.pool,
        user.id,
        &new_refresh_hash,
        session.device_info.as_deref(),
        ip_address,
        Utc::now() + Duration::days(REFRESH_TOKEN_TTL_DAYS),
    )
    .await
    .map_err(PublicError::from)?;

    let access_token = issue_access_token(user.id.into_uuid(), user.role, &state.config.jwt_access_secret)
        .map_err(|err| {
            tracing::error!(error = %err, "access token issuance failed");
            PublicError::Internal
        })?;

    let jar = jar.add(access_cookie(access_token)).add(refresh_cookie(new_raw_refresh));

    Ok(jar)
}
