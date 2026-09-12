//! `live`'s own error type (per `.claude/rules/code-style.md`: "every crate
//! exposes its own `Error` enum"), plus the conversion into
//! `dg_core::PublicError`.
//!
//! `Display` retains detail for internal logging (invalid board-op payloads,
//! Gemini Live client errors) — none of that is ever safe to show a student
//! verbatim, so the `From` impl below collapses every variant to a single
//! opaque `PublicError::Internal`/`PublicError::Unavailable`.

#[derive(Debug, thiserror::Error)]
pub enum LiveError {
    /// A `board_ops` message failed schema validation
    /// (`.claude/rules/whiteboard-sync.md`: "An invalid op is dropped and
    /// logged; it never reaches the client.").
    #[error("invalid board op: {0}")]
    InvalidBoardOp(String),

    /// Malformed JSON on a control frame, or a `board_ops` message missing
    /// the required `type: "board_ops"` discriminator.
    #[error("malformed message: {0}")]
    Malformed(String),

    /// The Gemini Live upstream (audio/tool-call channel) failed or is
    /// unreachable.
    #[error("live session error: {0}")]
    Session(String),

    /// A caller referenced a turn `seq` the `SyncGate` has no record of in a
    /// context that requires one to exist.
    #[error("unknown turn seq: {0}")]
    UnknownTurn(u32),
}

/// Never leak validation internals or upstream client errors to a client
/// (`CLAUDE.md` working agreements). `Session` failures map to `Unavailable`
/// since Gemini Live is an upstream dependency; everything else maps to a
/// generic internal error.
impl From<LiveError> for dg_core::PublicError {
    fn from(err: LiveError) -> Self {
        match err {
            LiveError::Session(detail) => {
                tracing::error!(detail = %detail, "gemini live session error");
                dg_core::PublicError::Unavailable
            }
            other => {
                tracing::error!(detail = %other, "live pipeline error");
                dg_core::PublicError::Internal
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, LiveError>;
