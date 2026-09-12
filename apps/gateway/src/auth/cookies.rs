//! httpOnly, SameSite=Strict cookie construction for the access and refresh
//! tokens (`.claude/rules/api-conventions.md`, `.claude/rules/security.md`).

use axum_extra::extract::cookie::{Cookie, SameSite};
use time::Duration;

use super::jwt::ACCESS_TOKEN_TTL_MINUTES;

pub const ACCESS_COOKIE: &str = "dg_access";
pub const REFRESH_COOKIE: &str = "dg_refresh";

/// Refresh tokens live for 30 days of inactivity; each successful
/// `/auth/refresh` call rotates both the token and this expiry.
pub const REFRESH_TOKEN_TTL_DAYS: i64 = 30;

fn base_cookie(name: &'static str, value: String) -> Cookie<'static> {
    Cookie::build((name, value))
        .http_only(true)
        .same_site(SameSite::Strict)
        // `secure` is only meaningful over HTTPS; every non-local
        // environment terminates TLS in front of the gateway
        // (`.claude/rules/security.md`: "TLS 1.3 everywhere ... WSS only").
        .secure(true)
        .path("/")
        .build()
}

pub fn access_cookie(token: String) -> Cookie<'static> {
    let mut cookie = base_cookie(ACCESS_COOKIE, token);
    cookie.set_max_age(Duration::minutes(ACCESS_TOKEN_TTL_MINUTES));
    cookie
}

pub fn refresh_cookie(token: String) -> Cookie<'static> {
    let mut cookie = base_cookie(REFRESH_COOKIE, token);
    cookie.set_max_age(Duration::days(REFRESH_TOKEN_TTL_DAYS));
    cookie
}

/// An immediately-expired cookie, used to clear a cookie on logout.
pub fn expired(name: &'static str) -> Cookie<'static> {
    let mut cookie = base_cookie(name, String::new());
    cookie.set_max_age(Duration::seconds(0));
    cookie
}
