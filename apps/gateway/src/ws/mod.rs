//! `GET /ws/session?token=...` — the Phase 3 realtime classroom socket
//! (`IMPLEMENTATION_PLAN.md` §6.1/§6.2, `.claude/rules/api-conventions.md`
//! "WebSocket", `.claude/rules/whiteboard-sync.md`).
//!
//! This module owns the transport and the per-connection loop that *uses*
//! `live::SyncGate` to enforce NN-1 (whiteboard-first). It does not own
//! the SyncGate state machine, the board-op schema/validator, or the Gemini
//! Live client itself — those live in `crates/live`.
//!
//! NN-1 in this file, concretely: **every** outbound audio frame goes through
//! [`live::SyncGate::push_audio`] and is written to the socket only on
//! `AudioDisposition::Forward`. There is no bypass and no fast path. Held
//! audio leaves only through a release edge — `board_ack`, `board_error`, or
//! the 400 ms hold ceiling surfaced by `poll_timeouts`.
//!
//! Scope of this pass: `session_init`, `board_ack`, `board_error`, and
//! `end_session` are fully wired. `skip_recap` and the real Gemini Live
//! upstream (student mic audio forwarding) are stubbed with `// TODO`
//! markers. The core deliverable is demonstrating the whiteboard-first
//! ordering end-to-end against `live::StubLiveSessionClient`'s scripted
//! turn.

use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::UserId;
use live::{
    AckOutcome, AudioDisposition, BoardOp, Clock, DrawShape, LiveModelEvent, LiveSessionClient,
    Point, ReleasedAudio, StubLiveSessionClient, SyncGate, SystemClock, TurnSeq,
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

/// Server -> client `board_ops`. Built from `live::BoardOpsAccepted`, so
/// what goes on the wire is exactly the set of ops the gate validated — never
/// the raw model output.
#[derive(Debug, Serialize)]
struct BoardOpsMsg<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    seq: u32,
    clear_first: bool,
    ops: &'a [BoardOp],
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

/// The per-connection loop. One tokio task per socket
/// (`.claude/rules/realtime-audio.md` "Gateway"): no `block_in_place`, no
/// blocking calls in the audio path.
async fn handle_socket(mut socket: WebSocket, state: AppState, user_id: UserId) {
    // The gate reads time through this same clock, so the socket's sleep and
    // the gate's deadlines share one monotonic epoch.
    let clock = SystemClock::new();
    let mut sync_gate = SyncGate::new(clock.clone());

    // Owns the demo scripted turn (BoardOps -> AudioChunk* -> TurnComplete)
    // used to prove the whiteboard-first ordering end-to-end. Real Gemini
    // Live wiring is out of scope for this pass.
    let mut live_client = StubLiveSessionClient::new(demo_script());

    let mut session_id: Option<Uuid> = None;
    // The turn the model is currently narrating. `AudioChunk` events from
    // the stub don't carry their own `seq` (per the `LiveModelEvent`
    // contract), so it is tracked from the most recent accepted `board_ops`.
    let mut current_seq: Option<TurnSeq> = None;
    // Set once `session_init` has been handled — gates when we start
    // draining the stub's scripted turn, so nothing is sent before the
    // client is ready to receive `session_ready`.
    let mut driving_turn = false;

    loop {
        // NN-1 hold timer: sleep exactly until the earliest outstanding hold
        // deadline the gate reports, never on a fixed tick. `None` means
        // nothing is holding, so this branch simply never fires.
        let hold_deadline = sync_gate.next_hold_deadline_ms();
        let hold_timer = async {
            match hold_deadline {
                Some(deadline_ms) => {
                    let remaining = deadline_ms.saturating_sub(clock.now_ms());
                    tokio::time::sleep(Duration::from_millis(remaining)).await;
                }
                None => std::future::pending::<()>().await,
            }
        };
        tokio::pin!(hold_timer);

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
                                let outcome = sync_gate.on_board_ack(TurnSeq(seq));
                                if flush_outcome(&mut socket, &mut sync_gate, outcome).await.is_err() {
                                    break;
                                }
                            }
                            Ok(ClientMessage::BoardError { seq, reason }) => {
                                // The gate logs the degrade-to-text-fallback
                                // decision and counts it; teaching continues.
                                let outcome = sync_gate.on_board_error(TurnSeq(seq), &reason);
                                if flush_outcome(&mut socket, &mut sync_gate, outcome).await.is_err() {
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

            () = &mut hold_timer => {
                // HOLD_MAX reached for at least one turn. The gate counts the
                // wb_violation and logs the seq; the socket's job is only to
                // get the released audio out, in order.
                let released = sync_gate.poll_timeouts();
                let mut send_failed = false;
                for audio in released {
                    if flush_released(&mut socket, &mut sync_gate, audio).await.is_err() {
                        send_failed = true;
                        break;
                    }
                }
                if send_failed {
                    break;
                }
            }

            event = live_client.poll_event(), if driving_turn => {
                match event {
                    Ok(LiveModelEvent::BoardOps(msg)) => {
                        let seq = TurnSeq(msg.seq);
                        let ops = to_gate_ops(&msg);
                        let accepted = match sync_gate.on_board_ops(seq, msg.clear_first, ops) {
                            Ok(accepted) => accepted,
                            Err(err) => {
                                // Not fatal: no turn was opened, so no audio
                                // is stranded behind a board that never went
                                // out. The gate already logged the detail.
                                tracing::warn!(%err, "board_ops rejected by SyncGate; nothing forwarded");
                                continue;
                            }
                        };
                        current_seq = Some(accepted.seq);

                        // Only the validated ops go on the wire.
                        let out = BoardOpsMsg {
                            kind: "board_ops",
                            seq: accepted.seq.get(),
                            clear_first: accepted.clear_first,
                            ops: &accepted.ops,
                        };
                        if send_json(&mut socket, &out).await.is_err() {
                            break;
                        }
                        // The hold timer for this turn is picked up on the
                        // next loop iteration via `next_hold_deadline_ms()`.
                    }
                    Ok(LiveModelEvent::AudioChunk(frame)) => {
                        let Some(seq) = current_seq else {
                            // No board_ops has been accepted yet on this
                            // connection — NN-1 forbids sending this audio.
                            tracing::warn!("dropping audio chunk with no preceding board_ops turn");
                            continue;
                        };
                        match sync_gate.push_audio(seq, &frame) {
                            AudioDisposition::Forward => {
                                if socket.send(Message::Binary(frame)).await.is_err() {
                                    break;
                                }
                            }
                            AudioDisposition::Buffered { .. }
                            | AudioDisposition::BufferedEvictingOldest { .. } => {
                                // Held behind the board. Released later by an
                                // ack, a board_error, or the hold ceiling.
                            }
                            AudioDisposition::UnknownTurn => {
                                tracing::warn!(seq = %seq, "audio chunk for unknown/retired turn; dropped");
                            }
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

    let metrics = sync_gate.metrics();
    if metrics.wb_violation > 0 {
        // `whiteboard-sync.md`: wb_violation must be 0 in CI E2E runs.
        tracing::error!(
            wb_violation = metrics.wb_violation,
            ops_dropped = metrics.ops_dropped,
            frames_evicted = metrics.frames_evicted,
            "session closed with NN-1 violations"
        );
    } else {
        tracing::debug!(?metrics, "session closed");
    }

    // Per IMPLEMENTATION_PLAN.md §6.1/A-8, `sess:{id}` in Redis SURVIVES a
    // disconnect so a reconnect resumes rather than restarts — it is
    // deliberately not deleted here, on `end_session`, or on any socket
    // error. Only the in-memory, task-local state above (the SyncGate) is
    // torn down with this task.
    //
    // TODO(phase3-reconnect): full resume needs more than the Redis hash
    // surviving. A fresh `session_init { resume: true }` after reconnect can
    // rehydrate `block_id` from Redis, but the in-flight `SyncGate` turn
    // state (which seq was PENDING_ACK, any buffered-but-unreleased audio)
    // lives only in this task and is lost when it exits.
    let _ = session_id;
}

/// Writes a release's frames out in arrival order, then hands the buffers
/// back to the gate's pool. This is the only path by which held audio reaches
/// the client.
async fn flush_released(
    socket: &mut WebSocket,
    sync_gate: &mut SyncGate<SystemClock>,
    audio: ReleasedAudio,
) -> Result<(), axum::Error> {
    let ReleasedAudio {
        seq,
        reason,
        held_ms,
        frames,
    } = audio;
    tracing::debug!(seq = %seq, ?reason, held_ms, frames = frames.len(), "releasing held audio");

    let mut result = Ok(());
    for frame in &frames {
        if let Err(err) = socket
            .send(Message::Binary(bytes::Bytes::copy_from_slice(frame)))
            .await
        {
            result = Err(err);
            break;
        }
    }
    // Return the buffers to the gate's pool so a warm session stops
    // allocating per held frame (`realtime-audio.md`, "Gateway").
    sync_gate.recycle(frames);
    result
}

/// Applies the outcome of a `board_ack` / `board_error`. Both non-release
/// outcomes are ignored, not fatal, per the forward-compatibility rule.
async fn flush_outcome(
    socket: &mut WebSocket,
    sync_gate: &mut SyncGate<SystemClock>,
    outcome: AckOutcome,
) -> Result<(), axum::Error> {
    match outcome {
        AckOutcome::Released(audio) => flush_released(socket, sync_gate, *audio).await,
        AckOutcome::AlreadyReleased(seq) => {
            tracing::debug!(seq = %seq, "turn already released; ignoring");
            Ok(())
        }
        AckOutcome::UnknownTurn(seq) => {
            tracing::debug!(seq = %seq, "control frame for unknown turn; ignoring");
            Ok(())
        }
    }
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

/// Bridge from the Gemini client's legacy `board::BoardOp` (which carries no
/// element ids) to the `ops::BoardOp` the `SyncGate` validates.
///
/// Ids are synthesised as `s{seq}-op{index}`: stable within a turn and unique
/// across a session, because `seq` is monotonic — so a later `highlight` can
/// still target an element emitted earlier. An op the legacy schema can
/// express but the validated one cannot (an unknown `draw` shape) is dropped
/// here rather than mistranslated; everything else that fails the schema is
/// dropped and logged by the gate.
fn to_gate_ops(msg: &live::board::BoardOpsMessage) -> Vec<BoardOp> {
    msg.ops
        .iter()
        .enumerate()
        .filter_map(|(index, op)| {
            let id = format!("s{}-op{}", msg.seq, index);
            match op {
                live::board::BoardOp::Heading { text, .. } => Some(BoardOp::Heading {
                    id,
                    text: text.clone(),
                }),
                live::board::BoardOp::Bullets { items } => Some(BoardOp::Bullets {
                    id,
                    items: items.clone(),
                }),
                live::board::BoardOp::Math { latex } => Some(BoardOp::Math {
                    id,
                    latex: latex.clone(),
                }),
                live::board::BoardOp::Draw { shape, from, to } => {
                    let shape = match shape.as_str() {
                        "line" => DrawShape::Line,
                        "arrow" => DrawShape::Arrow,
                        "rect" => DrawShape::Rect,
                        "ellipse" => DrawShape::Ellipse,
                        "polyline" => DrawShape::Polyline,
                        other => {
                            tracing::warn!(shape = other, "unknown draw shape from model; op dropped");
                            return None;
                        }
                    };
                    Some(BoardOp::Draw {
                        id,
                        shape,
                        points: vec![
                            Point {
                                x: from[0],
                                y: from[1],
                            },
                            Point { x: to[0], y: to[1] },
                        ],
                    })
                }
                live::board::BoardOp::Image { image_ref } => Some(BoardOp::Image {
                    id,
                    reference: image_ref.clone(),
                }),
                live::board::BoardOp::Highlight { target } => Some(BoardOp::Highlight {
                    target: target.clone(),
                }),
            }
        })
        .collect()
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

/// A hardcoded scripted turn standing in for what a real Gemini Live session
/// would emit — the fixture that lets `StubLiveSessionClient` demonstrate the
/// whiteboard-first (NN-1) ordering end-to-end without a real Gemini
/// connection (`TODO(phase3-gemini-api)` in `crates/live/src/gemini_client.rs`).
fn demo_script() -> Vec<LiveModelEvent> {
    let board_ops = live::board::BoardOpsMessage {
        msg_type: "board_ops".to_string(),
        seq: 1,
        clear_first: false,
        ops: vec![
            live::board::BoardOp::Heading {
                text: "Demo lesson".to_string(),
                page: 1,
            },
            live::board::BoardOp::Bullets {
                items: vec!["This is a scripted Phase 3 demo turn.".to_string()],
            },
        ],
    };

    vec![
        LiveModelEvent::BoardOps(board_ops),
        LiveModelEvent::AudioChunk(bytes::Bytes::from_static(b"demo-audio-frame-1")),
        LiveModelEvent::AudioChunk(bytes::Bytes::from_static(b"demo-audio-frame-2")),
        LiveModelEvent::TurnComplete,
        LiveModelEvent::SessionEnded,
    ]
}
