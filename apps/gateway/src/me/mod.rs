//! `/api/v1/me/*` — the authenticated caller's own context and profile.

pub mod context;
pub mod profile;

use axum::{routing::{get, patch}, Router};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/context", get(context::get_context))
        .route("/profile", patch(profile::update_profile))
}
