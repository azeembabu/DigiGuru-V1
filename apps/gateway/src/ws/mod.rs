//! `GET /ws/session?token=...` — the Phase 3 realtime classroom socket
//! (`IMPLEMENTATION_PLAN.md` §6.1/§6.2, `.claude/rules/api-conventions.md`
//! "WebSocket", `.claude/rules/whiteboard-sync.md`).
//!
//! This module owns the transport and the per-connection loop that *uses*
//! `dg_live::SyncGate` to enforce NN-1 (whiteboard-first). It does not own
//! the SyncGate state machine, the board-op schema/validator, or the Gemini
//! Live client itself — those live in `crates/live` (owned by the other
//! developer per `CLAUDE.md` "Machine ownership"; this machine only calls
//! into that crate's public interface).
//!
//! Scope of this pass: `session_init`, `board_ack`, `board_error`, and
//! `end_session` are fully wired. `skip_recap` and the real Gemini Live
//! upstream (student mic audio forwarding) are stubbed with `// TODO`
//! markers — see the acceptance note in the PR description. The core
//! deliverable of this pass is demonstrating the whiteboard-first ordering
//! end-to-end against `dg_live::StubLiveSessionClient`'s scripted turn.

use std::collections::HashMap;
use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::UserId;
use dg_live::{
    LiveModelEvent, LiveSessionClient, SyncGate, SyncGateAction, StubLiveSessionClient,
};

use crate::auth::jwt::verify_access_token;
use crate::state::AppState;

/// Close code: token missing/invalid. Per `.claude/rules/api-conventions.md`
/// "Close codes" table.
const CLOSE_UNAUTHENTICATED: u16 = 4001;
// TODO(phase4): 4003 quota reached — needs the quota ledger (crates/quota)
// wired into this loop.
// TODO(phase4): 4008 idle timeout — needs the `idle:{session_id}` watchdog
// (`IMPLEMENTATION_PLAN.md` §3.3, D-22).
// TODO(phase4): 4009 safety termination — needs the Tier-1 guardrail hook
// (`.claude/rules/security.md` "Guardrails (NN-5)").

/// Placeholder quota, since the real ledger (`crates/quota`) is not wired
/// into this pass. `quota:{student_id}:{yyyymmdd}` (§3.3) is the eventual
/// source of truth.
// TODO(phase4): replace with a real read of `quota:{student_id}:{yyyymmdd}`.
const PLACEHOLDER_QUOTA_REMAINING_MS: u64 = 1_200_000;

#[derive(Debug, Deserialize, Default)]
pub struct WsAuthQuery {
    #[serde(default)]
    token: Option<String>,
}

/// Client -> server control frames (`.claude/rules/api-conventions.md`).
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    SessionInit {
        block_id: Uuid,
        #[serde(default)]
        resume: bool,
    },
    BoardAck {
        seq: u32,
    },
    BoardError {
        seq: u32,
        #[serde(default)]
        reason: String,
    },
    SkipRecap,
    EndSession,
}

#[derive(Debug, Serialize)]
struct SessionReadyMsg {
    #[serde(rename = "type")]
    kind: &'static str,
    session_id: Uuid,
    is_first_login: bool,
    context: serde_json::Value,
    quota_remaining_ms: u64,
}

#[derive(Debug, Serialize)]
struct ErrorMsg<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    code: &'a str,
    message: &'a str,
}

/// `GET /ws/session?token=...`. The token is a query param, not the cookie
/// used by REST (`.claude/rules/api-conventions.md`): a browser cannot set a
/// cookie on the WS upgrade handshake the same way it does for `fetch`.
///
/// A missing/invalid token still completes the HTTP->WS upgrade and then
/// closes with code `4001` immediately, rather than rejecting the upgrade
/// with a plain 401 — this matches the documented close-code contract,
/// which is only observable once the socket exists.
pub async fn upgrade_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<WsAuthQuery>,
) -> impl IntoResponse {
    let claims = query
        .token
        .as_deref()
        .and_then(|token| verify_access_token(token, &state.config.jwt_access_secret).ok());

    ws.on_upgrade(move |socket| async move {
        match claims {
            Some(claims) => {
                let user_id = UserId::from(claims.sub);
                handle_socket(socket, state, user_id).await;
            }
            None => close_unauthenticated(socket).await,
        }
    })
}

async fn close_unauthenticated(mut socket: WebSocket) {
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code: CLOSE_UNAUTHENTICATED,
            reason: "unauthenticated".into(),
        })))
        .await;
}

/// Internal event fed back into the main select loop when a spawned hold
/// timer fires. A simple `HashMap<seq, JoinHandle>` plus an mpsc channel is
/// the "keep it simple" pattern called out in the task brief — no
/// `DelayQueue` dependency needed for one timer per pending turn.
enum InternalEvent {
    HoldTimeout(u32),
}

/// The per-connection loop. One tokio task per socket
/// (`.claude/rules/realtime-audio.md` "Gateway"): no `block_in_place`, no
/// blocking calls in the audio path.
async fn handle_socket(mut socket: WebSocket, state: AppState, user_id: UserId) {
    let mut sync_gate = SyncGate::default();
    // Owns the demo scripted turn (BoardOps -> AudioChunk* -> TurnComplete)
    // used to prove the whiteboard-first ordering end-to-end. Real Gemini
    // Live wiring is out of scope for this pass.
    let mut live_client = StubLiveSessionClient::default();

    let mut session_id: Option<Uuid> = None;
    // The turn the model is currently narrating. `AudioChunk` events from
    // the stub don't carry their own `seq` (per the `LiveModelEvent`
    // contract), so it is tracked from the most recent `BoardOps.seq`.
    let mut current_seq: Option<u32> = None;
    // Set once `session_init` has been handled — gates when we start
    // draining the stub's scripted turn, so nothing is sent before the
    // client is ready to receive `session_ready`.
    let mut driving_turn = false;

    let (timer_tx, mut timer_rx) = tokio::sync::mpsc::unbounded_channel::<InternalEvent>();
    let mut hold_timers: HashMap<u32, tokio::task::JoinHandle<()>> = HashMap::new();

    loop {
        tokio::select! {
            biased;

            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientMessage>(text.as_str()) {
                            Ok(ClientMessage::SessionInit { block_id, resume }) => {
                                let sid = init_session(&state, user_id, block_id, resume).await;
                                session_id = Some(sid);
                                let ready = SessionReadyMsg {
                                    kind: "session_ready",
                                    session_id: sid,
                                    // TODO(phase4): real value comes from
                                    // `students.is_first_login` (NN-2); this
                                    // pass has no DB read of the student row
                                    // in the WS path.
                                    is_first_login: false,
                                    // TODO(phase4): real value comes from
                                    // `ctx:{student_id}` / the students +
                                    // programs/semesters/blocks join used by
                                    // `GET /me/context`.
                                    context: serde_json::json!({ "block_id": block_id }),
                                    // TODO(phase4): real value comes from the
                                    // `quota:{student_id}:{yyyymmdd}` ledger
                                    // (`crates/quota`).
                                    quota_remaining_ms: PLACEHOLDER_QUOTA_REMAINING_MS,
                                };
                                if send_json(&mut socket, &ready).await.is_err() {
                                    break;
                                }
                                driving_turn = true;
                            }
                            Ok(ClientMessage::BoardAck { seq }) => {
                                abort_hold_timer(&mut hold_timers, seq);
                                let action = sync_gate.on_board_ack(seq);
                                if apply_action(&mut socket, action).await.is_err() {
                                    break;
                                }
                            }
                            Ok(ClientMessage::BoardError { seq, reason }) => {
                                tracing::warn!(seq, %reason, "client reported board_error; degrading to text-fallback for this turn");
                                abort_hold_timer(&mut hold_timers, seq);
                                let action = sync_gate.on_board_error(seq);
                                if apply_action(&mut socket, action).await.is_err() {
                                    break;
                                }
                            }
                            Ok(ClientMessage::SkipRecap) => {
                                // TODO(phase4): recap generation (and thus
                                // skipping it) is out of scope for this
                                // pass — there is no recap turn to skip yet.
                            }
                            Ok(ClientMessage::EndSession) => {
                                break;
                            }
                            Err(err) => {
                                // Unknown/malformed control frames are
                                // ignored, not fatal, per api-conventions.md
                                // ("forward compatibility").
                                tracing::debug!(%err, "ignoring unrecognized control frame");
                            }
                        }
                    }
                    Some(Ok(Message::Binary(data))) => {
                        // TODO(phase3-gemini-integration): forward student
                        // mic audio to the real Gemini Live upstream via
                        // `live_client.send_audio_frame(...)`. There is no
                        // real upstream in this pass (StubLiveSessionClient
                        // has no inbound audio path) — just acknowledge
                        // receipt so the wiring point is visible.
                        tracing::debug!(bytes = data.len(), "received student audio frame (not forwarded in this pass)");
                    }
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {
                        // axum answers Ping automatically; nothing to do.
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(err)) => {
                        tracing::warn!(%err, "websocket read error, closing session");
                        break;
                    }
                }
            }

            Some(event) = timer_rx.recv() => {
                match event {
                    InternalEvent::HoldTimeout(seq) => {
                        hold_timers.remove(&seq);
                        let action = sync_gate.on_hold_timeout(seq);
                        if apply_action(&mut socket, action).await.is_err() {
                            break;
                        }
                    }
                }
            }

            event = live_client.poll_event(), if driving_turn => {
                match event {
                    Ok(LiveModelEvent::BoardOps(msg)) => {
                        if let Err(err) = dg_live::validate(&msg) {
                            tracing::warn!(?err, "dropping invalid board_ops from model");
                            continue;
                        }
                        let seq = msg.seq;
                        current_seq = Some(seq);
                        let action = sync_gate.on_board_ops(seq);

                        if send_json(&mut socket, &msg).await.is_err() {
                            break;
                        }

                        if let SyncGateAction::StartHoldTimer { seq, hold_ms } = action {
                            let tx = timer_tx.clone();
                            let handle = tokio::spawn(async move {
                                tokio::time::sleep(Duration::from_millis(hold_ms)).await;
                                let _ = tx.send(InternalEvent::HoldTimeout(seq));
                            });
                            hold_timers.insert(seq, handle);
                        }
                    }
                    Ok(LiveModelEvent::AudioChunk(frame)) => {
                        if let Some(seq) = current_seq {
                            let action = sync_gate.buffer_audio(seq, frame);
                            if apply_action(&mut socket, action).await.is_err() {
                                break;
                            }
                        } else {
                            // No board_ops has been seen yet this connection
                            // — NN-1 forbids sending this audio at all.
                            tracing::warn!("dropping audio chunk with no preceding board_ops turn");
                        }
                    }
                    Ok(LiveModelEvent::TurnComplete) => {
                        // Scripted demo turn finished; stop polling until a
                        // real Live client drives further turns.
                        driving_turn = false;
                    }
                    Ok(LiveModelEvent::SessionEnded) => break,
                    Err(err) => {
                        tracing::warn!(?err, "live session client error, closing session");
                        let msg = ErrorMsg {
                            kind: "error",
                            code: "UPSTREAM_UNAVAILABLE",
                            message: "The tutoring session ended unexpectedly.",
                        };
                        let _ = send_json(&mut socket, &msg).await;
                        break;
                    }
                }
            }
        }
    }

    for (_, handle) in hold_timers.drain() {
        handle.abort();
    }
    sync_gate.reset_session();

    // Per IMPLEMENTATION_PLAN.md §6.1/A-8, `sess:{id}` in Redis SURVIVES a
    // disconnect so a reconnect resumes rather than restarts — it is
    // deliberately not deleted here, on `end_session`, or on any socket
    // error. Only the in-memory, task-local state above (SyncGate, timer
    // handles) is torn down with this task.
    //
    // TODO(phase3-reconnect): full resume needs more than the Redis hash
    // surviving. This pass's story is honest but partial: a fresh
    // `session_init { resume: true }` after reconnect can rehydrate
    // `block_id` (and, once wired, other `ctx:`/`sess:` fields) from Redis,
    // but the in-flight `SyncGate` turn state (which seq was PENDING_ACK,
    // any buffered-but-unreleased audio) lives only in this task and is
    // lost when it exits. A genuinely resumable mid-turn requires
    // persisting SyncGate state (or at minimum the last never-acked seq)
    // somewhere durable and replaying it on reconnect — real design work,
    // not a small addition, and explicitly out of scope here. The honest
    // behavior today is: the session continues, but its current turn
    // restarts.
    let _ = session_id;
}

fn abort_hold_timer(timers: &mut HashMap<u32, tokio::task::JoinHandle<()>>, seq: u32) {
    if let Some(handle) = timers.remove(&seq) {
        handle.abort();
    }
}

/// Applies a `SyncGateAction` produced by `on_board_ack` / `on_board_error`
/// / `on_hold_timeout` / `buffer_audio`. `StartHoldTimer` is only ever
/// returned from `on_board_ops` in practice and is handled at that call
/// site, not here.
async fn apply_action(socket: &mut WebSocket, action: SyncGateAction) -> Result<(), axum::Error> {
    match action {
        SyncGateAction::ForwardImmediately(frame) => {
            socket.send(Message::Binary(frame)).await?;
        }
        SyncGateAction::ReleaseBuffered(frames) => {
            for frame in frames {
                socket.send(Message::Binary(frame)).await?;
            }
        }
        SyncGateAction::ReleaseBufferedWithViolation(frames) => {
            // NN-1 / whiteboard-sync.md invariant: `wb_violation` must be 0
            // in CI end-to-end runs. A real counter/metric is out of scope
            // for this pass; this log line is the minimum required signal.
            tracing::warn!(count = frames.len(), "wb_violation: releasing buffered audio without an ack");
            for frame in frames {
                socket.send(Message::Binary(frame)).await?;
            }
        }
        SyncGateAction::Buffered | SyncGateAction::NoOp => {}
        SyncGateAction::StartHoldTimer { .. } => {
            tracing::debug!("unexpected StartHoldTimer outside on_board_ops handling");
        }
    }
    Ok(())
}

async fn send_json<T: Serialize>(socket: &mut WebSocket, value: &T) -> Result<(), axum::Error> {
    match serde_json::to_string(value) {
        Ok(text) => socket.send(Message::Text(text.into())).await,
        Err(err) => {
            tracing::error!(%err, "failed to serialize outbound WS control frame");
            Ok(())
        }
    }
}

/// `session_init` handling for this pass: persist minimal session state to
/// `sess:{session_id}` (`IMPLEMENTATION_PLAN.md` §3.3), generating a new
/// `session_id` unless `resume: true` finds an existing one to reuse via
/// `ctx:{student_id}`. Best-effort — a resume that finds nothing just starts
/// fresh (logged), it never errors the connection.
async fn init_session(state: &AppState, user_id: UserId, block_id: Uuid, resume: bool) -> Uuid {
    use redis::AsyncCommands;

    let mut redis = state.redis.clone();
    let ctx_key = format!("ctx:{}", user_id);

    let existing_session_id: Option<String> = if resume {
        match redis.hget::<_, _, Option<String>>(&ctx_key, "session_id").await {
            Ok(Some(sid)) => {
                let sess_key = format!("sess:{sid}");
                match redis.exists::<_, bool>(&sess_key).await {
                    Ok(true) => Some(sid),
                    Ok(false) => {
                        tracing::info!(%user_id, "resume requested but sess:{} expired; starting fresh", sid);
                        None
                    }
                    Err(err) => {
                        tracing::warn!(%err, "redis error checking resumable session; starting fresh");
                        None
                    }
                }
            }
            Ok(None) => {
                tracing::info!(%user_id, "resume requested but no prior session found; starting fresh");
                None
            }
            Err(err) => {
                tracing::warn!(%err, "redis error reading ctx: for resume; starting fresh");
                None
            }
        }
    } else {
        None
    };

    let session_id = match existing_session_id.and_then(|s| Uuid::parse_str(&s).ok()) {
        Some(id) => id,
        None => Uuid::new_v4(),
    };

    let sess_key = format!("sess:{session_id}");
    let created_at = chrono::Utc::now().to_rfc3339();
    let fields: [(&str, String); 3] = [
        ("user_id", user_id.to_string()),
        ("block_id", block_id.to_string()),
        ("created_at", created_at),
    ];

    if let Err(err) = redis.hset_multiple::<_, _, _, ()>(&sess_key, &fields).await {
        tracing::error!(%err, "failed to write sess:{} to redis", session_id);
    }
    // 6h TTL per §3.3's `sess:{session_id}` row.
    if let Err(err) = redis.expire::<_, ()>(&sess_key, 6 * 60 * 60).await {
        tracing::error!(%err, "failed to set TTL on sess:{}", session_id);
    }

    // Remember the session for future `resume: true` lookups, alongside
    // the "Remember Me" context (§3.3 `ctx:{student_id}`, 30 d TTL).
    if let Err(err) = redis
        .hset::<_, _, _, ()>(&ctx_key, "session_id", session_id.to_string())
        .await
    {
        tracing::error!(%err, "failed to update ctx:{} with session_id", user_id);
    }
    if let Err(err) = redis.expire::<_, ()>(&ctx_key, 30 * 24 * 60 * 60).await {
        tracing::error!(%err, "failed to set TTL on ctx:{}", user_id);
    }

    session_id
}
