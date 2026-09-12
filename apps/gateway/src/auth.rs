//! `/auth/*` route module. Placeholder wiring only — the next agent drops
//! login/signup/refresh/forgot-password handlers straight in here.

use axum::Router;

pub fn router() -> Router {
    // TODO: login, signup, refresh, forgot-password, reset-password,
    // session list/revoke (see IMPLEMENTATION_PLAN.md §4.2).
    Router::new()
}
