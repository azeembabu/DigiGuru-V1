//! Gemini Live bidirectional audio API surface (`IMPLEMENTATION_PLAN.md`
//! §6.1, §6.2). Kept deliberately minimal: this crate only needs to send
//! audio frames upstream and receive a small set of model events
//! (board-op tool calls, audio chunks, turn/session lifecycle signals).
//!
//! There is no `GEMINI_API_KEY` available in this environment (mirrors
//! `crates/rag/src/embed.rs`'s `GeminiEmbedder` stub pattern), so the real
//! network client is not implemented here — only the trait boundary and a
//! scripted stub for tests and for the other machine's WebSocket handler to
//! develop against without live connectivity.

use crate::board::BoardOpsMessage;
use crate::error::{LiveError, Result};
use bytes::Bytes;

/// A single event the model/session can produce, as observed by the
/// gateway's WebSocket handler.
#[derive(Debug, Clone, PartialEq)]
pub enum LiveModelEvent {
    /// The model called the `board_ops` tool (`IMPLEMENTATION_PLAN.md`
    /// §6.2) — already parsed and schema-validated (see `board::validate`).
    BoardOps(BoardOpsMessage),
    /// A chunk of synthesized speech audio for the current turn.
    AudioChunk(Bytes),
    /// The model has finished its current turn.
    TurnComplete,
    /// The Live session ended (model-initiated, or upstream closed).
    SessionEnded,
}

/// The Gemini Live session surface Phase 3 needs. Implementors own the
/// actual transport (WebSocket to the Live API, in the real client); callers
/// drive it by pushing audio frames in and polling events out.
#[async_trait::async_trait]
pub trait LiveSessionClient: Send {
    /// Send one frame of captured student audio upstream.
    async fn send_audio_frame(&mut self, frame: Bytes) -> Result<()>;

    /// Poll for the next model event. Implementations should block
    /// (asynchronously) until an event is available, an error occurs, or the
    /// session has ended — at which point every subsequent call returns
    /// `Ok(LiveModelEvent::SessionEnded)`.
    async fn poll_event(&mut self) -> Result<LiveModelEvent>;
}

/// Real Gemini Live API client. Not wired up yet — there is no
/// `GEMINI_API_KEY` available in this environment.
pub struct GeminiLiveSessionClient {
    pub api_key: String,
}

#[async_trait::async_trait]
impl LiveSessionClient for GeminiLiveSessionClient {
    async fn send_audio_frame(&mut self, _frame: Bytes) -> Result<()> {
        // TODO(phase3-gemini-api): wire the real Gemini Live SDK/WebSocket
        // client once an API key and SDK are available.
        Err(LiveError::Session(
            "GeminiLiveSessionClient is not implemented until GEMINI_API_KEY is available"
                .to_string(),
        ))
    }

    async fn poll_event(&mut self) -> Result<LiveModelEvent> {
        // TODO(phase3-gemini-api): wire the real Gemini Live SDK/WebSocket
        // client once an API key and SDK are available.
        Err(LiveError::Session(
            "GeminiLiveSessionClient is not implemented until GEMINI_API_KEY is available"
                .to_string(),
        ))
    }
}

/// A scripted, in-memory `LiveSessionClient` for tests and for downstream
/// development (e.g. the gateway's WebSocket handler) without real Gemini
/// connectivity. Pre-load it with the exact sequence of events it should
/// yield; `poll_event` returns them in order, then `SessionEnded` forever
/// after the script is exhausted.
pub struct StubLiveSessionClient {
    script: std::collections::VecDeque<LiveModelEvent>,
    sent_frames: Vec<Bytes>,
}

impl StubLiveSessionClient {
    pub fn new(script: Vec<LiveModelEvent>) -> Self {
        Self {
            script: script.into(),
            sent_frames: Vec::new(),
        }
    }

    /// Every audio frame handed to `send_audio_frame` so far, in order —
    /// lets a test assert on what the (fake) upstream received.
    pub fn sent_frames(&self) -> &[Bytes] {
        &self.sent_frames
    }
}

#[async_trait::async_trait]
impl LiveSessionClient for StubLiveSessionClient {
    async fn send_audio_frame(&mut self, frame: Bytes) -> Result<()> {
        self.sent_frames.push(frame);
        Ok(())
    }

    async fn poll_event(&mut self) -> Result<LiveModelEvent> {
        Ok(self
            .script
            .pop_front()
            .unwrap_or(LiveModelEvent::SessionEnded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::BoardOp;

    fn sample_board_ops() -> BoardOpsMessage {
        BoardOpsMessage {
            msg_type: "board_ops".to_string(),
            seq: 1,
            clear_first: false,
            ops: vec![BoardOp::Heading {
                text: "Intro".to_string(),
                page: 1,
            }],
        }
    }

    #[tokio::test]
    async fn stub_yields_scripted_events_in_order_then_session_ended() {
        let mut client = StubLiveSessionClient::new(vec![
            LiveModelEvent::BoardOps(sample_board_ops()),
            LiveModelEvent::AudioChunk(Bytes::from_static(b"audio")),
            LiveModelEvent::TurnComplete,
        ]);

        assert_eq!(
            client.poll_event().await.unwrap(),
            LiveModelEvent::BoardOps(sample_board_ops())
        );
        assert_eq!(
            client.poll_event().await.unwrap(),
            LiveModelEvent::AudioChunk(Bytes::from_static(b"audio"))
        );
        assert_eq!(client.poll_event().await.unwrap(), LiveModelEvent::TurnComplete);
        assert_eq!(client.poll_event().await.unwrap(), LiveModelEvent::SessionEnded);
        // Keeps returning SessionEnded, not panicking, once exhausted.
        assert_eq!(client.poll_event().await.unwrap(), LiveModelEvent::SessionEnded);
    }

    #[tokio::test]
    async fn stub_records_sent_audio_frames() {
        let mut client = StubLiveSessionClient::new(vec![]);
        client
            .send_audio_frame(Bytes::from_static(b"frame1"))
            .await
            .unwrap();
        client
            .send_audio_frame(Bytes::from_static(b"frame2"))
            .await
            .unwrap();
        assert_eq!(
            client.sent_frames(),
            &[Bytes::from_static(b"frame1"), Bytes::from_static(b"frame2")]
        );
    }

    #[tokio::test]
    async fn real_client_returns_session_error_without_api_key_wiring() {
        let mut client = GeminiLiveSessionClient {
            api_key: "unused".to_string(),
        };
        assert!(matches!(
            client.poll_event().await,
            Err(LiveError::Session(_))
        ));
    }
}
