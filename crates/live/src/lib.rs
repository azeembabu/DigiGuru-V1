//! `live` — Gemini Live client, `SyncGate`, and the whiteboard board-op
//! protocol. Owns the whiteboard-first enforcement (NN-1): audio for turn N
//! is buffered until the client ACKs turn N's board ops, or the 400 ms hold
//! ceiling expires.

pub mod board;
pub mod error;
pub mod gemini_client;
pub mod sync_gate;
pub mod turn_fsm;

pub use board::{BoardOp, BoardOpsMessage};
pub use error::LiveError;
pub use gemini_client::{
    GeminiLiveSessionClient, LiveModelEvent, LiveSessionClient, StubLiveSessionClient,
};
pub use sync_gate::{SyncGate, SyncGateAction, HOLD_MAX_MS};
pub use turn_fsm::TurnPhase;
