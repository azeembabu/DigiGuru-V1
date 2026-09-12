//! Struct definitions only for the later-phase tables — `documents`
//! (Phase 2), `learning_sessions` / `board_events` / `note_reminders`
//! (Phase 3/4), and `safety_incidents` (Phase 4). No query modules yet; the
//! phase that owns each table adds its own queries alongside its logic.
//! These exist now purely so shared typing (e.g. a `LearningSession` return
//! type) is available before that lands.

use chrono::{DateTime, Utc};
use dg_core::{BlockId, CourseId, DocumentId, SessionId, StudentId, UserId};
use serde_json::Value;
use uuid::Uuid;

/// `documents` — an ingested, block-scoped PDF (Phase 2).
#[derive(Debug, Clone)]
pub struct Document {
    pub id: DocumentId,
    pub block_id: BlockId,
    pub uploaded_by: UserId,
    pub title: String,
    pub storage_key: String,
    pub sha256: String,
    pub page_count: i32,
    pub ocr_confidence: Option<f32>,
    /// `pending|parsing|pending_review|embedded|failed` — kept as a plain
    /// `String` rather than a Rust enum until Phase 2 defines the ingestion
    /// state machine and its exact transitions.
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// `learning_sessions` — the classroom session (Phase 3/4). Named distinctly
/// from `auth_sessions` per `CLAUDE.md`.
#[derive(Debug, Clone)]
pub struct LearningSession {
    pub id: SessionId,
    pub student_id: StudentId,
    pub course_id: CourseId,
    pub block_id: BlockId,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    /// NN-3: server-authoritative cumulative active-voice time.
    pub active_voice_ms: i64,
    pub status: dg_core::SessionStatus,
    /// `quota|idle|user|jailbreak|error`
    pub end_reason: Option<String>,
    pub resume_summary: Option<String>,
    pub last_topic: Option<String>,
    pub last_page: Option<i32>,
}

/// `safety_incidents` — a guardrail hit (Phase 4, NN-5).
#[derive(Debug, Clone)]
pub struct SafetyIncident {
    pub id: Uuid,
    pub session_id: Option<SessionId>,
    pub student_id: StudentId,
    /// `jailbreak|toxicity|out_of_scope`
    pub kind: String,
    /// 0 = pre-LLM tap, 1 = transcript, 2 = output.
    pub tier: i16,
    /// PII-redacted excerpt of the offending transcript.
    pub excerpt: String,
    pub created_at: DateTime<Utc>,
}

/// `board_events` — whiteboard audit/replay log (Phase 3, NN-1).
#[derive(Debug, Clone)]
pub struct BoardEvent {
    pub id: i64,
    pub session_id: SessionId,
    pub turn_seq: i32,
    pub op: Value,
    pub emitted_at: DateTime<Utc>,
    /// Client ACK latency in ms; `None` means never acked (a `wb_violation`).
    pub acked_ms: Option<i32>,
}

/// `note_reminders` — the reminder ledger for exported notes (Phase 4, see
/// `.claude/rules/pedagogy.md`). Points at a `board_events` range and a
/// date; deliberately not a content store of its own.
#[derive(Debug, Clone)]
pub struct NoteReminder {
    pub id: Uuid,
    pub student_id: StudentId,
    pub session_id: SessionId,
    pub event_from_id: i64,
    pub event_to_id: i64,
    pub remind_at: chrono::NaiveDate,
    pub surfaced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
