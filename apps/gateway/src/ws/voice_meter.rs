//! NN-3 for the classroom socket: the per-session view of the student's daily
//! active-voice allowance.
//!
//! This module owns **no** quota logic. The ledger, the 20-minute constant,
//! the warning band and the midnight-in-`students.timezone` day boundary all
//! live in `crates/quota` and are reused verbatim — recomputing any of them
//! here is how the two copies drift and a student in another timezone loses
//! minutes to the wrong reset. All this type adds is the one thing the ledger
//! cannot know on its own: **when voice is actually active**, which on this
//! socket means "a frame moved in the last [`VOICE_GAP_MS`]".
//!
//! Silence is free (`IMPLEMENTATION_PLAN.md` D-19), so the meter closes the
//! voice window after a short gap rather than charging the whole wall-clock
//! session. Both directions count as voice: the student speaking and the
//! tutor speaking are both time the allowance is paying for.
//!
//! A socket with no `students` row (an admin opening the classroom) gets
//! [`VoiceMeter::disabled`]: there is no student to charge, and an admin
//! preview must not be billed to anyone.

use std::time::Duration;

use quota::{
    Clock as QuotaClock, QuotaLedger, QuotaStatus, RedisQuotaStore, SystemClock as QuotaSystemClock,
};

/// How long after the last audio frame voice is still considered active.
///
/// Tutor audio arrives in bursts and the student's VAD has a 300 ms hangover
/// (`.claude/rules/realtime-audio.md`), so a gap shorter than that is part of
/// the same utterance, not silence. 600 ms keeps a normal turn as one
/// continuous charge without charging a genuinely idle minute.
const VOICE_GAP_MS: u64 = 600;

/// How often the ledger is charged while a session runs. The ledger is
/// monotonic-clock based, so the tick only decides the *resolution* of
/// enforcement, not its accuracy.
pub const TICK: Duration = Duration::from_secs(1);

/// The quota meter for one socket.
pub enum VoiceMeter {
    /// No student to charge — the session runs unmetered and never ends on
    /// quota.
    Disabled,
    Enabled {
        ledger: Box<QuotaLedger<QuotaSystemClock, RedisQuotaStore>>,
        clock: QuotaSystemClock,
        /// Monotonic ms of the last audio frame in either direction.
        last_frame_mono_ms: Option<u64>,
    },
}

impl VoiceMeter {
    pub fn disabled() -> Self {
        Self::Disabled
    }

    /// Opens a ledger for a student. `timezone` is the raw `students.timezone`
    /// string; `quota::resolve_timezone` falls back to the documented default
    /// for an unparseable value rather than failing the session.
    pub fn enabled(
        student_id: dg_core::StudentId,
        timezone: &str,
        redis: redis::aio::ConnectionManager,
    ) -> Self {
        let clock = QuotaSystemClock::new();
        let tz = quota::resolve_timezone(timezone);
        Self::Enabled {
            ledger: Box::new(QuotaLedger::new(
                student_id,
                tz,
                clock.clone(),
                RedisQuotaStore::new(redis),
            )),
            clock,
            last_frame_mono_ms: None,
        }
    }

    /// `quota_remaining_ms` for `session_ready`. Reads the ledger; charges
    /// nothing, because no voice has flowed yet.
    pub async fn remaining_ms(&mut self) -> i64 {
        match self {
            Self::Disabled => quota::DAILY_QUOTA_MS,
            Self::Enabled { ledger, .. } => match ledger.status().await {
                Ok(status) => status.remaining_ms,
                Err(err) => {
                    // Fail open on a *read*: the tick below is the enforcement
                    // point and will still end the session, so a Redis blip
                    // must not refuse a student their lesson.
                    tracing::warn!(%err, "could not read the quota ledger for session_ready");
                    quota::DAILY_QUOTA_MS
                }
            },
        }
    }

    /// An audio frame moved, in either direction. Opens the voice window if
    /// it was closed.
    pub async fn note_audio_frame(&mut self) {
        let Self::Enabled {
            ledger,
            clock,
            last_frame_mono_ms,
        } = self
        else {
            return;
        };

        *last_frame_mono_ms = Some(clock.now_mono_ms());
        if !ledger.is_voice_active() {
            if let Err(err) = ledger.voice_started().await {
                tracing::warn!(%err, "failed to open the quota voice window");
            }
        }
    }

    /// The one-second tick: charges the elapsed interval when voice is
    /// active, closes the window after [`VOICE_GAP_MS`] of silence, and
    /// returns where the student now stands.
    ///
    /// `None` means nothing to enforce (disabled meter, or the ledger could
    /// not be reached). A store error is deliberately *not* treated as
    /// exhaustion: ending a paid-for lesson because Redis hiccuped is the
    /// worse failure, and the next tick one second later retries.
    pub async fn tick(&mut self) -> Option<QuotaStatus> {
        let Self::Enabled {
            ledger,
            clock,
            last_frame_mono_ms,
        } = self
        else {
            return None;
        };

        let idle = match *last_frame_mono_ms {
            Some(mark) => clock.now_mono_ms().saturating_sub(mark) >= VOICE_GAP_MS,
            None => true,
        };

        let result = if ledger.is_voice_active() && idle {
            ledger.voice_stopped().await
        } else {
            ledger.tick().await
        };

        match result {
            Ok(status) => Some(status),
            Err(err) => {
                tracing::warn!(%err, "quota tick failed; retrying on the next tick");
                None
            }
        }
    }

    /// Active-voice ms charged to the student's current day, for the
    /// `learning_sessions.active_voice_ms` column on teardown.
    ///
    /// This is the day's total rather than this session's alone; `close` takes
    /// `GREATEST(active_voice_ms, $3)`, and the day total is the
    /// server-authoritative figure NN-3 actually holds.
    pub async fn used_ms(&mut self) -> i64 {
        match self {
            Self::Disabled => 0,
            Self::Enabled { ledger, .. } => ledger.status().await.map(|s| s.used_ms).unwrap_or(0),
        }
    }

    /// Closes the voice window on teardown so the final interval is charged
    /// before the task exits.
    pub async fn finish(&mut self) {
        if let Self::Enabled { ledger, .. } = self {
            if let Err(err) = ledger.voice_stopped().await {
                tracing::warn!(%err, "failed to charge the final voice interval");
            }
        }
    }
}
