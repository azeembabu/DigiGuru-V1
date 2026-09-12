//! `/api/v1/auth/*` — signup, login, refresh, logout, forgot/reset
//! password, and own-session list/revoke. See `IMPLEMENTATION_PLAN.md`
//! §4.1 items 3-4 and `.claude/rules/api-conventions.md`.

pub mod cookies;
pub mod forgot_password;
pub mod jwt;
pub mod login;
pub mod logout;
pub mod password;
pub mod refresh;
pub mod reset_password;
pub mod sessions;
pub mod signup;
pub mod tokens;

use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/signup", post(signup::signup))
        .route("/login", post(login::login))
        .route("/refresh", post(refresh::refresh))
        .route("/logout", post(logout::logout))
        .route("/forgot-password", post(forgot_password::forgot_password))
        .route("/reset-password", post(reset_password::reset_password))
        .route("/sessions", get(sessions::list_sessions))
        .route("/sessions/{id}", delete(sessions::revoke_session))
}
