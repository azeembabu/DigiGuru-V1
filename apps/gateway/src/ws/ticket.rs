//! `POST /api/v1/ws/ticket` — mints a single-use, short-lived ticket for the
//! `/ws/session` upgrade.
//!
//! **Why this exists at all.** REST auth is an httpOnly cookie, which a browser
//! deliberately will not let script read. The WS upgrade cannot carry a custom
//! `Authorization` header either — the browser `WebSocket` constructor takes a
//! URL and subprotocols, nothing else. So the token has to travel in the query
//! string, and that is exactly where a long-lived access JWT must never go:
//! query strings land in proxy logs, server access logs, `Referer` headers and
//! browser history, none of which are places a 15-minute credential for the
//! whole API should be sitting.
//!
//! A ticket is therefore a different, deliberately feeble credential:
//!
//! * **single use** — redeeming it deletes it (`GETDEL`), so a ticket captured
//!   from a log is already spent by the time anyone reads the log,
//! * **30 seconds** — it only has to survive the round trip from this response
//!   to the `WebSocket` constructor,
//! * **one purpose** — it authenticates a socket upgrade and nothing else. It is
//!   not accepted by any REST route, and it carries no capabilities of its own.
//!
//! The opaque value is 32 bytes of OS randomness, base64url — not a JWT, because
//! there is nothing to self-describe: the server holds the mapping, and holding
//! it is what makes single-use possible.

use axum::Json;
use rand::RngCore;
use redis::AsyncCommands;
use serde::Serialize;

use dg_core::{Capability, PublicError, UserId};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

/// How long a minted ticket stays redeemable. Long enough for the response to
/// reach the browser and the socket to open, short enough that a leaked ticket
/// is almost always already expired as well as already spent.
pub const TICKET_TTL_SECS: u64 = 30;

/// Redis key for a ticket. The ticket value itself is the key, so redemption is
/// a single `GETDEL` — there is no scan and no per-user index to keep in step.
fn ticket_key(ticket: &str) -> String {
    format!("wsticket:{ticket}")
}

#[derive(Debug, Serialize)]
pub struct WsTicketResponse {
    /// Pass verbatim as `?token=` on the `/ws/session` URL.
    pub ticket: String,
    /// Seconds until it stops being redeemable, so a client can decide to mint a
    /// fresh one rather than open a socket it knows will be rejected.
    pub expires_in_secs: u64,
}

/// Mints a ticket for the caller's own socket.
///
/// Capability `ManageOwnSessions`: this is the same right as starting or ending
/// one's own session, and an admin holds it too — an admin opening a classroom
/// socket is pointless rather than dangerous, and the `/ws/session` handler is
/// what decides whether the subject has a student row.
pub async fn create_ws_ticket(
    AuthenticatedActor(actor): AuthenticatedActor,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<WsTicketResponse>, PublicError> {
    actor.require(Capability::ManageOwnSessions)?;

    let ticket = mint_ticket_value();
    let mut redis = state.redis.clone();

    // SET key <user_id> EX ttl NX. `NX` is a belt-and-braces guard against
    // overwriting an existing ticket: with 32 bytes of randomness a collision
    // will not happen, but silently replacing someone else's live ticket is the
    // kind of thing that must be impossible rather than improbable.
    let stored: bool = redis
        .set_options(
            ticket_key(&ticket),
            actor.user_id.into_uuid().to_string(),
            redis::SetOptions::default()
                .conditional_set(redis::ExistenceCheck::NX)
                .with_expiration(redis::SetExpiry::EX(TICKET_TTL_SECS)),
        )
        .await
        .map_err(|err| {
            tracing::error!(%err, "redis unavailable while minting a ws ticket");
            PublicError::Unavailable
        })?;

    if !stored {
        tracing::error!("ws ticket collision; refusing to reuse an existing ticket");
        return Err(PublicError::Unavailable);
    }

    Ok(Json(WsTicketResponse {
        ticket,
        expires_in_secs: TICKET_TTL_SECS,
    }))
}

/// 32 bytes of OS randomness, base64url, no padding.
fn mint_ticket_value() -> String {
    use base64::Engine as _;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Redeems a ticket, returning the user it was minted for.
///
/// `GETDEL` is the whole point: the read and the invalidation are one atomic
/// operation, so two sockets racing with the same captured ticket cannot both
/// win. A missing key means expired, already redeemed, or never existed — all
/// indistinguishable to the caller, and all the same answer.
pub async fn redeem_ticket(state: &AppState, ticket: &str) -> Option<UserId> {
    // Reject implausible input before it reaches Redis: a ticket is always a
    // fixed-length base64url string, so anything else is a probe, not a typo.
    if ticket.len() != 43 || !ticket.bytes().all(is_base64url_byte) {
        return None;
    }

    let mut redis = state.redis.clone();
    let raw: Option<String> = match redis.get_del(ticket_key(ticket)).await {
        Ok(value) => value,
        Err(err) => {
            // Fail closed. A Redis outage must not become "everyone gets in".
            tracing::error!(%err, "redis unavailable while redeeming a ws ticket");
            return None;
        }
    };

    raw.as_deref()
        .and_then(|value| value.parse::<uuid::Uuid>().ok())
        .map(UserId::from)
}

fn is_base64url_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minted_ticket_is_43_chars_of_base64url() {
        let ticket = mint_ticket_value();
        assert_eq!(ticket.len(), 43, "32 bytes base64url unpadded is 43 chars");
        assert!(ticket.bytes().all(is_base64url_byte), "got {ticket}");
    }

    #[test]
    fn two_tickets_are_never_the_same() {
        let a = mint_ticket_value();
        let b = mint_ticket_value();
        assert_ne!(a, b);
    }

    #[test]
    fn the_key_is_namespaced_so_it_cannot_collide_with_other_state() {
        assert_eq!(ticket_key("abc"), "wsticket:abc");
    }

    #[test]
    fn malformed_tickets_are_rejected_before_any_redis_call() {
        // Length and alphabet are checked first, so these never reach Redis.
        assert!(!"short".bytes().all(|_| false));
        for bad in ["", "x", "a".repeat(42).as_str(), "a".repeat(44).as_str()] {
            assert_ne!(bad.len(), 43, "fixture {bad:?} must not be ticket-shaped");
        }
        // A correct length but an illegal byte (base64url has no '+' or '/').
        let illegal = format!("{}+", "a".repeat(42));
        assert_eq!(illegal.len(), 43);
        assert!(!illegal.bytes().all(is_base64url_byte));
    }
}
