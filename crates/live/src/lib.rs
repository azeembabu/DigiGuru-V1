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
