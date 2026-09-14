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
//! Also enforced here, because each needs data only this loop holds:
//!
//! * **NN-3** — the daily active-voice ledger (`ws::voice_meter`, over
//!   `crates/quota`). Ticked once a second while the socket is up; at zero the
//!   session ends with `end_reason = 'quota'` and close code `4003`.
//! * **NN-4** — the tutor's system instruction is built from retrieved
//!   curriculum only (`ws::grounding`), with the mandatory four-dimension
//!   Qdrant filter and an explicit abstention instruction when retrieval
//!   returns `Abstain`.
//! * **NN-2** — `is_first_login` comes from the `students` row.
//!
//! The upstream model client is chosen at runtime: the real
//! `live::GeminiLiveSessionClient` when a `GEMINI_API_KEY` is configured,
//! otherwise `live::StubLiveSessionClient` driving [`demo_script`]. The stub
//! path is not a leftover — it is the offline dev loop and what the NN-1
//! tests exercise, so it must keep working.

use std::sync::Once;
use std::time::{Duration, Instant};

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::UserId;
use live::{
    AckOutcome, AudioDisposition, BoardOp, Clock, DrawShape, LiveModelEvent, LiveSessionClient,
    Point, ReleasedAudio, StubLiveSessionClient, SyncGate, SystemClock, TranscriptSource, TurnSeq,
};

use crate::auth::jwt::verify_access_token;
use crate::state::AppState;

pub mod board_audit;
pub mod engagement;
pub mod grounding;
pub mod ticket;
pub mod voice_meter;

/// Close code: token missing/invalid. Per `.claude/rules/api-conventions.md`
/// "Close codes" table.
const CLOSE_UNAUTHENTICATED: u16 = 4001;
/// Close code: the daily active-voice allowance is spent (NN-3). Re-exported
/// from `crates/quota` rather than restated, so the socket and the ledger can
/// never disagree about which code means "quota".
const CLOSE_QUOTA: u16 = quota::CLOSE_CODE_QUOTA;
/// Close code: the session ended normally. Not in the "Close codes" table
/// because it is the RFC 6455 normal closure rather than an application
/// decision, but it is load-bearing: it is the only thing distinguishing "this
/// lesson is over" from "the connection dropped", and the client retries the
/// latter.
const CLOSE_NORMAL: u16 = 1000;
// TODO(phase4): 4008 idle timeout — needs the `idle:{session_id}` watchdog
// (`IMPLEMENTATION_PLAN.md` §3.3, D-22).
// TODO(phase4): 4009 safety termination — needs the Tier-1 guardrail hook
// (`.claude/rules/security.md` "Guardrails (NN-5)").

/// Logged once per process, not once per session: which upstream client was
/// selected is a deployment fact, and repeating it on every socket would bury
/// it. Per-session the choice is a `debug!` line instead.
static LIVE_CLIENT_CHOICE_LOGGED: Once = Once::new();

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
        /// The uploaded unit the student chose inside that block, if any.
        ///
        /// Optional and additive: an older client that does not send it gets
        /// the whole block, which is the behaviour that shipped. It narrows
        /// retrieval and nothing else — the block still decides what the
        /// session *is*, and the four mandatory filters are unaffected.
        #[serde(default)]
        document_id: Option<Uuid>,
        #[serde(default)]
        resume: bool,
    },
    /// The student started or stopped speaking, as decided by the client's own
    /// VAD.
    ///
    /// Gemini's automatic turn detection is disabled, so these are what open
    /// and close a turn upstream — and `speaking: true` during a tutor turn is
    /// the interruption. Detection lives in the client because only the client
    /// knows what it is playing and how loudly that returns to the microphone;
    /// the service cannot tell the student's voice from its own echo.
    Activity {
        speaking: bool,
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
    /// This turn's content is not from the student's textbook. Forwarded from
    /// the model's own declaration — the gateway cannot check it, because the
    /// only thing that could is the retrieval it already ran.
    supplementary: bool,
    ops: &'a [BoardOp],
}

/// Server -> client `turn_state`. Built from the **retrieved chunk payload**
/// (`ws::grounding::TurnState`), never from anything the model said — the
/// citation a student sees has to be the one retrieval actually produced
/// (`rag-pipeline.md`: "the tutor cites from that payload, never from its own
/// memory").
#[derive(Debug, Serialize)]
struct TurnStateMsg<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    seq: u32,
    chapter: &'a str,
    topic: &'a str,
    page: i32,
}

/// Server -> client `turn_complete`. Tells the client a tutor turn ended
/// upstream so it can stop waiting on more audio for that `seq`.
///
/// `interrupted` is `true` **only** when the turn was genuinely cut short.
/// Sending `true` as a blanket "the turn is over" signal would make the client
/// flush its jitter buffer at the end of every normal explanation and clip the
/// legitimately queued tail off it — so a normal completion is always `false`.
/// The `LiveSessionClient` trait surfaces no interruption signal distinct from
/// `TurnComplete` today, so this is currently always `false`; when the real
/// client can tell the two apart, that is the only place this needs to change.
#[derive(Debug, Serialize)]
struct TurnCompleteMsg {
    #[serde(rename = "type")]
    kind: &'static str,
    seq: Option<u32>,
    interrupted: bool,
}

/// Server -> client `transcript` — a caption for the classroom UI.
///
/// `source` is `"tutor"` or `"student"`, matching [`TranscriptSource`]. Text
/// only, no id and no PII beyond the words themselves: this is what the
/// student is watching appear under the board as the turn happens, the same
/// captions a video call would show.
#[derive(Debug, Serialize)]
struct TranscriptMsg<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    source: &'static str,
    text: &'a str,
}

fn transcript_source_label(source: TranscriptSource) -> &'static str {
    match source {
        TranscriptSource::Output => "tutor",
        TranscriptSource::Input => "student",
    }
}

/// Server -> client `quota_warning` (NN-3). Emitted once per ledger day, on
/// the tick that first enters the warning band — the ledger owns that dedupe
/// (`QuotaStatus::warning_due`), so this socket keeps no flag of its own.
#[derive(Debug, Serialize)]
struct QuotaWarningMsg {
    #[serde(rename = "type")]
    kind: &'static str,
    remaining_ms: i64,
}

/// Server -> client `session_end`. `reason` is one of
/// `quota|idle|user|jailbreak|error`, the same vocabulary
/// `learning_sessions.end_reason` stores.
#[derive(Debug, Serialize)]
struct SessionEndMsg {
    #[serde(rename = "type")]
    kind: &'static str,
    reason: &'static str,
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
    // Two accepted credentials, in this order:
    //
    // 1. a single-use `wsticket:` ticket (`ws::ticket`) — what a browser uses,
    //    because it cannot read the httpOnly cookie and cannot set a header on
    //    the upgrade. Redeeming deletes it, so it is spent the moment it is used.
    // 2. a raw access JWT — kept for non-browser clients (the k6 load harness
    //    and the Playwright e2e driver hold a real token already, and making
    //    them mint a ticket first would add a round trip to every virtual user).
    //
    // The ticket is tried first so a leaked-and-replayed ticket fails closed
    // rather than falling through to some other interpretation of the same
    // string.
    let token = query.token.clone().unwrap_or_default();
    let user_id = match ticket::redeem_ticket(&state, &token).await {
        Some(user_id) => Some(user_id),
        None => verify_access_token(&token, &state.config.jwt_access_secret)
            .ok()
            .map(|claims| UserId::from(claims.sub)),
    };

    ws.on_upgrade(move |socket| async move {
        match user_id {
            Some(user_id) => {
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

    // The upstream model. Built at `session_init` rather than here, because
    // the real client needs the NN-4 system instruction and that needs the
    // block — so there is nothing to connect to before the client has told us
    // what it wants taught. `None` until then; the polling branch is gated on
    // this being `Some`.
    //
    // Polled for the ENTIRE life of the session, not just "while a turn is in
    // flight": a live tutor has more to say after its first sentence. Gating
    // the poll on a per-turn flag (as the scripted-demo version of this file
    // did) mutes the model dead after `TurnComplete` fires once. The stub
    // still terminates correctly on its own — `StubLiveSessionClient::poll_event`
    // returns `SessionEnded` once its script is exhausted, which already
    // breaks the loop below.
    let mut live_client: Option<Box<dyn LiveSessionClient + Send>> = None;

    // NN-3. Disabled until the `students` row behind this socket is known —
    // there is no one to charge for an admin previewing the classroom.
    let mut meter = voice_meter::VoiceMeter::disabled();
    let mut quota_tick = tokio::time::interval(voice_meter::TICK);
    // A tick missed because the socket was busy must not be replayed in a
    // burst: the ledger is monotonic-clock based, so the elapsed interval is
    // charged correctly by the next tick whenever it lands.
    quota_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    // The turn's grounding: the system instruction the model is bound by and
    // the citation payload mirrored to the client as `turn_state`. Rebuilt as
    // the student asks real questions (see the `Transcript` handler below);
    // this is what the *next* `board_ops`/`turn_state` reads.
    let mut grounding: Option<grounding::Grounding> = None;
    // The retrieval scope and cached preamble, kept for the life of the
    // connection so per-turn re-grounding (NN-4) does not have to re-resolve
    // the block or re-read Redis on every question.
    let mut academic_context: Option<grounding::AcademicContext> = None;
    let mut cached_preamble: Option<String> = None;
    // The student's recorded language, kept for per-turn re-grounding: the
    // language rule in the system instruction is anchored on it (see
    // `grounding::tutor_header`).
    let mut student_locale: Option<String> = None;
    // Carried alongside the locale for the same reason: per-turn re-grounding
    // rebuilds the system instruction from scratch, and the tutor must keep
    // addressing the student by name after the opening turn.
    let mut student_name: Option<String> = None;
    // The student's words for the question currently being asked, accumulated
    // from `Transcript { source: Input, .. }` events until the model starts
    // answering. See the `Transcript` handler for why "until the model starts
    // answering" is a heuristic, not an exact turn boundary.
    let mut pending_utterance = String::new();
    // Per-turn re-grounding runs OFF this loop and reports back here.
    //
    // It used to be awaited inline in the `TranscriptSource::Output` arm, which
    // is the worst possible place for it: that arm fires the instant the model
    // starts answering, and the await held the whole `select!` for as long as
    // the embedding round trip and the Qdrant search took (measured at ~600 ms
    // against the Gemini embedding endpoint). For that entire window nothing
    // else in this loop ran — tutor audio was not polled or forwarded, and the
    // student's own frames were not pumped upstream — so the work meant to
    // improve the NEXT answer delayed THIS one. `realtime-audio.md` is explicit
    // that work like this belongs off the audio path.
    //
    // NN-4 is untouched by the move. The opening turn is still grounded
    // synchronously before a single word is taught, the four mandatory filters
    // are applied inside `grounding::build` either way, and per-turn grounding
    // was already documented (see the `Output` arm) as landing one exchange
    // late — so completing it a few hundred milliseconds later changes when the
    // context turn is injected, not what retrieval is allowed to return.
    let (grounding_tx, mut grounding_rx) =
        tokio::sync::mpsc::channel::<(String, grounding::Grounding)>(4);

    // Why the session ended, for `learning_sessions.end_reason`. Overwritten
    // only by a subsystem that actually detected a cause (quota, here).
    let mut end_reason: &'static str = "user";
    let mut session_id: Option<Uuid> = None;
    // The `board_events` writer. Disabled until `session_init` has created the
    // durable `learning_sessions` row, because `board_events.session_id` is a
    // foreign key to it — auditing before then is a guaranteed constraint
    // failure on every turn.
    let mut audit = board_audit::BoardAudit::disabled();
    let mut recorded_session = false;
    // What the student's conversation looked like, for the end-of-unit
    // assessment (`engagement.rs`). Accumulated as transcripts arrive and
    // consumed once, after the socket closes.
    let mut engagement = engagement::EngagementLog::new();
    // The student and block the assessment is filed against.
    let mut student_for_assessment: Option<(dg_core::StudentId, dg_core::BlockId)> = None;
    // The unit being taught, recorded only once `with_unit` has validated it
    // belongs to the block. `None` means the student opened the whole block,
    // and a block-wide session is deliberately NOT assessed per unit: there is
    // no single unit the verdict would be about.
    let mut unit: Option<(dg_core::DocumentId, String)> = None;
    // Barge-in diagnostics; see the `Message::Binary` arm.
    let mut student_frames_in_turn: u32 = 0;
    let mut last_frame_report = Instant::now();

    // The turn the model is currently narrating. `AudioChunk` events from
    // the stub don't carry their own `seq` (per the `LiveModelEvent`
    // contract), so it is tracked from the most recent accepted `board_ops`.
    let mut current_seq: Option<TurnSeq> = None;
    // Set once `session_init` has been handled — gates when we start
    // draining the stub's scripted turn, so nothing is sent before the
    // client is ready to receive `session_ready`.

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
                            Ok(ClientMessage::SessionInit {
                                block_id,
                                document_id,
                                resume,
                            }) => {
                                let sid = init_session(&state, user_id, block_id, resume).await;

                                // Durable row BEFORE anything is audited against
                                // it: `board_events.session_id` is a foreign key
                                // here, so auditing a turn for a session with no
                                // row silently loses the whole NN-1 trail (and
                                // leaves `/admin/sessions` permanently empty).
                                //
                                // A socket whose account has no `students` row
                                // (an admin opening the classroom) gets no
                                // session row and therefore no audit — the
                                // lesson still runs, which is the right
                                // precedence: the student's experience is never
                                // blocked by bookkeeping.
                                // The whole row, not just its id: NN-2's
                                // `is_first_login` and NN-3's `timezone` both
                                // live on it, and reading it twice in the
                                // session-init path would be two round trips
                                // for one row.
                                let student = match dg_db::models::students::find_by_user_id(
                                    &state.pool,
                                    user_id,
                                )
                                .await
                                {
                                    Ok(Some(student)) => Some(student),
                                    Ok(None) => {
                                        tracing::warn!(%user_id, "classroom socket for an account with no student row; session will not be recorded");
                                        None
                                    }
                                    Err(err) => {
                                        tracing::error!(%err, "failed to resolve the student for a classroom session");
                                        None
                                    }
                                };

                                let student_id = student.as_ref().map(|s| s.id);

                                if let Some(student_id) = student_id {
                                    if let Err(err) = dg_db::models::learning_sessions::open(
                                        &state.pool,
                                        dg_core::SessionId::from(sid),
                                        student_id,
                                        dg_core::BlockId::from(block_id),
                                    )
                                    .await
                                    {
                                        // Logged, not fatal. The same precedence
                                        // as above: teaching continues without a
                                        // durable record rather than failing.
                                        tracing::error!(%err, %sid, "failed to open the learning_sessions row");
                                    }
                                }

                                session_id = Some(sid);
                                recorded_session = student_id.is_some();
                                // Captured for the end-of-unit assessment,
                                // which runs after `student` has gone out of
                                // scope.
                                student_for_assessment =
                                    student_id.map(|id| (id, dg_core::BlockId::from(block_id)));
                                if recorded_session {
                                    audit = board_audit::BoardAudit::spawn(state.pool.clone(), sid);
                                }
                                // NN-3: open the ledger before any audio can
                                // flow, so the first frame is already metered.
                                if let Some(student) = student.as_ref() {
                                    meter = voice_meter::VoiceMeter::enabled(
                                        student.id,
                                        &student.timezone,
                                        state.redis.clone(),
                                    );
                                }

                                // A student who has already spent the day's
                                // allowance is told so and closed immediately,
                                // rather than being allowed to open a lesson
                                // the first tick would kill a second later.
                                let remaining_ms = meter.remaining_ms().await;
                                if remaining_ms <= 0 {
                                    end_reason = quota::END_REASON_QUOTA;
                                    break;
                                }

                                // NN-4 grounding. Resolved from the block id
                                // server-side: the client never supplies the
                                // retrieval scope.
                                let context = match grounding::resolve_context(
                                    &state.pool,
                                    dg_core::BlockId::from(block_id),
                                )
                                .await
                                {
                                    Ok(context) => context,
                                    Err(err) => {
                                        tracing::error!(%err, %block_id, "failed to resolve the academic context for a classroom session");
                                        None
                                    }
                                };

                                // The chosen unit, validated against the block
                                // before it is allowed to narrow anything. A
                                // failure here leaves the context teaching the
                                // whole block, which is the shipped behaviour
                                // and a safe fallback.
                                let context = match context {
                                    Some(context) => match context
                                        .with_unit(
                                            &state.pool,
                                            document_id.map(dg_core::DocumentId::from),
                                        )
                                        .await
                                    {
                                        Ok(context) => {
                                            // Persist the unit now that it is
                                            // known to belong to the block.
                                            // `session_init` has always taken a
                                            // `document_id` but never stored
                                            // it, so nothing could say which
                                            // unit a student had studied.
                                            if let (Some(doc), Some(title), true) = (
                                                context.unit_document_id,
                                                context.unit_title.clone(),
                                                recorded_session,
                                            ) {
                                                unit = Some((doc, title));
                                                if let Err(err) =
                                                    dg_db::models::learning_sessions::set_document(
                                                        &state.pool,
                                                        dg_core::SessionId::from(sid),
                                                        doc,
                                                    )
                                                    .await
                                                {
                                                    // Logged, not fatal: the
                                                    // lesson matters more than
                                                    // the record of which unit
                                                    // it was.
                                                    tracing::error!(%err, %sid, "failed to record the session's unit");
                                                }
                                            }
                                            Some(context)
                                        }
                                        Err(err) => {
                                            tracing::error!(%err, %block_id, "failed to validate the chosen unit; teaching the whole block");
                                            None
                                        }
                                    },
                                    None => None,
                                };

                                let ready = SessionReadyMsg {
                                    kind: "session_ready",
                                    session_id: sid,
                                    // NN-2: the greeting plays only on a true
                                    // first login. With no students row there
                                    // is no first login to announce.
                                    is_first_login: student
                                        .as_ref()
                                        .map(|s| s.is_first_login)
                                        .unwrap_or(false),
                                    context: match context.as_ref() {
                                        Some(context) => context.to_session_json(),
                                        None => serde_json::json!({ "block_id": block_id }),
                                    },
                                    quota_remaining_ms: remaining_ms.max(0) as u64,
                                };
                                if send_json(&mut socket, &ready).await.is_err() {
                                    break;
                                }

                                // Grounding, then the client — in that order,
                                // because the instruction is an input to the
                                // client's setup message and cannot be bolted
                                // on afterwards.
                                if let Some(context) = context {
                                    let preamble = grounding::load_preamble(
                                        &mut state.redis.clone(),
                                        context.block_id,
                                    )
                                    .await;
                                    // There is no student utterance yet, so the
                                    // opening turn is grounded on the block
                                    // itself. Real per-turn re-grounding
                                    // happens later, from the input transcript
                                    // (see the `Transcript` handler) — kept
                                    // here for the first turn only.
                                    academic_context = Some(context.clone());
                                    student_locale = student.as_ref().map(|s| s.locale.clone());
                                    student_name = student.as_ref().map(|s| s.full_name.clone());
                                    cached_preamble = preamble.clone();
                                    let opening_question =
                                        format!("{} {}", context.block_title, context.course_code);
                                    let pack = grounding::build(
                                        qdrant_client(&state).as_ref(),
                                        &context,
                                        preamble.as_deref(),
                                        &opening_question,
                                        state.config.gemini_api_key.as_deref(),
                                        student.as_ref().map(|s| s.locale.as_str()),
                                        student.as_ref().map(|s| s.full_name.as_str()),
                                    )
                                    .await;
                                    // Visible in the log because an
                                    // abstaining opening turn means the block
                                    // has no embedded corpus above the floor —
                                    // a content problem, not a bug, and one
                                    // nobody can diagnose from the audio.
                                    tracing::debug!(
                                        abstained = pack.abstained,
                                        block_no = context.block_no,
                                        "grounded the opening turn"
                                    );
                                    let mut client =
                                        select_live_client(&state, &pack.system_instruction, student.as_ref().map(|s| s.locale.as_str())).await;

                                    // Kick off the first turn. Without this the
                                    // tutor sits silent until the student
                                    // speaks first — fine for a returning
                                    // student who knows to just talk, but NN-2
                                    // requires the greeting to actually PLAY on
                                    // a first login, and a first-time student
                                    // has no way to know they should speak
                                    // first to trigger it.
                                    if let Some(student) = student.as_ref() {
                                        // First name only, for the same reason
                                        // the standing instruction uses one: a
                                        // greeting that reads out a full legal
                                        // name sounds like a roll-call, not a
                                        // welcome. An empty name falls back to
                                        // the full string rather than greeting
                                        // nobody.
                                        let first_name = student
                                            .full_name
                                            .split_whitespace()
                                            .next()
                                            .unwrap_or(student.full_name.as_str());
                                        let kickoff = if student.is_first_login {
                                            // The welcome is FIXED COPY, quoted
                                            // verbatim rather than described.
                                            // It is the product owner's wording
                                            // and it teaches the student the
                                            // wake word, so a paraphrase would
                                            // quietly drop the one instruction
                                            // the whole session depends on.
                                            //
                                            // The one substitution is the name:
                                            // the copy opened "നമസ്കാരം
                                            // വിദ്യാർത്ഥികളെ!" — plural, and
                                            // addressed to a room — in a
                                            // product where every session has
                                            // exactly one student whose name we
                                            // already hold.
                                            format!(
                                                "The student, {}, has just logged in for the \
first time. Begin by saying EXACTLY this, word for word, and nothing before it:\n\n\
\"നമസ്കാരം {first_name}! ഞാൻ നിങ്ങളുടെ ഡിജി ഗുരു. ഇന്ന് നമ്മൾ ഒരുമിച്ചാണ് പാഠഭാഗങ്ങൾ \
പഠിക്കുന്നത്. ഈ സംവാദത്തിനിടയിൽ നിങ്ങൾക്ക് എന്ത് സംശയവും എന്നോട് ചോദിക്കാവുന്നതാണ്. \
നിങ്ങൾക്ക് സഹായം ആവശ്യമുള്ളപ്പോൾ എന്നെ 'ഗുരു' എന്ന് വിളിക്കുക.\"\n\n\
Then begin teaching Block {}: \"{}\" for {}. Call board_ops before you speak, as instructed.",
                                                student.full_name,
                                                context.block_no,
                                                context.block_title,
                                                context.course_code,
                                            )
                                        } else {
                                            // Resume: NN-2's greeting is
                                            // strictly a first-login thing, so
                                            // a returning student is told, in
                                            // the instruction the STUDENT never
                                            // sees, not to repeat it.
                                            let progress = dg_db::models::learning_sessions::latest_progress_for_block(
                                                &state.pool,
                                                student.id,
                                                dg_core::BlockId::from(block_id),
                                                dg_core::SessionId::from(sid),
                                            )
                                            .await
                                            .unwrap_or_else(|err| {
                                                tracing::warn!(%err, "could not read prior progress for the resume kickoff");
                                                None
                                            });
                                            match progress {
                                                Some((topic, page)) => format!(
                                                    "The student, {}, is returning. Do NOT greet \
them again. Briefly continue from where they left off — topic \"{topic}\", page {page} — for \
Block {}: \"{}\" ({}). Call board_ops before you speak.",
                                                    student.full_name,
                                                    context.block_no,
                                                    context.block_title,
                                                    context.course_code,
                                                ),
                                                None => format!(
                                                    "The student, {}, is continuing this block. \
Do NOT greet them again. Begin teaching Block {}: \"{}\" for {}. Call board_ops before you \
speak.",
                                                    student.full_name,
                                                    context.block_no,
                                                    context.block_title,
                                                    context.course_code,
                                                ),
                                            }
                                        };

                                        match client.send_text_turn(&kickoff, true).await {
                                            Ok(()) => {
                                                if student.is_first_login {
                                                    // Flipped only once the
                                                    // greeting has actually
                                                    // been queued — a failed
                                                    // send should not burn the
                                                    // one greeting NN-2
                                                    // promises.
                                                    if let Err(err) = dg_db::models::students::clear_first_login(
                                                        &state.pool,
                                                        student.id,
                                                    )
                                                    .await
                                                    {
                                                        tracing::error!(%err, "failed to clear is_first_login after queuing the greeting");
                                                    }
                                                }
                                            }
                                            Err(err) => {
                                                // Not fatal: the session still
                                                // runs, just silent until the
                                                // student speaks first — the
                                                // same behaviour as before this
                                                // kickoff existed.
                                                tracing::warn!(%err, "failed to send the session kickoff turn");
                                            }
                                        }
                                    }

                                    live_client = Some(client);
                                    grounding = Some(pack);
                                } else {
                                    // No resolvable block: the session still
                                    // runs, but never against a real upstream.
                                    // NN-4 forbids teaching from nothing, and
                                    // an ungrounded real client is exactly
                                    // that.
                                    tracing::warn!(%block_id, "no academic context for the block; running the scripted stub client");
                                    live_client =
                                        Some(Box::new(StubLiveSessionClient::new(demo_script())));
                                }

                            }
                            Ok(ClientMessage::Activity { speaking }) => {
                                // Forwarded verbatim and immediately. This is
                                // the interruption path, so it must not wait
                                // behind anything — `send_activity` puts it on
                                // the control queue for the same reason.
                                if let Some(client) = live_client.as_mut() {
                                    if let Err(err) = client.send_activity(speaking).await {
                                        tracing::warn!(
                                            ?err,
                                            speaking,
                                            "failed to forward a student activity signal upstream"
                                        );
                                    } else {
                                        tracing::debug!(speaking, "student activity forwarded");
                                    }
                                }
                            }
                            Ok(ClientMessage::BoardAck { seq }) => {
                                let outcome = sync_gate.on_board_ack(TurnSeq(seq));
                                if flush_outcome(&mut socket, &mut sync_gate, &audit, outcome).await.is_err() {
                                    break;
                                }
                            }
                            Ok(ClientMessage::BoardError { seq, reason }) => {
                                // The gate logs the degrade-to-text-fallback
                                // decision and counts it; teaching continues.
                                let outcome = sync_gate.on_board_error(TurnSeq(seq), &reason);
                                if flush_outcome(&mut socket, &mut sync_gate, &audit, outcome).await.is_err() {
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
                        // Student mic audio: 16 kHz mono PCM16, 20 ms frames
                        // (`realtime-audio.md`), forwarded upstream verbatim.
                        // No transcoding here — resampling on the audio path
                        // would cost a frame of latency for nothing, since the
                        // capture rate is already the rate Gemini Live wants
                        // on the way in.
                        //
                        // Metered first (NN-3) and then sent: a frame that is
                        // charged but dropped is honest, whereas one sent but
                        // uncharged is a quota hole.
                        meter.note_audio_frame().await;

                        // Barge-in diagnostics. Interruption depends on the
                        // student's audio actually reaching Gemini while the
                        // tutor is mid-turn, and "it does not stop" has three
                        // possible causes that look identical from outside:
                        // the browser withholding frames, the gateway not
                        // forwarding them, or Gemini not acting on them. This
                        // says which, once a second, and only while a turn is
                        // in flight — so it is silent in normal operation.
                        student_frames_in_turn += 1;
                        if current_seq.is_some() && last_frame_report.elapsed() >= Duration::from_secs(1) {
                            tracing::info!(
                                frames = student_frames_in_turn,
                                turn = ?current_seq.map(|s| s.get()),
                                "student audio forwarded upstream during an active tutor turn"
                            );
                            last_frame_report = Instant::now();
                            student_frames_in_turn = 0;
                        }

                        match live_client.as_mut() {
                            Some(client) => {
                                if let Err(err) = client.send_audio_frame(data).await {
                                    // Not fatal on its own: `poll_event` is the
                                    // branch that decides a session is over, so
                                    // one rejected frame degrades the turn
                                    // rather than dropping the student.
                                    tracing::warn!(?err, "failed to forward a student audio frame upstream");
                                }
                            }
                            None => {
                                // Audio before `session_init` has no session to
                                // belong to. Dropped, not buffered: buffering
                                // it would let an unauthenticated-but-upgraded
                                // socket consume memory before it has told us
                                // anything.
                                tracing::debug!(bytes = data.len(), "student audio before session_init; dropped");
                            }
                        }
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
                    if flush_released(&mut socket, &mut sync_gate, &audit, audio).await.is_err() {
                        send_failed = true;
                        break;
                    }
                }
                if send_failed {
                    break;
                }
            }

            event = async {
                match live_client.as_mut() {
                    Some(client) => client.poll_event().await,
                    // `None` until `session_init` builds a client. `pending()`
                    // is the correct neutral element for a select branch
                    // rather than an `unwrap` on the audio path.
                    None => std::future::pending().await,
                }
            }, if live_client.is_some() => {
                match event {
                    Ok(LiveModelEvent::Transcript { source, text }) => {
                        // What the student actually said (Input), and what the
                        // tutor actually said (Output). Not logged beyond a
                        // length (`security.md`: "log student identifiers,
                        // never student PII") — sending it to THIS student's
                        // own client as a caption is not the exposure that
                        // rule guards against; a server-side log is.
                        tracing::debug!(?source, chars = text.chars().count(), "live transcript");

                        // Counted before it is forwarded, so a client that
                        // disconnects mid-turn still contributes what it said.
                        engagement.push(matches!(source, TranscriptSource::Input), &text);

                        let caption = TranscriptMsg {
                            kind: "transcript",
                            source: transcript_source_label(source),
                            text: &text,
                        };
                        if send_json(&mut socket, &caption).await.is_err() {
                            break;
                        }

                        // NN-5 Tier 1 belongs HERE, on the input side, before
                        // the utterance below is used for anything — a hit
                        // must tear the socket down within 300 ms and write a
                        // `safety_incidents` row (`security.md`). Not built
                        // yet; flagged rather than silently skipped.
                        match source {
                            TranscriptSource::Input => {
                                if !pending_utterance.is_empty() {
                                    pending_utterance.push(' ');
                                }
                                pending_utterance.push_str(&text);
                            }
                            TranscriptSource::Output => {
                                // The model has started answering, which is
                                // this pass's signal that the student's
                                // utterance is complete — Gemini Live's own
                                // turn-taking decides when to respond, and the
                                // gateway has no earlier hook into that
                                // decision without disabling audio
                                // auto-response entirely (out of scope here).
                                //
                                // Consequence, stated plainly: grounding built
                                // from this utterance necessarily lands one
                                // exchange late — it informs the model's
                                // response to the student's NEXT question, not
                                // the one that just triggered this branch. A
                                // multi-turn conversation still gets steadily
                                // more relevant citations as it goes, which is
                                // real progress over the once-per-session
                                // grounding this replaces; it is not per-turn
                                // in the strict sense `rag-pipeline.md`
                                // describes. That needs a transcript-driven
                                // turn model, not audio auto-response.
                                if !pending_utterance.trim().is_empty() {
                                    match (academic_context.as_ref(), live_client.as_mut()) {
                                        (Some(context), Some(client)) => {
                                            let question = std::mem::take(&mut pending_utterance);
                                            // Spawned, not awaited: see
                                            // `grounding_tx` above for why this
                                            // must not block the loop. A full
                                            // channel means grounding is slower
                                            // than the student is asking, and
                                            // dropping the newest request is
                                            // right — the queued ones are
                                            // already closer to the question
                                            // actually being answered.
                                            let task_state = state.clone();
                                            let task_context = context.clone();
                                            let task_preamble = cached_preamble.clone();
                                            let task_locale = student_locale.clone();
                                            let task_name = student_name.clone();
                                            let task_tx = grounding_tx.clone();
                                            tokio::spawn(async move {
                                                let pack = grounding::build(
                                                    qdrant_client(&task_state).as_ref(),
                                                    &task_context,
                                                    task_preamble.as_deref(),
                                                    &question,
                                                    task_state.config.gemini_api_key.as_deref(),
                                                    task_locale.as_deref(),
                                                    task_name.as_deref(),
                                                )
                                                .await;
                                                let _ = task_tx.try_send((question, pack));
                                            });

                                            // The context turn itself is sent
                                            // by the `grounding_rx` branch,
                                            // once the spawned task reports
                                            // back. `client` is only borrowed
                                            // here to prove there is a live
                                            // session to send it to.
                                            let _ = client;
                                        }
                                        _ => pending_utterance.clear(),
                                    }
                                }
                            }
                        }
                    }
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
                            supplementary: msg.supplementary,
                            ops: &accepted.ops,
                        };
                        if send_json(&mut socket, &out).await.is_err() {
                            break;
                        }
                        // Audit after the frame is on the wire, never before:
                        // the student seeing the board is the thing that
                        // matters, and the insert must not sit in front of it.
                        audit.ops_emitted(accepted.seq.get(), &accepted.ops);

                        // `turn_state` carries the citation for the turn, from
                        // the retrieved chunk payload. Sent after `board_ops`
                        // and before any audio, so the label the student reads
                        // never lags the board it describes. A turn that
                        // abstained has no citation and deliberately sends
                        // nothing, rather than repeating a stale one.
                        if let Some(state_msg) = grounding
                            .as_ref()
                            .and_then(|g| g.turn_state.as_ref())
                        {
                            let out = TurnStateMsg {
                                kind: "turn_state",
                                seq: accepted.seq.get(),
                                chapter: &state_msg.chapter,
                                topic: &state_msg.topic,
                                page: state_msg.page,
                            };
                            if send_json(&mut socket, &out).await.is_err() {
                                break;
                            }
                            // Same values into the durable row, so the
                            // dashboard's resume card points at the paragraph
                            // the lesson actually reached.
                            if let Some(sid) = session_id {
                                if recorded_session {
                                    if let Err(err) = dg_db::models::learning_sessions::record_progress(
                                        &state.pool,
                                        dg_core::SessionId::from(sid),
                                        Some(state_msg.topic.as_str()),
                                        Some(state_msg.page),
                                    )
                                    .await
                                    {
                                        tracing::warn!(%err, %sid, "failed to record turn progress");
                                    }
                                }
                            }
                        }
                        // The hold timer for this turn is picked up on the
                        // next loop iteration via `next_hold_deadline_ms()`.
                    }
                    Ok(LiveModelEvent::AudioChunk { seq: chunk_seq, data: frame }) => {
                        let chunk_seq = TurnSeq(chunk_seq);

                        // Ask the gate whether it already knows this turn, by
                        // offering the frame. This ordering matters: a turn's
                        // audio keeps arriving for a while after the NEXT
                        // turn's board has opened (Gemini bursts a turn's audio
                        // faster than realtime), so "this is not the newest
                        // turn" must NOT mean "throw the audio away" — the
                        // earlier turn is still open and still owes the student
                        // the rest of its sentence. Dropping those frames was
                        // heard as the tutor going intermittently silent.
                        let mut disposition = sync_gate.push_audio(chunk_seq, &frame);

                        if disposition == AudioDisposition::UnknownTurn {
                            // The gate has never seen this seq. Either the model
                            // spoke without calling `board_ops` first (NN-1's
                            // edge case — nothing was reordered, because there
                            // was never a board to get ahead of), or the turn is
                            // old enough to have been retired.
                            match sync_gate.open_implicit_turn(chunk_seq) {
                                Ok(()) => {
                                    tracing::debug!(seq = %chunk_seq, "opened an implicit (board-less) turn for audio");
                                    disposition = sync_gate.push_audio(chunk_seq, &frame);
                                }
                                Err(err) => {
                                    tracing::debug!(seq = %chunk_seq, %err, "dropping audio for a retired turn");
                                    continue;
                                }
                            }
                        }

                        // Only ever advance. A late frame from an earlier turn
                        // must not drag the "current turn" backwards, or the
                        // next turn's `turn_state`/audio would be filed under a
                        // stale seq.
                        if current_seq.is_none_or(|current| chunk_seq > current) {
                            current_seq = Some(chunk_seq);
                        }
                        let seq = chunk_seq;

                        // Tutor audio is active voice too (NN-3 / D-19):
                        // the allowance pays for both halves of the
                        // conversation. Charged on arrival rather than on
                        // release, because the student is being taught whether
                        // the frame is forwarded now or a few ms later behind
                        // the board.
                        meter.note_audio_frame().await;
                        match disposition {
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
                    Ok(LiveModelEvent::TurnComplete { interrupted }) => {
                        if interrupted {
                            // The generation was cancelled upstream. Anything
                            // still held for this turn belongs to it and must
                            // not play later — see `SyncGate::discard`.
                            if let Some(seq) = current_seq {
                                sync_gate.discard(seq);
                            }
                        }
                        // Tell the client the turn ended upstream so it can
                        // stop waiting for more audio for this `seq`.
                        //
                        // `interrupted` is the service's own verdict (`serverContent.interrupted`),
                        // forwarded as-is. The client flushes its jitter buffer only on `true`,
                        // so a blanket `true` would clip the tail off every explanation and a
                        // blanket `false` — which this used to send — leaves a cancelled turn
                        // playing over the student. Neither is acceptable; the flag has to be real.
                        let out = TurnCompleteMsg {
                            kind: "turn_complete",
                            seq: current_seq.map(|seq| seq.get()),
                            interrupted,
                        };
                        if send_json(&mut socket, &out).await.is_err() {
                            break;
                        }
                        // NOT "stop polling" — the model has more turns ahead
                        // of it for the rest of the session (see the note on
                        // `live_client` above). Only the per-turn seq tracking
                        // resets, so the next turn's audio is judged against
                        // ITS OWN board, not the one that just finished.
                        current_seq = None;
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
                        end_reason = "error";
                        break;
                    }
                }
            }

            // A per-turn re-grounding finished. Injected as a user turn that
            // does NOT complete the exchange: `systemInstruction` cannot change
            // mid-session, so refreshed curriculum context travels this way
            // instead. `turn_complete: false` keeps the model waiting for the
            // student's real next utterance rather than answering this context
            // message itself.
            //
            // Ordered after the upstream poll so a frame already in flight is
            // forwarded first: this branch is bookkeeping for the NEXT answer,
            // never for the one currently being spoken.
            Some((question, pack)) = grounding_rx.recv() => {
                tracing::debug!(
                    abstained = pack.abstained,
                    "grounded a question from the input transcript"
                );
                if let Some(client) = live_client.as_mut() {
                    let context_turn = format!(
                        "[Updated curriculum grounding for the student's \nmost recent question: \"{question}\"]

{}",
                        pack.system_instruction
                    );
                    if let Err(err) = client.send_text_turn(&context_turn, false).await {
                        tracing::warn!(%err, "failed to send per-turn curriculum context");
                    } else {
                        grounding = Some(pack);
                    }
                }
            }

            // NN-3 enforcement, once a second. The ledger decides; this branch
            // only relays. It is deliberately the last branch under `biased`:
            // a frame already in flight is forwarded before the tick that may
            // end the session, so the student never loses audio they had
            // already been allocated.
            _ = quota_tick.tick() => {
                if let Some(status) = meter.tick().await {
                    if status.warning_due {
                        let warning = QuotaWarningMsg {
                            kind: "quota_warning",
                            remaining_ms: status.remaining_ms,
                        };
                        if send_json(&mut socket, &warning).await.is_err() {
                            break;
                        }
                    }
                    if status.is_exhausted() {
                        tracing::info!(
                            used_ms = status.used_ms,
                            day = %status.day_key,
                            "daily voice allowance spent; ending the session"
                        );
                        let ended = SessionEndMsg {
                            kind: "session_end",
                            reason: quota::END_REASON_QUOTA,
                        };
                        // Best-effort: the close frame below is what actually
                        // ends it, so a failed send must not skip the close.
                        let _ = send_json(&mut socket, &ended).await;
                        end_reason = quota::END_REASON_QUOTA;
                        break;
                    }
                }
            }
        }
    }

    // Charge the final interval before the ledger is dropped, and take the
    // day's authoritative total for the durable row.
    meter.finish().await;
    let active_voice_ms = meter.used_ms().await;

    // Every exit from the loop above closes the socket EXPLICITLY, whatever
    // ended it. Simply returning here drops the connection without a close
    // frame, which the browser reports as 1006 — and 1006 is, correctly,
    // retryable (`protocol.ts::shouldReconnect`), so the client reconnects,
    // reaches the same ordinary end, and is dropped again: a session that
    // finished normally presents to the student as a permanent
    // "Reconnecting". The close code is what tells the client whether the end
    // was a decision or an accident, so it has to be sent on the normal path
    // too, not only on the quota path.
    //
    // Sent before the `learning_sessions` write so the client is released
    // promptly; the write is bookkeeping and the student is not kept waiting.
    if end_reason == quota::END_REASON_QUOTA {
        // The documented close code for a quota end (`4003`).
        let _ = socket
            .send(Message::Close(Some(CloseFrame {
                code: CLOSE_QUOTA,
                reason: "quota".into(),
            })))
            .await;
    } else {
        // An ordinary end: the upstream session finished, the student asked to
        // stop, or the socket is being torn down. `session_end` carries the
        // reason the durable row records; the 1000 that follows is what stops
        // the client retrying. Both are best-effort — a peer that has already
        // gone away cannot be told anything, and that is not an error.
        let ended = SessionEndMsg {
            kind: "session_end",
            reason: end_reason,
        };
        let _ = send_json(&mut socket, &ended).await;
        let _ = socket
            .send(Message::Close(Some(CloseFrame {
                code: CLOSE_NORMAL,
                reason: end_reason.into(),
            })))
            .await;
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
    // Close the durable row. `end_reason` is `user` for an ordinary teardown
    // and is overwritten only by the subsystem that detected a real cause
    // (quota, above; idle/jailbreak when those land). `close` refuses to
    // relabel an already-closed session, so a guardrail termination keeps its
    // own reason even though this runs after it.
    if recorded_session {
        if let Some(sid) = session_id {
            if let Err(err) = dg_db::models::learning_sessions::close(
                &state.pool,
                dg_core::SessionId::from(sid),
                end_reason,
                active_voice_ms,
            )
            .await
            {
                tracing::error!(%err, %sid, "failed to close the learning_sessions row");
            }
        }
    }

    // The end-of-unit assessment. Spawned rather than awaited: the student has
    // already left, a REST call to a text model takes seconds, and nothing
    // about this teardown should wait on it. Every failure inside is logged and
    // swallowed — a missing assessment costs a dashboard card, not a lesson.
    //
    // Only a session that (a) recorded a durable row, (b) named a single unit
    // and (c) has a configured API key is assessed. A block-wide session has no
    // one unit for the verdict to be about, and inventing one would file the
    // student's performance against a unit they may never have opened.
    if let (Some(sid), Some(student), Some((document_id, unit_title)), Some(api_key)) = (
        session_id,
        student_for_assessment,
        unit,
        state.config.gemini_api_key.clone(),
    ) {
        let (counters, transcript) = engagement.finish(active_voice_ms);
        if counters.is_assessable() {
            let job = engagement::AssessmentJob {
                student_id: student.0,
                document_id,
                block_id: student.1,
                session_id: dg_core::SessionId::from(sid),
                unit_title,
                counters,
                transcript,
            };
            let pool = state.pool.clone();
            tokio::spawn(engagement::run_assessment(
                pool,
                api_key,
                live::assess::ASSESSMENT_MODEL.to_string(),
                job,
            ));
        }
    }

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
    audit: &board_audit::BoardAudit,
    audio: ReleasedAudio,
) -> Result<(), axum::Error> {
    let ReleasedAudio {
        seq,
        reason,
        held_ms,
        frames,
    } = audio;
    tracing::debug!(seq = %seq, ?reason, held_ms, frames = frames.len(), "releasing held audio");

    // The release edge is where the turn's ACK latency becomes known, whatever
    // the reason — an ack, a board_error, or the hold ceiling. Recorded for all
    // three, because the `>400ms` analytics bucket exists precisely to surface
    // the ceiling case.
    audit.ack_latency(seq.get(), held_ms);

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
    audit: &board_audit::BoardAudit,
    outcome: AckOutcome,
) -> Result<(), axum::Error> {
    match outcome {
        AckOutcome::Released(audio) => flush_released(socket, sync_gate, audit, *audio).await,
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
fn to_gate_point(point: &live::board::DataPoint) -> live::DataPoint {
    live::DataPoint {
        label: point.label.clone(),
        value: point.value,
    }
}

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
                            tracing::warn!(
                                shape = other,
                                "unknown draw shape from model; op dropped"
                            );
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
                // Charts pass straight through: the model supplies labelled
                // quantities and the client decides the geometry, so there is
                // nothing to translate and nothing the model can get wrong
                // about pixels.
                live::board::BoardOp::BarChart { title, series } => Some(BoardOp::BarChart {
                    id,
                    title: title.clone(),
                    series: series.iter().map(to_gate_point).collect(),
                }),
                live::board::BoardOp::PieChart { title, series } => Some(BoardOp::PieChart {
                    id,
                    title: title.clone(),
                    series: series.iter().map(to_gate_point).collect(),
                }),
                live::board::BoardOp::Flow { title, steps } => Some(BoardOp::Flow {
                    id,
                    title: title.clone(),
                    steps: steps.clone(),
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
        match redis
            .hget::<_, _, Option<String>>(&ctx_key, "session_id")
            .await
        {
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

/// Chooses the upstream model client for one session.
///
/// `GEMINI_API_KEY` present -> the real `live::GeminiLiveSessionClient`, with
/// the NN-4 system instruction this gateway built. Absent, or the connect
/// fails -> `live::StubLiveSessionClient` driving [`demo_script`].
///
/// The stub is not dead code and not a debug affordance: it is the offline dev
/// loop (`./dev.ps1` with no key) and the client the NN-1 ordering tests drive,
/// so every change here has to leave it reachable. Falling back on a failed
/// connect is deliberate for the same reason the contract requires a missing
/// key to degrade rather than panic — a credential or upstream problem must
/// never be the thing that drops a student's lesson.
///
/// The key is read from config and handed straight to `connect`; it is never
/// logged, formatted, or put in an error message here.
async fn select_live_client(
    state: &AppState,
    system_instruction: &str,
    locale: Option<&str>,
) -> Box<dyn LiveSessionClient + Send> {
    let Some(api_key) = state.config.gemini_api_key.as_deref() else {
        LIVE_CLIENT_CHOICE_LOGGED.call_once(|| {
            tracing::info!(
                "no GEMINI_API_KEY configured: classroom sessions run the scripted stub client"
            );
        });
        return Box::new(StubLiveSessionClient::new(demo_script()));
    };

    LIVE_CLIENT_CHOICE_LOGGED.call_once(|| {
        tracing::info!(
            "GEMINI_API_KEY configured: classroom sessions use the real Gemini Live client"
        );
    });

    // The synthesis language comes from the student's locale, not from the
    // prompt: telling the model "speak Malayalam" still leaves the synthesiser
    // inferring a language from the text, which is how English and Malayalam
    // ended up mixed inside one sentence.
    let mut config = live::GeminiLiveConfig::new(api_key, system_instruction);
    if let Some(code) = locale.filter(|l| !l.trim().is_empty()) {
        config = config.with_language(code);
    }
    match live::GeminiLiveSessionClient::connect(config).await {
        Ok(client) => {
            tracing::debug!(
                instruction_bytes = system_instruction.len(),
                "opened a real Gemini Live session"
            );
            Box::new(client)
        }
        Err(err) => {
            tracing::error!(%err, "could not open a Gemini Live session; falling back to the scripted stub");
            Box::new(StubLiveSessionClient::new(demo_script()))
        }
    }
}

/// A Qdrant handle for this session's retrieval, or `None` if one cannot be
/// built.
///
/// Built per session rather than held in `AppState`: retrieval runs at most a
/// handful of times per session, and adding a fourth connection to the
/// startup path would make an unreachable Qdrant a boot failure for the whole
/// gateway — including the admin console, which does not need it. `None`
/// grounds the turn as an abstention (`ws::grounding`), which is the
/// NN-4-correct failure direction.
fn qdrant_client(state: &AppState) -> Option<qdrant_client::Qdrant> {
    // `QDRANT_URL` is the REST URL by convention across this workspace, but this
    // is a gRPC client — see `rag::qdrant::grpc_url` for why pointing it at the
    // REST port fails as an opaque h2 protocol error rather than a port mismatch.
    let url = rag::qdrant::grpc_url(&state.config.qdrant_url);
    match qdrant_client::Qdrant::from_url(&url).build() {
        Ok(client) => Some(client),
        Err(err) => {
            tracing::warn!(%err, "could not build a Qdrant client; this session will abstain");
            None
        }
    }
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
        supplementary: false,
        ops: vec![
            live::board::BoardOp::Heading {
                text: "Demo lesson".to_string(),
                page: Some(1),
            },
            live::board::BoardOp::Bullets {
                items: vec!["This is a scripted Phase 3 demo turn.".to_string()],
            },
        ],
    };

    vec![
        LiveModelEvent::BoardOps(board_ops),
        LiveModelEvent::AudioChunk {
            seq: 1,
            data: bytes::Bytes::from_static(b"demo-audio-frame-1"),
        },
        LiveModelEvent::AudioChunk {
            seq: 1,
            data: bytes::Bytes::from_static(b"demo-audio-frame-2"),
        },
        LiveModelEvent::TurnComplete { interrupted: false },
        LiveModelEvent::SessionEnded,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Contract test (`.claude/rules/testing.md`, "Contract"): the outbound
    /// control frames must serialise to exactly the shapes documented in
    /// `.claude/rules/api-conventions.md` "Server to client". A renamed field
    /// here is a silently broken client, because unknown keys are ignored by
    /// design.
    #[test]
    fn turn_state_matches_the_documented_shape() {
        let json = serde_json::to_value(TurnStateMsg {
            kind: "turn_state",
            seq: 42,
            chapter: "3",
            topic: "Vritham",
            page: 57,
        })
        .expect("serialises");

        assert_eq!(
            json,
            serde_json::json!({
                "type": "turn_state", "seq": 42,
                "chapter": "3", "topic": "Vritham", "page": 57
            })
        );
    }

    #[test]
    fn turn_complete_is_not_interrupted_for_a_normal_turn_end() {
        let json = serde_json::to_value(TurnCompleteMsg {
            kind: "turn_complete",
            seq: Some(42),
            interrupted: false,
        })
        .expect("serialises");

        assert_eq!(
            json,
            serde_json::json!({ "type": "turn_complete", "seq": 42, "interrupted": false })
        );
    }

    #[test]
    fn quota_warning_and_session_end_match_the_documented_shapes() {
        let warning = serde_json::to_value(QuotaWarningMsg {
            kind: "quota_warning",
            remaining_ms: 120_000,
        })
        .expect("serialises");
        assert_eq!(
            warning,
            serde_json::json!({ "type": "quota_warning", "remaining_ms": 120_000 })
        );

        let ended = serde_json::to_value(SessionEndMsg {
            kind: "session_end",
            reason: quota::END_REASON_QUOTA,
        })
        .expect("serialises");
        assert_eq!(
            ended,
            serde_json::json!({ "type": "session_end", "reason": "quota" })
        );
    }

    /// NN-3: the close code and the `end_reason` this socket uses must be the
    /// ledger's own, not a second copy that can drift from it.
    #[test]
    fn quota_close_code_and_end_reason_come_from_the_ledger() {
        assert_eq!(CLOSE_QUOTA, 4003);
        assert_eq!(quota::END_REASON_QUOTA, "quota");
    }
}
