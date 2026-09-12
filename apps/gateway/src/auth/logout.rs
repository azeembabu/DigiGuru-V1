//! `POST /api/v1/auth/logout` — revokes the presented refresh token's
//! `auth_sessions` row (if any) and clears both cookies. Always succeeds
//! from the client's point of view, even if the refresh token was already
//! invalid — logging out of an already-dead session is not an error.

use axum::extract::State;
use axum_extra::extract::CookieJar;

use dg_core::PublicError;
use dg_db::models::auth_sessions;

use super::cookies::{expired, ACCESS_COOKIE, REFRESH_COOKIE};
use super::tokens::hash_token;
use crate::state::AppState;

pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> Result<CookieJar, PublicError> {
    if let Some(raw_refresh) = jar.get(REFRESH_COOKIE).map(|c| c.value().to_string()) {
        let refresh_hash = hash_token(&state.config.jwt_refresh_secret, &raw_refresh);
        if let Some(session) = auth_sessions::find_active_by_hash(&state.pool, &refresh_hash)
            .await
            .map_err(PublicError::from)?
        {
            auth_sessions::revoke(&state.pool, session.id).await.map_err(PublicError::from)?;
            crate::audit::log(&state, Some(session.user_id), "auth.logout", None, None, None).await;
        }
    }

    Ok(jar.add(expired(ACCESS_COOKIE)).add(expired(REFRESH_COOKIE)))
}
