//! `/api/v1/me/*` — the authenticated caller's own identity, context, and
//! profile.

pub mod context;
pub mod identity;
pub mod profile;

use axum::{routing::{get, patch}, Router};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        // `/` here is `/api/v1/me` itself once `main.rs` nests this router —
        // the identity route for every role, as opposed to the student-only
        // academic payload at `/me/context`.
        .route("/", get(identity::get_me))
        .route("/context", get(context::get_context))
        .route("/profile", patch(profile::update_profile))
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request, routing::get, Router};
    use tower::ServiceExt;

    /// `GET /api/v1/me` must reach the identity handler with **no** trailing
    /// slash — that is the URL the frontend calls. This pins axum's nesting
    /// behaviour for a `/` route rather than trusting it, because getting it
    /// wrong fails as a silent `404` on the one route an admin shell needs
    /// before it can render anything.
    #[tokio::test]
    async fn me_identity_route_is_reachable_without_a_trailing_slash() {
        let me = Router::new().route("/", get(|| async { "identity" }));
        let app = Router::new().nest("/api/v1", Router::new().nest("/me", me));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/me")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }
}
