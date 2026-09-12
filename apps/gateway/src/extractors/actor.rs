//! The `Actor` extractor: resolves the authenticated caller from the
//! access-token cookie on every request that needs one.
//!
//! `.claude/rules/api-conventions.md` sketches capability checking as a
//! const-generic extractor (`Actor<RequireCap<{ Cap::UploadDocument }>>`).
//! Enums as `const` generic parameters are not stable Rust (the
//! `adt_const_params` feature is nightly-only), so this scaffold uses the
//! equivalent, stable pattern instead: every handler extracts
//! `AuthenticatedActor` and calls `actor.require(Capability::X)` or
//! `actor.require_scoped(Capability::X, program_id)` as its first line. The
//! guarantee is the same — no implicit authorization, and the check cannot
//! be skipped without it being visible in the handler body — just spelled
//! as a method call instead of a type parameter. Flagged as a judgment call
//! in the Phase 1 scaffold report.

use axum::{extract::FromRequestParts, http::request::Parts};
use axum_extra::extract::CookieJar;

use dg_core::{Actor, PublicError, Role};
use dg_db::models::sub_admin_scopes;

use crate::auth::cookies::ACCESS_COOKIE;
use crate::auth::jwt::verify_access_token;
use crate::state::AppState;

/// Wraps `dg_core::Actor` so it can be extracted directly in a handler
/// signature: `AuthenticatedActor(actor): AuthenticatedActor`.
pub struct AuthenticatedActor(pub Actor);

impl FromRequestParts<AppState> for AuthenticatedActor {
    type Rejection = PublicError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(ACCESS_COOKIE)
            .map(|c| c.value().to_string())
            .ok_or(PublicError::Unauthorized)?;

        let claims = verify_access_token(&token, &state.config.jwt_access_secret)
            .map_err(|_| PublicError::Unauthorized)?;

        let user_id = dg_core::UserId::from(claims.sub);
        let scopes = if claims.role == Role::SubAdmin {
            sub_admin_scopes::list_for_user(&state.pool, user_id)
                .await
                .map_err(PublicError::from)?
        } else {
            Vec::new()
        };

        Ok(AuthenticatedActor(Actor::new(user_id, claims.role, scopes)))
    }
}
