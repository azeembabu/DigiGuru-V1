//! `SyncGate` — the NN-1 whiteboard-first enforcement state machine
//! (`IMPLEMENTATION_PLAN.md` §6.2, `.claude/rules/whiteboard-sync.md`).
//!
//! This is a **pure, synchronous** state machine: it owns no tokio task, no
//! timer, and no socket. The caller (the gateway's WebSocket handler) drives
//! it by calling the `on_*` methods when the corresponding event happens,
//! and is responsible for acting on the returned `SyncGateAction` — actually
//! starting/spawning the 400 ms hold timer, forwarding buffered frames to
//! the client, and bumping the `wb_violation` metric. Keeping all of that
//! out of this module is what makes it exhaustively unit-testable without a
//! real runtime.
//!
//! ## Timer contract
//!
//! `on_board_ops` tells the caller to start a `HOLD_MAX_MS` timer for a
//! given `seq`. This module does **not** cancel that timer when an ACK
//! arrives first — timer cancellation is the caller's job (e.g. dropping a
//! `tokio::time::Sleep` or ignoring an `AbortHandle`). Consequently the
//! caller may end up invoking `on_hold_timeout(seq)` *after* `on_board_ack`
//! already fired for that same `seq` (a "stale" timer). `on_hold_timeout`
//! is written to be safe to call in that case: it only acts if the turn is
//! still `PendingAck`, and returns `SyncGateAction::NoOp` otherwise, so a
//! stale timer can never double-release buffered audio or double-count a
//! violation.

use bytes::Bytes;
use std::collections::HashMap;

/// `.claude/rules/whiteboard-sync.md`: "HOLD_MAX expires -> release audio,
/// wb_violation += 1, log turn N".
pub const HOLD_MAX_MS: u64 = 400;

/// Per-turn state tracked by `SyncGate`.
#[derive(Debug, Clone, PartialEq)]
enum TurnState {
    /// `board_ops` sent to the client, waiting for `board_ack` or the hold
    /// timer to expire. Audio for this turn is buffered here, in arrival
    /// order.
    PendingAck { audio_buffer: Vec<Bytes> },
    /// The board op for this turn was acknowledged (or its hold expired) —
    /// audio for this turn forwards immediately from here on.
    Open,
    /// The client reported a render exception for this turn
    /// (`board_error`); the board has degraded to text-only, but per
    /// `.claude/rules/whiteboard-sync.md` "a canvas exception must never
    /// kill the voice stream" — audio still forwards immediately.
    TextFallback,
}

/// What the caller must do in response to a `SyncGate` event. `SyncGate`
/// never performs I/O or timer scheduling itself — every variant here is an
/// instruction for the caller to carry out.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncGateAction {
    /// Start (or spawn) a hold timer of `hold_ms` for `seq`. When it fires,
    /// the caller must call `on_hold_timeout(seq)` — regardless of whether
    /// an ack has already been seen; see the timer contract above.
    StartHoldTimer { seq: u32, hold_ms: u64 },
    /// The frame was buffered; the caller must not forward it yet.
    Buffered,
    /// The frame must be forwarded to the client immediately.
    ForwardImmediately(Bytes),
    /// The turn is now `Open` (ack arrived before the hold timer). Forward
    /// every buffered frame, in order, with no violation.
    ReleaseBuffered(Vec<Bytes>),
    /// The hold timer expired before an ack arrived. Forward every buffered
    /// frame, in order, AND the caller must increment `wb_violation` and log
    /// the turn (`.claude/rules/whiteboard-sync.md`).
    ReleaseBufferedWithViolation(Vec<Bytes>),
    /// Nothing to do — e.g. a stale hold-timeout firing after the turn was
    /// already resolved by an ack. Must not release anything a second time.
    NoOp,
}

/// The NN-1 state machine. One `SyncGate` per live session.
#[derive(Debug, Default)]
pub struct SyncGate {
    turns: HashMap<u32, TurnState>,
}

impl SyncGate {
    pub fn new() -> Self {
        Self {
            turns: HashMap::new(),
        }
    }

    /// `board_ops(seq=N)` received from the model and validated: mark turn
    /// `N` `PENDING_ACK` and tell the caller to start the hold timer.
    pub fn on_board_ops(&mut self, seq: u32) -> SyncGateAction {
        self.turns.insert(
            seq,
            TurnState::PendingAck {
                audio_buffer: Vec::new(),
            },
        );
        SyncGateAction::StartHoldTimer {
            seq,
            hold_ms: HOLD_MAX_MS,
        }
    }

    /// An audio frame arrived for turn `seq`.
    ///
    /// - `PendingAck` -> buffered, not forwarded (this is the NN-1 hold).
    /// - `Open` -> forwarded immediately.
    /// - `TextFallback` -> forwarded immediately (audio must keep teaching
    ///   even in degraded-board mode).
    /// - Unknown turn (no `board_ops` was ever announced for it, or it
    ///   predates this `SyncGate`'s lifetime) -> forwarded immediately,
    ///   fail-open. NN-1 only holds audio for a turn that *has* an
    ///   announced visual; it is not a general audio kill-switch.
    pub fn buffer_audio(&mut self, seq: u32, frame: Bytes) -> SyncGateAction {
        match self.turns.get_mut(&seq) {
            Some(TurnState::PendingAck { audio_buffer }) => {
                audio_buffer.push(frame);
                SyncGateAction::Buffered
            }
            Some(TurnState::Open) | Some(TurnState::TextFallback) | None => {
                SyncGateAction::ForwardImmediately(frame)
            }
        }
    }

    /// `client board_ack(seq=N)` -> release buffered audio, turn `N` `OPEN`.
    pub fn on_board_ack(&mut self, seq: u32) -> SyncGateAction {
        match self.turns.get(&seq) {
            Some(TurnState::PendingAck { .. }) => {
                let buffered = self.take_buffer(seq);
                self.turns.insert(seq, TurnState::Open);
                SyncGateAction::ReleaseBuffered(buffered)
            }
            _ => SyncGateAction::NoOp,
        }
    }

    /// The `HOLD_MAX_MS` timer for `seq` fired. Per the timer contract
    /// documented on this module, this may be a stale firing if
    /// `on_board_ack` or `on_board_error` already resolved the turn — in
    /// that case this is a no-op.
    pub fn on_hold_timeout(&mut self, seq: u32) -> SyncGateAction {
        match self.turns.get(&seq) {
            Some(TurnState::PendingAck { .. }) => {
                let buffered = self.take_buffer(seq);
                self.turns.insert(seq, TurnState::Open);
                SyncGateAction::ReleaseBufferedWithViolation(buffered)
            }
            _ => SyncGateAction::NoOp,
        }
    }

    /// `client board_error(seq=N)` -> release buffered audio, switch to
    /// text-fallback board, keep teaching.
    pub fn on_board_error(&mut self, seq: u32) -> SyncGateAction {
        match self.turns.get(&seq) {
            Some(TurnState::PendingAck { .. }) => {
                let buffered = self.take_buffer(seq);
                self.turns.insert(seq, TurnState::TextFallback);
                SyncGateAction::ReleaseBuffered(buffered)
            }
            _ => {
                // Already open/fallback/unknown: nothing buffered to
                // release, but still record the fallback so future audio on
                // this turn is treated consistently.
                self.turns.insert(seq, TurnState::TextFallback);
                SyncGateAction::ReleaseBuffered(Vec::new())
            }
        }
    }

    /// Clears all turn state. Called on session start/reconnect.
    pub fn reset_session(&mut self) {
        self.turns.clear();
    }

    /// Takes the buffered frames for `seq` out of the map, leaving an empty
    /// `Vec` behind if the turn wasn't `PendingAck` (should not happen given
    /// the call sites above, but keeps this helper total).
    fn take_buffer(&mut self, seq: u32) -> Vec<Bytes> {
        match self.turns.get_mut(&seq) {
            Some(TurnState::PendingAck { audio_buffer }) => std::mem::take(audio_buffer),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(byte: u8) -> Bytes {
        Bytes::from(vec![byte])
    }

    #[test]
    fn ack_before_timeout_releases_buffered_audio_with_no_violation() {
        let mut gate = SyncGate::new();

        let action = gate.on_board_ops(1);
        assert_eq!(
            action,
            SyncGateAction::StartHoldTimer {
                seq: 1,
                hold_ms: HOLD_MAX_MS
            }
        );

        assert_eq!(gate.buffer_audio(1, frame(1)), SyncGateAction::Buffered);
        assert_eq!(gate.buffer_audio(1, frame(2)), SyncGateAction::Buffered);

        let action = gate.on_board_ack(1);
        assert_eq!(
            action,
            SyncGateAction::ReleaseBuffered(vec![frame(1), frame(2)])
        );

        // Turn is now Open: subsequent audio forwards immediately.
        assert_eq!(
            gate.buffer_audio(1, frame(3)),
            SyncGateAction::ForwardImmediately(frame(3))
        );
    }

    #[test]
    fn timeout_before_ack_releases_with_violation() {
        let mut gate = SyncGate::new();
        gate.on_board_ops(2);
        gate.buffer_audio(2, frame(9));

        let action = gate.on_hold_timeout(2);
        assert_eq!(
            action,
            SyncGateAction::ReleaseBufferedWithViolation(vec![frame(9)])
        );

        // Turn is now Open: a late ack for the same seq is a no-op (nothing
        // left to release).
        assert_eq!(gate.on_board_ack(2), SyncGateAction::NoOp);
    }

    #[test]
    fn board_error_releases_and_enters_text_fallback_keeping_audio_flowing() {
        let mut gate = SyncGate::new();
        gate.on_board_ops(3);
        gate.buffer_audio(3, frame(5));

        let action = gate.on_board_error(3);
        assert_eq!(action, SyncGateAction::ReleaseBuffered(vec![frame(5)]));

        // Text-fallback: audio keeps forwarding immediately, never buffered.
        assert_eq!(
            gate.buffer_audio(3, frame(6)),
            SyncGateAction::ForwardImmediately(frame(6))
        );
    }

    #[test]
    fn stale_timeout_after_ack_is_noop_and_does_not_double_release() {
        let mut gate = SyncGate::new();
        gate.on_board_ops(4);
        gate.buffer_audio(4, frame(1));

        let ack_action = gate.on_board_ack(4);
        assert_eq!(ack_action, SyncGateAction::ReleaseBuffered(vec![frame(1)]));

        // The 400ms timer for turn 4 fires late, after the ack already
        // resolved it. Must be a no-op — no second release, no violation.
        let stale = gate.on_hold_timeout(4);
        assert_eq!(stale, SyncGateAction::NoOp);
    }

    #[test]
    fn stale_timeout_after_board_error_is_noop() {
        let mut gate = SyncGate::new();
        gate.on_board_ops(5);
        gate.buffer_audio(5, frame(1));
        gate.on_board_error(5);

        let stale = gate.on_hold_timeout(5);
        assert_eq!(stale, SyncGateAction::NoOp);
    }

    #[test]
    fn audio_for_unknown_turn_forwards_immediately_fail_open() {
        let mut gate = SyncGate::new();
        // No on_board_ops(42) was ever called.
        assert_eq!(
            gate.buffer_audio(42, frame(7)),
            SyncGateAction::ForwardImmediately(frame(7))
        );
    }

    #[test]
    fn reset_session_clears_all_turn_state() {
        let mut gate = SyncGate::new();
        gate.on_board_ops(1);
        gate.buffer_audio(1, frame(1));
        gate.reset_session();

        // Turn 1 is now unknown again: fail-open forwards immediately
        // instead of buffering.
        assert_eq!(
            gate.buffer_audio(1, frame(2)),
            SyncGateAction::ForwardImmediately(frame(2))
        );
    }

    #[test]
    fn multiple_turns_are_tracked_independently() {
        let mut gate = SyncGate::new();
        gate.on_board_ops(1);
        gate.on_board_ops(2);
        gate.buffer_audio(1, frame(1));
        gate.buffer_audio(2, frame(2));

        // Acking turn 1 must not affect turn 2's buffered audio.
        assert_eq!(
            gate.on_board_ack(1),
            SyncGateAction::ReleaseBuffered(vec![frame(1)])
        );
        assert_eq!(gate.buffer_audio(2, frame(3)), SyncGateAction::Buffered);
        assert_eq!(
            gate.on_hold_timeout(2),
            SyncGateAction::ReleaseBufferedWithViolation(vec![frame(2), frame(3)])
        );
    }
}
