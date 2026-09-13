//! `JsonBody<T>` — a JSON request-body extractor whose failures are
//! `PublicError`s.
//!
//! Extracting `axum::Json<T>` directly is the bug this fixes. When the body
//! does not deserialise, axum answers with its own rejection:
//!
//! ```text
//! 422 Failed to deserialize the JSON body into the target type: missing field `course_id` at line 1 column 11
//! ```
//!
//! That is a plaintext body, not the `{ "error": { "code", "message" } }`
//! envelope `.claude/rules/api-conventions.md` says errors are "always ...
//! never anything else", so an envelope-parsing client can only render it as a
//! generic failure with nothing actionable in it. It is also a disclosure: the
//! text names the internal request struct's field and the byte position, and the
//! `/auth/*` routes are **unauthenticated**, so anyone could enumerate the
//! shape of the login and signup payloads from the outside.
//!
//! Rejections are mapped by **status code** rather than by matching
//! `JsonRejection`'s variants, for the same reason `upload_body` does it:
//! the enum is `#[non_exhaustive]`, so a new variant in a future axum would
//! otherwise fall through as plaintext again.
//!
//! This is a second mapper rather than a reuse of
//! [`crate::extractors::upload_body::map_rejection`] because that one's
//! messages are about an uploaded *file* and its 413 names a per-route upload
//! limit in megabytes — wording that would be wrong, and a limit that would be
//! a lie, on `POST /auth/login`. The *shape* is shared: match on the status,
//! never on the variant.
//!
//! `Json` is still the right type for a **response**; this only replaces it in
//! the argument position.

use axum::{
    extract::{FromRequest, Request},
    http::StatusCode,
    response::IntoResponse,
};
use serde::de::DeserializeOwned;

use dg_core::PublicError;

/// Translate a JSON body rejection into the error envelope.
///
/// Keyed on the status, and deliberately vague in the message: the point is to
/// say the body was unacceptable without echoing axum's text, the request
/// struct's field names, or a line/column position.
///
/// Both `400` (syntactically invalid JSON) and `422` (valid JSON that does not
/// fit the target type, including a missing field) become
/// `400 VALIDATION_ERROR` on field `body`. That is the status the rest of this
/// API already uses for every field error — the `422` in the status table is for
/// a request that parsed and was *semantically* rejected by a handler, such as
/// `EXAM_NOT_MCQ`, not for one that never deserialised at all.
pub(crate) fn map_rejection(status: StatusCode) -> PublicError {
    match status {
        StatusCode::PAYLOAD_TOO_LARGE => PublicError::PayloadTooLarge {
            message: "The request body is larger than this endpoint accepts.".to_string(),
        },
        StatusCode::UNSUPPORTED_MEDIA_TYPE => PublicError::validation(
            "body",
            "the request body must be JSON, sent with Content-Type: application/json",
        ),
        // Everything else is a body this endpoint cannot accept: malformed JSON,
        // a missing or mistyped field, or an aborted read.
        _ => PublicError::validation("body", "the request body is not valid for this endpoint"),
    }
}

/// A JSON request body, with extraction failures reported inside the error
/// envelope.
///
/// Use in the argument position everywhere `axum::Json<T>` was used to *read* a
/// body:
///
/// ```ignore
/// pub async fn create(JsonBody(payload): JsonBody<CreateThingRequest>) -> ...
/// ```
pub struct JsonBody<T>(pub T);

impl<T, S> FromRequest<S> for JsonBody<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = PublicError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(value)) => Ok(JsonBody(value)),
            Err(rejection) => Err(map_rejection(rejection.into_response().status())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::post, Json, Router};
    use serde::Deserialize;
    use tower::ServiceExt;

    /// Stands in for a real request struct. `secret_internal_field` exists to be
    /// looked for in the response: if it ever appears, the disclosure this
    /// extractor exists to stop is back.
    #[derive(Debug, Deserialize)]
    struct Payload {
        secret_internal_field: String,
    }

    /// Mirrors an admin create route and an `/auth/*` route closely enough to
    /// exercise the extractor: both take a JSON body and return the envelope on
    /// failure. `()` rather than `AppState`, which would need a live Postgres
    /// and Redis — the behaviour under test is the mapping, which is shared.
    fn app() -> Router {
        Router::new()
            .route(
                "/api/v1/admin/question-pool",
                post(|JsonBody(p): JsonBody<Payload>| async move {
                    Json(serde_json::json!({ "ok": p.secret_internal_field }))
                }),
            )
            .route(
                "/api/v1/auth/login",
                post(|JsonBody(p): JsonBody<Payload>| async move {
                    Json(serde_json::json!({ "ok": p.secret_internal_field }))
                }),
            )
    }

    async fn post_body(uri: &str, body: &'static str) -> (StatusCode, serde_json::Value) {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("body reads");
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_else(|_| {
            panic!(
                "the body must be the JSON envelope, got: {}",
                String::from_utf8_lossy(&bytes)
            )
        });
        (status, json)
    }

    /// Assert the envelope, and that nothing axum wrote leaked into it.
    fn assert_envelope_without_disclosure(json: &serde_json::Value) {
        assert_eq!(json["error"]["code"], "VALIDATION_ERROR");
        let message = json["error"]["message"]
            .as_str()
            .expect("message is a string");

        assert!(
            !message.contains("Failed to deserialize"),
            "axum's own text must not reach the client: {message}"
        );
        assert!(
            !message.contains("secret_internal_field"),
            "the request struct's field names must not be disclosed: {message}"
        );
        assert!(
            !message.contains("line") && !message.contains("column"),
            "a byte position tells a caller nothing and probes the parser: {message}"
        );
        // No details key, no stack trace, nothing but the two documented fields.
        assert!(json["error"].get("details").is_none());
        assert_eq!(
            json["error"].as_object().map(|o| o.len()),
            Some(2),
            "the envelope is exactly code + message"
        );
    }

    /// The live observation from verification: a bogus body on an admin route
    /// returned axum's plaintext. It must now be the envelope.
    #[tokio::test]
    async fn a_bogus_body_on_an_admin_route_returns_the_envelope() {
        let (status, json) = post_body("/api/v1/admin/question-pool", r#"{"bogus":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_envelope_without_disclosure(&json);
    }

    /// The one that matters most: `/auth/*` is unauthenticated, so axum's
    /// rejection disclosed the login struct's field names to anyone at all.
    #[tokio::test]
    async fn a_bogus_body_on_an_unauthenticated_auth_route_discloses_nothing() {
        let (status, json) = post_body("/api/v1/auth/login", r#"{"bogus":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_envelope_without_disclosure(&json);
    }

    /// Syntactically broken JSON takes the same path as a missing field: one
    /// `code` for "this body is not acceptable", so a client has one branch.
    #[tokio::test]
    async fn malformed_json_returns_the_same_stable_code() {
        let (status, json) = post_body("/api/v1/auth/login", "{not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_envelope_without_disclosure(&json);
    }

    #[tokio::test]
    async fn a_valid_body_is_still_extracted() {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/admin/question-pool")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"secret_internal_field":"value"}"#))
                    .expect("request builds"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::OK);
    }

    // -- The mapping, unit level ------------------------------------------

    #[test]
    fn a_missing_content_type_is_a_field_error_naming_the_requirement() {
        let err = map_rejection(StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(err.code(), "VALIDATION_ERROR");
        assert!(err.public_message().contains("application/json"));
    }

    #[test]
    fn an_oversize_json_body_is_payload_too_large_without_naming_a_wrong_limit() {
        let err = map_rejection(StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(err.code(), "PAYLOAD_TOO_LARGE");
        // Deliberately no megabyte figure: the per-route upload cap is not this
        // endpoint's limit, and quoting it would tell the caller to cut the
        // wrong amount.
        assert!(!err.public_message().contains("MB"));
    }

    /// Mapped by status, not by variant, so an axum version that introduces a
    /// new `JsonRejection` variant still produces the envelope.
    #[test]
    fn an_unrecognised_rejection_status_still_maps_to_the_envelope() {
        for status in [
            StatusCode::BAD_REQUEST,
            StatusCode::UNPROCESSABLE_ENTITY,
            StatusCode::IM_A_TEAPOT,
        ] {
            assert_eq!(map_rejection(status).code(), "VALIDATION_ERROR");
        }
    }
}
