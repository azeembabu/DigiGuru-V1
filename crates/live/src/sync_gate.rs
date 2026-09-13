//! NN-1 enforcement: the `SyncGate`.
//!
//! The state machine is the one in `.claude/rules/whiteboard-sync.md`, verbatim:
//!
//! ```text
//! board_ops(seq=N) received from model
//!   -> forward control frame to client
//!   -> mark turn N PENDING_ACK, start HOLD_MAX timer (400 ms)
//!   -> audio frames for turn N are BUFFERED, not forwarded
//!
//! client board_ack(seq=N)   -> release buffered audio, turn N OPEN
//! HOLD_MAX expires          -> release audio, wb_violation += 1, log turn N
//! client board_error(seq=N) -> release audio, switch to text-fallback board, keep teaching
//! ```
//!
//! There is deliberately **no** "simultaneous release" or fast path. Audio for
//! turn N leaves this gate only through one of those three release edges; a
//! mode that let audio and board ops go out together was proposed and
//! rejected, because it makes NN-1 unprovable — the board would be ahead only
//! by whatever the network happened to do.
//!
//! The gate is a pure, synchronous state machine. It performs no I/O and owns
//! no timer task: it reports the deadline it wants and the caller (the socket
//! task) is responsible for waking it via [`SyncGate::poll_timeouts`]. That is
//! what lets every timing test run instantly against a [`TestClock`].
//!
//! [`TestClock`]: crate::clock::TestClock

use std::collections::{BTreeMap, VecDeque};

use crate::clock::Clock;
use crate::ops::{validate_op, BoardOp, OpError};

/// Monotonic turn counter, one per session.
///
/// A newtype rather than a bare `u32` so a `seq` can never be transposed with
/// a frame count or a byte length in a call to this gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TurnSeq(pub u32);

impl TurnSeq {
    /// The value as it appears on the wire.
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for TurnSeq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

/// The 400 ms hold ceiling from `whiteboard-sync.md`. Exceeding it releases the
/// audio anyway and counts a `wb_violation` — a stalled canvas must never
/// silence the tutor.
pub const HOLD_MAX_MS: u64 = 400;

/// Tuning knobs. The defaults are the documented protocol values; only the
/// buffer sizing is an implementation choice.
#[derive(Debug, Clone, Copy)]
pub struct SyncGateConfig {
    /// Hold ceiling in milliseconds. Defaults to [`HOLD_MAX_MS`].
    pub hold_max_ms: u64,
    /// Per-turn audio buffer ceiling, in frames. At the 20 ms frames of
    /// `realtime-audio.md` the default is 4 s of audio — an order of magnitude
    /// more than the 400 ms hold can legitimately accumulate, so hitting it
    /// means the downstream client has stopped draining.
    pub max_buffered_frames: usize,
    /// How many already-released turns to keep addressable, so late audio for
    /// a turn the client has moved past is still forwarded rather than dropped.
    pub retained_turns: usize,
}

impl Default for SyncGateConfig {
    fn default() -> Self {
        Self {
            hold_max_ms: HOLD_MAX_MS,
            max_buffered_frames: 200,
            retained_turns: 8,
        }
    }
}

/// Errors the gate returns to its caller. None of them is fatal to the socket.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncGateError {
    /// `seq` is monotonic per session (`api-conventions.md`). A repeat or a
    /// step backwards means the upstream turn counter is broken; the ops are
    /// refused rather than silently renumbered.
    #[error("board_ops seq {got} is not monotonic; last accepted was {last}")]
    NonMonotonicSeq { got: TurnSeq, last: TurnSeq },
    /// A `board_ops` frame with every op rejected by the validator. Nothing is
    /// forwarded, so there is no board for the audio to wait behind.
    #[error("board_ops seq {seq} contained no valid ops")]
    NoValidOps { seq: TurnSeq },
}

/// Why a turn's buffered audio was released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseReason {
    /// The client acknowledged the board ops — the normal path.
    BoardAck,
    /// The 400 ms ceiling expired first. Counts a `wb_violation`.
    HoldExpired,
    /// The client could not render the board. Audio is released and the turn
    /// continues in text-fallback mode; teaching does not stop.
    BoardError,
}

/// One op the validator refused. Already logged by the gate; kept on the
/// outcome so a caller can surface it in a trace.
#[derive(Debug, Clone, PartialEq)]
pub struct DroppedOp {
    pub index: usize,
    pub kind: Option<&'static str>,
    pub error: OpError,
}

/// The result of accepting a `board_ops` frame.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardOpsAccepted {
    pub seq: TurnSeq,
    pub clear_first: bool,
    /// The ops that passed validation. **These, and only these, go to the
    /// client** — the caller must forward this vector, not its own input.
    pub ops: Vec<BoardOp>,
    /// Ops that were dropped and logged. Never sent to the client.
    pub dropped: Vec<DroppedOp>,
    /// Monotonic-clock instant at which the hold expires.
    pub hold_deadline_ms: u64,
}

/// What the gate did with one audio frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioDisposition {
    /// Send the frame the caller already holds. No copy, no allocation — this
    /// is the steady-state branch of the per-frame hot loop.
    Forward,
    /// The frame was copied into the turn's hold buffer.
    Buffered { queued_frames: usize },
    /// Buffered, but the buffer was full, so the **oldest** frame was dropped
    /// to make room (`realtime-audio.md` backpressure rule). Logged by the gate.
    BufferedEvictingOldest {
        queued_frames: usize,
        evicted_frames: u64,
    },
    /// No such turn: either the seq was never opened by a `board_ops`, or it
    /// is old enough to have been retired. Dropped, counted, not fatal.
    UnknownTurn,
}

/// Audio handed back to the caller when a turn is released. Frames are in
/// arrival order.
#[derive(Debug, Clone, PartialEq)]
pub struct ReleasedAudio {
    pub seq: TurnSeq,
    pub reason: ReleaseReason,
    /// How long the audio was actually held, in milliseconds. This is the
    /// "SyncGate board hold" segment of the latency budget.
    pub held_ms: u64,
    pub frames: Vec<Vec<u8>>,
}

impl ReleasedAudio {
    /// True when this release degraded the turn to the text-fallback board.
    pub fn is_text_fallback(&self) -> bool {
        self.reason == ReleaseReason::BoardError
    }
}

/// The outcome of a client control message that names a turn.
#[derive(Debug, Clone, PartialEq)]
pub enum AckOutcome {
    /// The turn was holding; here is its audio.
    Released(Box<ReleasedAudio>),
    /// The turn exists but was already released (duplicate ack, or an ack that
    /// lost the race with the hold timer). Ignored, per the forward-compatible
    /// "unknown messages are not fatal" rule.
    AlreadyReleased(TurnSeq),
    /// No such turn — out of order, or retired. Ignored and counted.
    UnknownTurn(TurnSeq),
}

/// Counters the gate exposes for CI gating and tracing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GateMetrics {
    /// **Release blocker.** Non-zero means audio outran an unacknowledged
    /// board at least once (`whiteboard-sync.md`: must be 0 in CI E2E runs).
    pub wb_violation: u64,
    /// Ops refused by the schema validator and never forwarded.
    pub ops_dropped: u64,
    /// Audio frames discarded by backpressure (oldest-first).
    pub frames_evicted: u64,
    /// Frames addressed to a turn the gate does not know about.
    pub frames_unknown_turn: u64,
    /// Turns that ended in text-fallback after a `board_error`.
    pub board_errors: u64,
    /// Control messages naming an unknown turn.
    pub unknown_seq_messages: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnState {
    /// Board ops forwarded, waiting for the ack. Audio is held.
    PendingAck,
    /// Released; audio flows straight through.
    Open,
    /// Released after a `board_error`; audio flows and the client is on the
    /// text-fallback board.
    TextFallback,
}

#[derive(Debug)]
struct Turn {
    state: TurnState,
    forwarded_at_ms: u64,
    deadline_ms: u64,
    buffer: VecDeque<Vec<u8>>,
}

/// The whiteboard-first gate for a single session.
///
/// One gate per socket; it is not `Sync`-shared — the socket task owns it.
#[derive(Debug)]
pub struct SyncGate<C: Clock> {
    clock: C,
    config: SyncGateConfig,
    turns: BTreeMap<u32, Turn>,
    last_seq: Option<TurnSeq>,
    /// Element ids emitted on the board so far, so a `highlight` can be
    /// validated against something real. Cleared by `clear_first`.
    board_ids: Vec<String>,
    /// Recycled frame buffers, so holding audio does not allocate per frame
    /// once the session is warm.
    frame_pool: Vec<Vec<u8>>,
    metrics: GateMetrics,
}

impl<C: Clock> SyncGate<C> {
    /// Build a gate with the protocol defaults.
    pub fn new(clock: C) -> Self {
        Self::with_config(clock, SyncGateConfig::default())
    }

    /// Build a gate with explicit tuning.
    pub fn with_config(clock: C, config: SyncGateConfig) -> Self {
        Self {
            clock,
            config,
            turns: BTreeMap::new(),
            last_seq: None,
            board_ids: Vec::new(),
            frame_pool: Vec::new(),
            metrics: GateMetrics::default(),
        }
    }

    /// Current counters. `wb_violation` is the one CI gates on.
    pub fn metrics(&self) -> GateMetrics {
        self.metrics
    }

    /// Shorthand for `metrics().wb_violation`.
    pub fn wb_violation(&self) -> u64 {
        self.metrics.wb_violation
    }

    /// The highest `seq` accepted so far, if any.
    pub fn last_seq(&self) -> Option<TurnSeq> {
        self.last_seq
    }

    /// True while turn `seq` is holding audio.
    pub fn is_holding(&self, seq: TurnSeq) -> bool {
        matches!(
            self.turns.get(&seq.0).map(|t| t.state),
            Some(TurnState::PendingAck)
        )
    }

    /// True when turn `seq` ended up on the text-fallback board.
    pub fn is_text_fallback(&self, seq: TurnSeq) -> bool {
        matches!(
            self.turns.get(&seq.0).map(|t| t.state),
            Some(TurnState::TextFallback)
        )
    }

    /// The earliest hold deadline still outstanding, as a monotonic-clock
    /// instant. The socket task sleeps until this and then calls
    /// [`SyncGate::poll_timeouts`]; `None` means nothing is holding.
    pub fn next_hold_deadline_ms(&self) -> Option<u64> {
        self.turns
            .values()
            .filter(|t| t.state == TurnState::PendingAck)
            .map(|t| t.deadline_ms)
            .min()
    }

    /// Accept a `board_ops` frame from the model.
    ///
    /// Validates every op, drops and logs the invalid ones, and opens turn
    /// `seq` in `PENDING_ACK` with the hold timer started. The returned
    /// [`BoardOpsAccepted::ops`] is what the caller forwards to the client.
    ///
    /// From this point until a release edge, [`SyncGate::push_audio`] for this
    /// `seq` buffers instead of forwarding. That ordering is the whole of NN-1.
    pub fn on_board_ops(
        &mut self,
        seq: TurnSeq,
        clear_first: bool,
        ops: Vec<BoardOp>,
    ) -> Result<BoardOpsAccepted, SyncGateError> {
        if let Some(last) = self.last_seq {
            if seq <= last {
                tracing::warn!(seq = %seq, last = %last, "board_ops with non-monotonic seq refused");
                return Err(SyncGateError::NonMonotonicSeq { got: seq, last });
            }
        }

        // `clear_first` wipes the board, so previously emitted element ids are
        // no longer valid highlight targets.
        if clear_first {
            self.board_ids.clear();
        }

        let mut accepted = Vec::with_capacity(ops.len());
        let mut dropped = Vec::new();
        for (index, op) in ops.into_iter().enumerate() {
            match validate_op(&op, &self.board_ids) {
                Ok(()) => {
                    if let Some(id) = op.declared_id() {
                        self.board_ids.push(id.to_owned());
                    }
                    accepted.push(op);
                }
                Err(error) => {
                    self.metrics.ops_dropped += 1;
                    tracing::warn!(
                        seq = %seq,
                        index,
                        kind = op.kind(),
                        %error,
                        "board op failed schema validation; dropped, not forwarded"
                    );
                    dropped.push(DroppedOp {
                        index,
                        kind: Some(op.kind()),
                        error,
                    });
                }
            }
        }

        if accepted.is_empty() {
            // Nothing reaches the client, so nothing can be acked. Opening a
            // hold here would strand the turn's audio for the full 400 ms and
            // bank a wb_violation for a fault that is entirely upstream.
            tracing::error!(seq = %seq, dropped = dropped.len(), "board_ops had no valid ops");
            return Err(SyncGateError::NoValidOps { seq });
        }

        let now = self.clock.now_ms();
        let deadline = now.saturating_add(self.config.hold_max_ms);
        self.turns.insert(
            seq.0,
            Turn {
                state: TurnState::PendingAck,
                forwarded_at_ms: now,
                deadline_ms: deadline,
                buffer: VecDeque::new(),
            },
        );
        self.last_seq = Some(seq);
        self.retire_old_turns();

        Ok(BoardOpsAccepted {
            seq,
            clear_first,
            ops: accepted,
            dropped,
            hold_deadline_ms: deadline,
        })
    }

    /// Offer one audio frame for turn `seq`.
    ///
    /// Returns [`AudioDisposition::Forward`] only when the turn has already
    /// been released. While the turn holds, the frame is copied into the hold
    /// buffer and the caller must not send it.
    pub fn push_audio(&mut self, seq: TurnSeq, frame: &[u8]) -> AudioDisposition {
        let max_frames = self.config.max_buffered_frames;
        let Some(turn) = self.turns.get_mut(&seq.0) else {
            self.metrics.frames_unknown_turn += 1;
            tracing::debug!(seq = %seq, "audio frame for unknown or retired turn; dropped");
            return AudioDisposition::UnknownTurn;
        };

        match turn.state {
            // Hot path: no lookup beyond the map, no copy, no allocation.
            TurnState::Open | TurnState::TextFallback => AudioDisposition::Forward,
            TurnState::PendingAck => {
                let mut evicted = false;
                if turn.buffer.len() >= max_frames {
                    // Backpressure (`realtime-audio.md`): drop the OLDEST
                    // buffered audio rather than growing unbounded.
                    if let Some(old) = turn.buffer.pop_front() {
                        self.frame_pool.push(old);
                    }
                    evicted = true;
                }
                let mut buf = self.frame_pool.pop().unwrap_or_default();
                buf.clear();
                buf.extend_from_slice(frame);
                turn.buffer.push_back(buf);
                let queued_frames = turn.buffer.len();

                if evicted {
                    self.metrics.frames_evicted += 1;
                    tracing::warn!(
                        seq = %seq,
                        queued_frames,
                        total_evicted = self.metrics.frames_evicted,
                        "audio hold buffer full; dropped oldest frame"
                    );
                    AudioDisposition::BufferedEvictingOldest {
                        queued_frames,
                        evicted_frames: self.metrics.frames_evicted,
                    }
                } else {
                    AudioDisposition::Buffered { queued_frames }
                }
            }
        }
    }

    /// `{"type":"board_ack","seq":N}` — the client rendered the board. Release
    /// the held audio and open turn N.
    pub fn on_board_ack(&mut self, seq: TurnSeq) -> AckOutcome {
        self.release(seq, ReleaseReason::BoardAck)
    }

    /// `{"type":"board_error","seq":N,"reason":...}` — the canvas failed.
    /// Release the audio, switch the turn to the text-fallback board, keep
    /// teaching. A canvas exception must never kill the voice stream.
    pub fn on_board_error(&mut self, seq: TurnSeq, reason: &str) -> AckOutcome {
        let outcome = self.release(seq, ReleaseReason::BoardError);
        if matches!(outcome, AckOutcome::Released(_)) {
            self.metrics.board_errors += 1;
            tracing::warn!(seq = %seq, reason, "client board_error; degrading turn to text fallback");
        }
        outcome
    }

    /// Release every turn whose 400 ms hold has expired.
    ///
    /// Each release counts a `wb_violation` and is logged with the turn seq, as
    /// the spec requires. Call this whenever the clock reaches
    /// [`SyncGate::next_hold_deadline_ms`].
    pub fn poll_timeouts(&mut self) -> Vec<ReleasedAudio> {
        let now = self.clock.now_ms();
        let expired: Vec<u32> = self
            .turns
            .iter()
            .filter(|(_, t)| t.state == TurnState::PendingAck && t.deadline_ms <= now)
            .map(|(seq, _)| *seq)
            .collect();

        let mut released = Vec::with_capacity(expired.len());
        for seq in expired {
            if let AckOutcome::Released(audio) = self.release(TurnSeq(seq), ReleaseReason::HoldExpired)
            {
                self.metrics.wb_violation += 1;
                tracing::error!(
                    seq,
                    held_ms = audio.held_ms,
                    frames = audio.frames.len(),
                    wb_violation = self.metrics.wb_violation,
                    "HOLD_MAX expired before board_ack; releasing audio (NN-1 violation)"
                );
                released.push(*audio);
            }
        }
        released
    }

    /// Hand released frame buffers back so they can be refilled instead of
    /// reallocated. Purely an optimisation; skipping it is always safe.
    pub fn recycle(&mut self, frames: Vec<Vec<u8>>) {
        for frame in frames {
            if self.frame_pool.len() < self.config.max_buffered_frames {
                self.frame_pool.push(frame);
            }
        }
    }

    fn release(&mut self, seq: TurnSeq, reason: ReleaseReason) -> AckOutcome {
        let now = self.clock.now_ms();
        let Some(turn) = self.turns.get_mut(&seq.0) else {
            self.metrics.unknown_seq_messages += 1;
            tracing::debug!(seq = %seq, ?reason, "control message for unknown or retired turn; ignored");
            return AckOutcome::UnknownTurn(seq);
        };
        if turn.state != TurnState::PendingAck {
            tracing::debug!(seq = %seq, ?reason, "turn already released; ignored");
            return AckOutcome::AlreadyReleased(seq);
        }

        turn.state = match reason {
            ReleaseReason::BoardError => TurnState::TextFallback,
            _ => TurnState::Open,
        };
        let held_ms = now.saturating_sub(turn.forwarded_at_ms);
        let frames: Vec<Vec<u8>> = turn.buffer.drain(..).collect();
        AckOutcome::Released(Box::new(ReleasedAudio {
            seq,
            reason,
            held_ms,
            frames,
        }))
    }

    /// Keep the turn map bounded. Only already-released turns are retired — a
    /// turn that is still holding audio is never dropped on the floor, because
    /// that audio would be lost rather than released.
    fn retire_old_turns(&mut self) {
        let retired: Vec<u32> = {
            let releasable: Vec<u32> = self
                .turns
                .iter()
                .filter(|(_, t)| t.state != TurnState::PendingAck)
                .map(|(seq, _)| *seq)
                .collect();
            let excess = releasable.len().saturating_sub(self.config.retained_turns);
            releasable.into_iter().take(excess).collect()
        };
        for seq in retired {
            if let Some(mut turn) = self.turns.remove(&seq) {
                for frame in turn.buffer.drain(..) {
                    if self.frame_pool.len() < self.config.max_buffered_frames {
                        self.frame_pool.push(frame);
                    }
                }
            }
        }
    }
}
