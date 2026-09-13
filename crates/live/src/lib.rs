//! `live` — the whiteboard board-op protocol and the `SyncGate`.
//!
//! Owns the whiteboard-first non-negotiable (NN-1): audio for turn N is
//! buffered until the client ACKs turn N's board ops, or the 400 ms hold
//! ceiling expires. Enforcement is here, in the gateway, not in the prompt —
//! the model cannot outrun the board because its audio is physically held.
//!
//! The crate is deliberately pure: no sockets, no Gemini client, no timers of
//! its own. [`SyncGate`] is a synchronous state machine over an injected
//! [`Clock`], so every hold-timer test runs instantly and without sleeps.
//!
//! ```
//! use live::{BoardOp, SyncGate, TestClock, TurnSeq, AudioDisposition, AckOutcome};
//!
//! let clock = TestClock::new();
//! let mut gate = SyncGate::new(clock.clone());
//!
//! let ops = vec![BoardOp::Heading { id: "h1".into(), text: "Sandhi".into() }];
//! let accepted = gate.on_board_ops(TurnSeq(1), true, ops)?;
//! // forward `accepted.ops` to the client, then:
//! assert!(matches!(gate.push_audio(TurnSeq(1), &[0u8; 640]),
//!                  AudioDisposition::Buffered { .. }));
//! assert!(matches!(gate.on_board_ack(TurnSeq(1)), AckOutcome::Released(_)));
//! assert!(matches!(gate.push_audio(TurnSeq(1), &[0u8; 640]),
//!                  AudioDisposition::Forward));
//! # Ok::<(), live::SyncGateError>(())
//! ```

pub mod clock;
pub mod ops;
pub mod sync_gate;

pub use clock::{Clock, SystemClock, TestClock};
pub use ops::{parse_op, validate_op, BoardOp, DrawShape, OpError, Point};
pub use sync_gate::{
    AckOutcome, AudioDisposition, BoardOpsAccepted, DroppedOp, GateMetrics, ReleaseReason,
    ReleasedAudio, SyncGate, SyncGateConfig, SyncGateError, TurnSeq, HOLD_MAX_MS,
};

// ---------------------------------------------------------------------------
// Merged from main (the other developer's parallel Phase 3 pass).
//
// Both branches implemented NN-1 independently. `sync_gate` here is the
// injected-clock version with the fuller test suite; the Gemini client, turn
// FSM and legacy board schema below come from main and are kept, since nothing
// in this crate duplicates them.
//
// `board::BoardOp` and `ops::BoardOp` are two different types for the same
// protocol. `ops` is the one `SyncGate` validates against and the one exported
// at the crate root; `board` stays reachable by path for `gemini_client`, and
// is NOT re-exported, so a caller cannot pick the wrong `BoardOp` by accident.
// Collapsing the two is a follow-up, not a merge-time change.
// ---------------------------------------------------------------------------

pub mod board;
pub mod error;
pub mod gemini_client;
pub mod turn_fsm;

pub use error::LiveError;
pub use gemini_client::{
    GeminiLiveSessionClient, LiveModelEvent, LiveSessionClient, StubLiveSessionClient,
};
pub use turn_fsm::TurnPhase;
