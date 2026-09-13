//! `UploadBody` — a raw-bytes body extractor whose failures are
//! `PublicError`s.
//!
//! Extracting `Bytes` directly is the bug this fixes: when the request
//! exceeds the route's `DefaultBodyLimit`, axum's own `BytesRejection`
//! answers with a **plaintext** `413 Failed to buffer the request body:
//! length limit exceeded`. `.claude/rules/api-conventions.md` says errors are
//! always the `{ "error": { "code", "message" } }` envelope and "never
//! anything else" — a raw framework rejection escaping as plaintext is a
//! contract violation, and a client parsing the envelope can only render it
//! as a generic failure with nothing actionable in it.
//!
//! Rejections are mapped by **status code** rather than by matching
//! `BytesRejection`'s variants: the enum is `#[non_exhaustive]`, so a new
//! variant in a future axum would otherwise silently fall through as
//! plaintext again. Anything that is not a length-limit failure is a
//! malformed or aborted body, which is a client-side problem.

use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
    http::StatusCode,
    response::IntoResponse,
};

use dg_core::PublicError;

use crate::state::AppState;

/// Translate a body-extraction failure into the error envelope.
///
/// Split out from the extractor so it can be tested without an `AppState`
/// (which needs a live Postgres and Redis to construct), and keyed on the
/// status rather than the rejection variant for the `#[non_exhaustive]`
/// reason above.
pub(crate) fn map_rejection(status: StatusCode, max_bytes: usize) -> PublicError {
    if status == StatusCode::PAYLOAD_TOO_LARGE {
        // The message names the real limit, so the console can tell the
        // admin how much too large the file is instead of just that it
        // failed.
        PublicError::payload_too_large(max_bytes)
    } else {
        PublicError::validation(
            "file",
            "The uploaded file could not be read. Please retry the upload.",
        )
    }
}

/// The request body as raw bytes, with the route's configured upload cap
/// enforced and reported inside the error envelope.
pub struct UploadBody(pub Bytes);

impl FromRequest<AppState> for UploadBody {
    type Rejection = PublicError;

    async fn from_request(req: Request, state: &AppState) -> Result<Self, Self::Rejection> {
        match Bytes::from_request(req, state).await {
            Ok(bytes) => Ok(UploadBody(bytes)),
            Err(rejection) => Err(map_rejection(
                rejection.into_response().status(),
                state.config.max_upload_bytes,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, extract::DefaultBodyLimit, http::Request, routing::post, Router};
    use tower::ServiceExt;

    const LIMIT: usize = 64 * 1024 * 1024;

    /// Mirrors `UploadBody` exactly, over `()` instead of `AppState` —
    /// `AppState` needs a live Postgres and Redis, which a unit test should
    /// not require. The behaviour under test is the mapping, which is
    /// shared.
    struct TestBody(#[allow(dead_code)] Bytes);

    impl FromRequest<()> for TestBody {
        type Rejection = PublicError;

        async fn from_request(req: Request<Body>, state: &()) -> Result<Self, Self::Rejection> {
            match Bytes::from_request(req, state).await {
                Ok(bytes) => Ok(TestBody(bytes)),
                Err(rejection) => Err(map_rejection(rejection.into_response().status(), LIMIT)),
            }
        }
    }

    fn app(limit: usize) -> Router {
        Router::new()
            .route("/upload", post(|_: TestBody| async { "ok" }))
            .layer(DefaultBodyLimit::max(limit))
    }

    #[test]
    fn oversize_body_maps_to_the_payload_too_large_envelope() {
        let err = map_rejection(StatusCode::PAYLOAD_TOO_LARGE, LIMIT);
        assert_eq!(err.code(), "PAYLOAD_TOO_LARGE");
        assert!(
            err.public_message().contains("64 MB"),
            "the message must name the actual limit, got: {}",
            err.public_message()
        );
    }

    #[test]
    fn any_other_body_failure_is_a_validation_error() {
        assert_eq!(
            map_rejection(StatusCode::BAD_REQUEST, LIMIT).code(),
            "VALIDATION_ERROR"
        );
    }

    /// The composition that actually broke in production: axum's
    /// `DefaultBodyLimit` fires, and what reaches the client must be the
    /// JSON envelope, not axum's plaintext "Failed to buffer the request
    /// body: length limit exceeded".
    #[tokio::test]
    async fn body_over_the_limit_returns_the_json_envelope_not_plaintext() {
        let response = app(8)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/upload")
                    .body(Body::from(vec![b'x'; 64]))
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("body reads");
        let json: serde_json::Value = serde_json::from_slice(&body)
            .expect("the body must be JSON — a plaintext rejection is the bug this pins");

        assert_eq!(json["error"]["code"], "PAYLOAD_TOO_LARGE");
        assert!(json["error"]["message"].as_str().is_some());
    }

    #[tokio::test]
    async fn body_under_the_limit_is_accepted() {
        let response = app(1024)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/upload")
                    .body(Body::from(vec![b'x'; 512]))
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::OK);
    }
}
