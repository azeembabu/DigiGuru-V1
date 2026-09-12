//! The `ClientIp` extractor: the peer address of the current request, when
//! one is available.
//!
//! `main` serves with `into_make_service_with_connect_info::<SocketAddr>()`,
//! which puts a `ConnectInfo<SocketAddr>` into the request extensions. Reading
//! it back as `Option<ConnectInfo<SocketAddr>>` in a handler signature does
//! not compile on axum 0.8: the blanket `Option<T>` extractor impl was
//! removed, and `Option<T>` now requires `T: OptionalFromRequestParts`, which
//! `ConnectInfo` does not implement.
//!
//! So the optionality lives here instead. This extractor is infallible — a
//! request with no peer address (anything driven through `oneshot` in a test,
//! or a transport that has none) yields `ClientIp(None)` rather than a
//! rejection. That matters: the IP is only ever recorded alongside an audit or
//! session row, and failing a login because the socket address was unavailable
//! would be a far worse outcome than storing `NULL`.

use std::net::{IpAddr, SocketAddr};

use axum::{extract::ConnectInfo, extract::FromRequestParts, http::request::Parts};

/// The caller's IP, or `None` when the transport does not expose one.
pub struct ClientIp(pub Option<IpAddr>);

impl<S> FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    // Infallible: there is no failure mode worth rejecting a request over.
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(ClientIp(
            parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ConnectInfo(addr)| addr.ip()),
        ))
    }
}
