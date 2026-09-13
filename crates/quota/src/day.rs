//! Day boundaries in the **student's** timezone.
//!
//! NN-3: "The day boundary is midnight in the student's own timezone
//! (`students.timezone`), not UTC and not the server's — a student in a
//! different zone must not lose minutes to a reset that happens mid-afternoon
//! for them."

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;

/// The default from the `students.timezone` column
/// (`IMPLEMENTATION_PLAN.md` §3.1).
pub const DEFAULT_TIMEZONE: Tz = chrono_tz::Asia::Kolkata;

/// Resolve an IANA timezone name from `students.timezone`.
///
/// An unrecognised name falls back to [`DEFAULT_TIMEZONE`] with a warning
/// rather than failing the session: a bad row in the students table must not
/// stop a student from learning, and the fallback is stricter than UTC for the
/// population this platform serves.
pub fn resolve_timezone(name: &str) -> Tz {
    match name.trim().parse::<Tz>() {
        Ok(tz) => tz,
        Err(_) => {
            tracing::warn!(
                timezone = name,
                fallback = %DEFAULT_TIMEZONE,
                "unrecognised students.timezone; falling back to the default"
            );
            DEFAULT_TIMEZONE
        }
    }
}

/// The student's local calendar date at `instant`.
pub fn local_date(instant: DateTime<Utc>, tz: Tz) -> NaiveDate {
    instant.with_timezone(&tz).date_naive()
}

/// The `yyyymmdd` component of the Redis key `quota:{student_id}:{yyyymmdd}`.
pub fn day_key_for(date: NaiveDate) -> String {
    date.format("%Y%m%d").to_string()
}

/// The full ledger key for a student-day.
pub fn ledger_key(student_id: dg_core::StudentId, date: NaiveDate) -> String {
    format!("quota:{student_id}:{}", day_key_for(date))
}

/// The UTC instant of the first midnight strictly after `instant`, in `tz`.
///
/// DST is handled explicitly: in a zone that skips midnight on a transition day
/// (Brazil historically did), local `00:00` does not exist, so the boundary is
/// the first instant that *does* — the moment the local date changes.
pub fn next_local_midnight(instant: DateTime<Utc>, tz: Tz) -> DateTime<Utc> {
    let today = local_date(instant, tz);
    let mut date = today.succ_opt().unwrap_or(today);
    for _ in 0..4 {
        if let Some(boundary) = midnight_utc(date, tz) {
            if boundary > instant {
                return boundary;
            }
        }
        // Midnight did not exist (or fell before `instant`, which a DST jump
        // can produce): step to the next candidate day.
        date = match date.succ_opt() {
            Some(next) => next,
            None => break,
        };
    }
    // Unreachable for any real zone; a 24 h fallback keeps the ledger moving
    // rather than panicking on the audio path.
    instant + Duration::days(1)
}

/// The UTC instant of local midnight starting `date` in `tz`, if it exists.
fn midnight_utc(date: NaiveDate, tz: Tz) -> Option<DateTime<Utc>> {
    let naive = date.and_hms_opt(0, 0, 0)?;
    match tz.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => Some(dt.with_timezone(&Utc)),
        // Ambiguous (clocks went back): the earlier instant is the one at which
        // the local date changed, so it is the boundary.
        chrono::LocalResult::Ambiguous(earlier, _) => Some(earlier.with_timezone(&Utc)),
        // Skipped (clocks went forward over midnight): the local date changes
        // at the transition itself. Probe forward in minutes to find it.
        chrono::LocalResult::None => (1..=240).find_map(|minutes| {
            let probe = naive.checked_add_signed(Duration::minutes(minutes))?;
            match tz.from_local_datetime(&probe) {
                chrono::LocalResult::Single(dt) => Some(dt.with_timezone(&Utc)),
                chrono::LocalResult::Ambiguous(earlier, _) => Some(earlier.with_timezone(&Utc)),
                chrono::LocalResult::None => None,
            }
        }),
    }
}

/// Seconds from `instant` until the student's next local midnight, which is the
/// TTL the ledger key is written with — the key expires exactly when the
/// allowance resets, so a stale ledger can never outlive its day.
pub fn seconds_until_next_local_midnight(instant: DateTime<Utc>, tz: Tz) -> i64 {
    let boundary = next_local_midnight(instant, tz);
    (boundary - instant).num_seconds().max(1)
}
