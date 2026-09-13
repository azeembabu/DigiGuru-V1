//! Lightweight per-turn lifecycle FSM for Phase 3's audio/board sync needs.
//!
//! This is deliberately narrow: `SyncGate` already carries the real NN-1
//! state (pending-ack / open / text-fallback per turn). `TurnPhase` is just
//! a coarse label the WebSocket handler can use to track overall turn
//! progress (e.g. for logging/tracing spans), independent of `SyncGate`'s
//! internal bookkeeping.
//!
//! The full session-level FSM — quota states, idle timeout, jailbreak halt —
//! is `IMPLEMENTATION_PLAN.md` §7.1 / `.claude/rules/security.md` territory
//! and is Phase 4 scope, not this module's.
//!
//! Phase 4 extends this: a session-level FSM will wrap per-turn phases like
//! this one inside states such as `Active`, `IdleWarning`, `QuotaExceeded`,
//! `JailbreakHalt`.

/// A turn's coarse lifecycle phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnPhase {
    /// Waiting for the model to produce its `board_ops` tool call and/or
    /// audio for this turn.
    AwaitingModel,
    /// `board_ops` has been sent to the client; the turn is held per NN-1
    /// until ack or hold-timeout (mirrors `SyncGate`'s `PendingAck`).
    BoardPending,
    /// Audio for the turn is flowing to the client.
    Open,
    /// The turn has finished (model signalled turn-complete, or the session
    /// ended).
    Closed,
}

impl TurnPhase {
    /// Whether transitioning from `self` to `to` is a valid forward step.
    /// The lifecycle is strictly linear and forward-only — there is no
    /// going back from `Open` to `BoardPending` within the same turn.
    pub fn can_transition(&self, to: &TurnPhase) -> bool {
        matches!(
            (self, to),
            (TurnPhase::AwaitingModel, TurnPhase::BoardPending)
                | (TurnPhase::AwaitingModel, TurnPhase::Open)
                | (TurnPhase::BoardPending, TurnPhase::Open)
                | (TurnPhase::Open, TurnPhase::Closed)
                | (TurnPhase::BoardPending, TurnPhase::Closed)
                | (TurnPhase::AwaitingModel, TurnPhase::Closed)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_transitions_are_allowed() {
        assert!(TurnPhase::AwaitingModel.can_transition(&TurnPhase::BoardPending));
        assert!(TurnPhase::BoardPending.can_transition(&TurnPhase::Open));
        assert!(TurnPhase::Open.can_transition(&TurnPhase::Closed));
    }

    #[test]
    fn skipping_board_pending_is_allowed_when_no_board_op_is_emitted() {
        // Not every turn necessarily emits board_ops (e.g. a pure
        // clarification question); AwaitingModel -> Open directly is valid.
        assert!(TurnPhase::AwaitingModel.can_transition(&TurnPhase::Open));
    }

    #[test]
    fn backward_transitions_are_rejected() {
        assert!(!TurnPhase::Open.can_transition(&TurnPhase::BoardPending));
        assert!(!TurnPhase::Closed.can_transition(&TurnPhase::Open));
        assert!(!TurnPhase::BoardPending.can_transition(&TurnPhase::AwaitingModel));
    }

    #[test]
    fn closed_is_terminal() {
        assert!(!TurnPhase::Closed.can_transition(&TurnPhase::AwaitingModel));
        assert!(!TurnPhase::Closed.can_transition(&TurnPhase::BoardPending));
        assert!(!TurnPhase::Closed.can_transition(&TurnPhase::Closed));
    }
}
