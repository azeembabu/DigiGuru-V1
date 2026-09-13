//! NN-3 enforcement: the daily active-voice ledger.
//!
//! Twenty minutes of active voice per student per calendar day,
//! server-authoritative and Redis-backed, counted only while voice is actually
//! active. The day boundary is midnight in the student's own timezone.
//!
//! The ledger is driven entirely by its owner (the socket task): mark voice
//! active or idle, and call [`QuotaLedger::tick`] on the gateway's one-second
//! ticker. It reads no client input at all — [`QuotaLedger::note_client_claim`]
//! exists purely so a client's assertion about elapsed time has a place to be
//! recorded and *ignored*.

use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use dg_core::StudentId;

use crate::clock::Clock;
use crate::day::{
    ledger_key, local_date, next_local_midnight, seconds_until_next_local_midnight,
};
use crate::store::{QuotaStore, StoreError};

/// The daily allowance: 20 minutes of active voice.
pub const DAILY_QUOTA_MS: i64 = 20 * 60 * 1_000;

/// Remaining allowance at which `quota_warning` is emitted — two minutes,
/// matching the worked example in `api-conventions.md` and the "begin wrapping
/// up at 18:00 elapsed" rule in `IMPLEMENTATION_PLAN.md` §7.2.
pub const WARNING_REMAINING_MS: i64 = 2 * 60 * 1_000;

/// `learning_sessions.end_reason` written when the allowance runs out.
pub const END_REASON_QUOTA: &str = "quota";

/// WebSocket close code for quota exhaustion (`api-conventions.md`).
pub const CLOSE_CODE_QUOTA: u16 = 4003;

/// Guard on the day-splitting loop. A tick can never legitimately span more
/// than a couple of local days; the bound just makes the loop provably finite.
const MAX_DAY_SEGMENTS: usize = 64;

/// Anything that can go wrong charging the ledger.
#[derive(Debug, thiserror::Error)]
pub enum QuotaError {
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Where the student stands against today's allowance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaState {
    /// More than [`WARNING_REMAINING_MS`] left.
    Ok,
    /// Inside the warning band; the session should start wrapping up.
    Warning,
    /// Allowance spent. The gateway ends the session with
    /// [`END_REASON_QUOTA`] and closes with [`CLOSE_CODE_QUOTA`].
    Exhausted,
}

/// A ledger reading, safe to derive a `quota_warning` or `session_end` from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaStatus {
    /// The student-local ledger day, `yyyymmdd`.
    pub day_key: String,
    /// Active-voice milliseconds charged to that day so far.
    pub used_ms: i64,
    /// `quota_remaining_ms` for `session_ready`; never negative.
    pub remaining_ms: i64,
    pub state: QuotaState,
    /// True exactly once per ledger day, on the tick that first enters the
    /// warning band — so the caller can emit `quota_warning` without keeping
    /// its own dedupe flag.
    pub warning_due: bool,
}

impl QuotaStatus {
    /// True when the session must end now.
    pub fn is_exhausted(&self) -> bool {
        self.state == QuotaState::Exhausted
    }
}

/// The per-session view of a student's daily voice allowance.
///
/// One ledger per socket. Several ledgers for the same student (two devices)
/// stay consistent because every charge is an atomic increment in the shared
/// store.
#[derive(Debug)]
pub struct QuotaLedger<C: Clock, S: QuotaStore> {
    student_id: StudentId,
    timezone: Tz,
    clock: C,
    store: S,
    /// Whether voice is currently active (student speaking or AI speaking).
    /// Silence is free (`IMPLEMENTATION_PLAN.md` D-19).
    voice_active: bool,
    /// Monotonic instant of the last accrual — the authority for *how much*.
    mark_mono_ms: u64,
    /// Wall instant of the last accrual — the authority for *which day*.
    mark_utc: DateTime<Utc>,
    /// The ledger day a `quota_warning` has already been raised for.
    warned_day: Option<String>,
}

impl<C: Clock, S: QuotaStore> QuotaLedger<C, S> {
    /// Open a ledger for a student. `timezone` comes from `students.timezone`;
    /// use [`crate::day::resolve_timezone`] to parse it.
    pub fn new(student_id: StudentId, timezone: Tz, clock: C, store: S) -> Self {
        let mark_mono_ms = clock.now_mono_ms();
        let mark_utc = clock.now_utc();
        Self {
            student_id,
            timezone,
            clock,
            store,
            voice_active: false,
            mark_mono_ms,
            mark_utc,
            warned_day: None,
        }
    }

    /// The student this ledger charges.
    pub fn student_id(&self) -> StudentId {
        self.student_id
    }

    /// The backing store, for callers that want to inspect it (tests) or share
    /// it with another ledger.
    pub fn store(&self) -> &S {
        &self.store
    }

    /// The timezone whose midnight resets the allowance.
    pub fn timezone(&self) -> Tz {
        self.timezone
    }

    /// True while voice time is being charged.
    pub fn is_voice_active(&self) -> bool {
        self.voice_active
    }

    /// The student-local ledger day right now, `yyyymmdd`.
    pub fn current_day_key(&self) -> String {
        crate::day::day_key_for(self.current_date())
    }

    /// The UTC instant at which today's allowance resets for this student.
    pub fn next_reset_at(&self) -> DateTime<Utc> {
        next_local_midnight(self.clock.now_utc(), self.timezone)
    }

    /// Voice became active (student started speaking, or the tutor's audio
    /// started flowing). Time from here is charged.
    pub async fn voice_started(&mut self) -> Result<QuotaStatus, QuotaError> {
        let charged = self.accrue().await?;
        self.voice_active = true;
        self.status_after(charged).await
    }

    /// Voice went idle. Charges everything up to this instant and stops the
    /// meter — silence costs nothing.
    pub async fn voice_stopped(&mut self) -> Result<QuotaStatus, QuotaError> {
        let charged = self.accrue().await?;
        self.voice_active = false;
        self.status_after(charged).await
    }

    /// The gateway's one-second tick. Charges the elapsed interval if voice is
    /// active and returns the current standing.
    pub async fn tick(&mut self) -> Result<QuotaStatus, QuotaError> {
        let charged = self.accrue().await?;
        self.status_after(charged).await
    }

    /// Read the ledger without charging anything new beyond what has already
    /// accrued. Use this for `session_ready`'s `quota_remaining_ms`.
    pub async fn status(&mut self) -> Result<QuotaStatus, QuotaError> {
        self.tick().await
    }

    /// Record a client's own claim about how long it has been speaking.
    ///
    /// The claim is **never** used. It exists so a skewed or hostile client
    /// clock has a single, auditable place to arrive at and be discarded — NN-3
    /// forbids trusting a client for either elapsed time or the date.
    pub fn note_client_claim(&self, claimed_elapsed_ms: i64) {
        tracing::debug!(
            student_id = %self.student_id,
            claimed_elapsed_ms,
            "ignoring client-reported elapsed voice time; the ledger is server-authoritative"
        );
    }

    fn current_date(&self) -> NaiveDate {
        local_date(self.clock.now_utc(), self.timezone)
    }

    /// Charge the interval since the last mark, splitting it across the
    /// student's local midnight if it crosses one.
    ///
    /// Returns the new total for *today's* key if this call happened to write
    /// it, so the common path needs only one store round-trip.
    async fn accrue(&mut self) -> Result<Option<i64>, QuotaError> {
        let now_mono = self.clock.now_mono_ms();
        let now_utc = self.clock.now_utc();
        let prev_utc = self.mark_utc;
        // Monotonic, so a wall-clock step cannot inflate or erase the charge.
        let elapsed_ms = i64::try_from(now_mono.saturating_sub(self.mark_mono_ms)).unwrap_or(i64::MAX);
        let was_active = self.voice_active;

        self.mark_mono_ms = now_mono;
        self.mark_utc = now_utc;

        if !was_active || elapsed_ms <= 0 {
            return Ok(None);
        }

        let today = local_date(now_utc, self.timezone);
        let wall_span_ms = (now_utc - prev_utc).num_milliseconds();
        if wall_span_ms <= 0 {
            // The wall clock did not advance (or went backwards). Charge the
            // whole monotonic interval to the current local day.
            let total = self.charge(today, elapsed_ms, now_utc).await?;
            return Ok(Some(total));
        }

        let mut cursor = prev_utc;
        let mut unspent = elapsed_ms;
        let mut today_total = None;
        for _ in 0..MAX_DAY_SEGMENTS {
            let boundary = next_local_midnight(cursor, self.timezone);
            if boundary >= now_utc || unspent <= 0 {
                break;
            }
            // Apportion by wall-clock share, but never spend more monotonic
            // milliseconds than actually elapsed.
            let seg_wall = (boundary - cursor).num_milliseconds().max(0);
            let portion = ((elapsed_ms as i128 * seg_wall as i128) / wall_span_ms as i128) as i64;
            let portion = portion.clamp(0, unspent);
            let date = local_date(cursor, self.timezone);
            let total = self.charge(date, portion, cursor).await?;
            if date == today {
                today_total = Some(total);
            }
            unspent -= portion;
            cursor = boundary;
        }

        if unspent > 0 {
            let date = local_date(cursor, self.timezone);
            let total = self.charge(date, unspent, now_utc).await?;
            if date == today {
                today_total = Some(total);
            }
        }

        Ok(today_total)
    }

    /// Increment one student-day key, with a TTL that expires it at that day's
    /// own local midnight.
    async fn charge(
        &self,
        date: NaiveDate,
        delta_ms: i64,
        at: DateTime<Utc>,
    ) -> Result<i64, QuotaError> {
        let key = ledger_key(self.student_id, date);
        let ttl = seconds_until_next_local_midnight(at, self.timezone);
        let total = self.store.add_ms(&key, delta_ms, ttl).await?;
        Ok(total)
    }

    /// Build the status for the current local day, reusing the total from the
    /// accrual when there is one.
    async fn status_after(&mut self, charged_total: Option<i64>) -> Result<QuotaStatus, QuotaError> {
        let now_utc = self.clock.now_utc();
        let date = local_date(now_utc, self.timezone);
        let used_ms = match charged_total {
            Some(total) => total,
            // A zero increment both reads the total and refreshes the TTL, so
            // an idle session still keeps today's key alive to its own midnight.
            None => self.charge(date, 0, now_utc).await?,
        };

        let remaining_ms = (DAILY_QUOTA_MS - used_ms).max(0);
        let state = if remaining_ms == 0 {
            QuotaState::Exhausted
        } else if remaining_ms <= WARNING_REMAINING_MS {
            QuotaState::Warning
        } else {
            QuotaState::Ok
        };

        let day_key = crate::day::day_key_for(date);
        let warning_due = state == QuotaState::Warning
            && self.warned_day.as_deref() != Some(day_key.as_str());
        if warning_due {
            self.warned_day = Some(day_key.clone());
            tracing::info!(
                student_id = %self.student_id,
                day = %day_key,
                remaining_ms,
                "daily voice quota entering the warning band"
            );
        }
        if state == QuotaState::Exhausted {
            // The meter stops here; the caller closes the socket.
            self.voice_active = false;
            tracing::info!(
                student_id = %self.student_id,
                day = %day_key,
                used_ms,
                end_reason = END_REASON_QUOTA,
                close_code = CLOSE_CODE_QUOTA,
                "daily voice quota exhausted"
            );
        }

        Ok(QuotaStatus {
            day_key,
            used_ms,
            remaining_ms,
            state,
            warning_due,
        })
    }
}
