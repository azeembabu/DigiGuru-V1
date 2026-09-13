//! NN-3 conformance tests for the daily active-voice ledger.
//!
//! `testing.md`: "Cap fires at 1200 s of active voice with a skewed client
//! clock." Every test drives a `TestClock` by hand — no sleeps, no Redis.

use chrono::{TimeZone, Utc};
use dg_core::StudentId;
use quota::{
    day_key_for, local_date, next_local_midnight, resolve_timezone, MemoryStore, QuotaLedger,
    QuotaState, TestClock, CLOSE_CODE_QUOTA, DAILY_QUOTA_MS, DEFAULT_TIMEZONE, END_REASON_QUOTA,
    WARNING_REMAINING_MS,
};

/// A zone whose midnight is nowhere near UTC midnight: UTC-7 in September.
const NON_UTC_ZONE: &str = "America/Los_Angeles";

type Ledger = QuotaLedger<TestClock, MemoryStore>;

fn at(y: i32, m: u32, d: u32, hh: u32, mm: u32) -> chrono::DateTime<Utc> {
    match Utc.with_ymd_and_hms(y, m, d, hh, mm, 0) {
        chrono::LocalResult::Single(dt) => dt,
        _ => panic!("test fixture instant {y}-{m}-{d} {hh}:{mm} is not a valid UTC time"),
    }
}

fn ledger(zone: &str, start_utc: chrono::DateTime<Utc>) -> (TestClock, StudentId, Ledger) {
    let clock = TestClock::starting_at(start_utc);
    let student = StudentId::new();
    let ledger = QuotaLedger::new(
        student,
        resolve_timezone(zone),
        clock.clone(),
        MemoryStore::new(),
    );
    (clock, student, ledger)
}

// ---------------------------------------------------------------------------
// Counting only active voice.
// ---------------------------------------------------------------------------

/// Silence is free (D-19): the ledger only advances between `voice_started` and
/// `voice_stopped`.
#[tokio::test]
async fn time_spent_not_speaking_is_not_counted() {
    let (clock, _student, mut ledger) = ledger("Asia/Kolkata", at(2026, 9, 13, 6, 0));

    // Ten minutes of silence before anyone speaks.
    clock.advance_ms(600_000);
    let status = ledger.tick().await.expect("tick");
    assert_eq!(status.used_ms, 0);
    assert_eq!(status.remaining_ms, DAILY_QUOTA_MS);

    ledger.voice_started().await.expect("voice on");
    clock.advance_ms(30_000);
    ledger.voice_stopped().await.expect("voice off");

    // Another ten minutes of silence.
    clock.advance_ms(600_000);
    let status = ledger.tick().await.expect("tick");
    assert_eq!(status.used_ms, 30_000, "only the spoken 30 s was charged");
    assert!(!ledger.is_voice_active());
}

/// Ticking while voice is active charges each interval exactly once.
#[tokio::test]
async fn active_voice_is_charged_once_per_interval() {
    let (clock, _student, mut ledger) = ledger("Asia/Kolkata", at(2026, 9, 13, 6, 0));
    ledger.voice_started().await.expect("voice on");

    for expected in 1..=5i64 {
        clock.advance_ms(1_000);
        let status = ledger.tick().await.expect("tick");
        assert_eq!(status.used_ms, expected * 1_000);
    }
    // A tick with no elapsed time charges nothing.
    let status = ledger.tick().await.expect("tick");
    assert_eq!(status.used_ms, 5_000);
}

// ---------------------------------------------------------------------------
// The cap.
// ---------------------------------------------------------------------------

/// The core NN-3 property: exhaustion at exactly 1 200 s of active voice, and
/// the documented end reason and close code.
#[tokio::test]
async fn quota_fires_at_exactly_1200_seconds_of_active_voice() {
    let (clock, _student, mut ledger) = ledger("Asia/Kolkata", at(2026, 9, 13, 6, 0));
    ledger.voice_started().await.expect("voice on");

    clock.advance_ms(1_199_999);
    let status = ledger.tick().await.expect("tick");
    assert_eq!(status.remaining_ms, 1);
    assert_eq!(status.state, QuotaState::Warning, "not yet exhausted");

    clock.advance_ms(1);
    let status = ledger.tick().await.expect("tick");
    assert_eq!(status.used_ms, DAILY_QUOTA_MS);
    assert_eq!(status.remaining_ms, 0);
    assert_eq!(status.state, QuotaState::Exhausted);
    assert!(status.is_exhausted());
    assert_eq!(END_REASON_QUOTA, "quota");
    assert_eq!(CLOSE_CODE_QUOTA, 4003);

    // The meter stops itself; overrun does not drive `remaining_ms` negative.
    assert!(!ledger.is_voice_active());
    clock.advance_ms(600_000);
    let status = ledger.tick().await.expect("tick");
    assert_eq!(status.remaining_ms, 0);
    assert_eq!(status.used_ms, DAILY_QUOTA_MS);
}

/// A client that lies about elapsed time — or whose wall clock is two hours
/// fast — changes nothing: the cap still fires at 1 200 s of server-measured
/// active voice.
#[tokio::test]
async fn a_skewed_client_clock_is_ignored_and_the_cap_still_fires_at_1200_seconds() {
    let (clock, _student, mut ledger) = ledger("Asia/Kolkata", at(2026, 9, 13, 6, 0));
    ledger.voice_started().await.expect("voice on");

    let mut server_ms = 0i64;
    let mut spoken = 0i64;
    while spoken < DAILY_QUOTA_MS {
        // The client insists it has already spoken for two hours, and then for
        // negative time. Both claims are recorded and discarded.
        ledger.note_client_claim(7_200_000);
        ledger.note_client_claim(-500_000);

        clock.advance_ms(1_000);
        server_ms += 1_000;
        let status = ledger.tick().await.expect("tick");
        spoken = status.used_ms;
        assert_eq!(spoken, server_ms, "the ledger follows the server clock only");
        if status.is_exhausted() {
            break;
        }
    }
    assert_eq!(server_ms, DAILY_QUOTA_MS, "fired at exactly 1200 s, not sooner or later");
}

/// A wall-clock step (NTP correction, operator change) must not be mistaken for
/// elapsed speech: durations come from the monotonic reading.
#[tokio::test]
async fn a_wall_clock_step_does_not_charge_the_student() {
    let (clock, _student, mut ledger) = ledger("Asia/Kolkata", at(2026, 9, 13, 6, 0));
    ledger.voice_started().await.expect("voice on");

    // Wall clock jumps two hours forward; no monotonic time has passed.
    clock.set_utc(at(2026, 9, 13, 8, 0));
    let status = ledger.tick().await.expect("tick");
    assert_eq!(status.used_ms, 0);

    // Real speech after the step is still charged normally.
    clock.advance_ms(5_000);
    let status = ledger.tick().await.expect("tick");
    assert_eq!(status.used_ms, 5_000);
}

/// `quota_warning` is emitted once, on the tick that enters the band.
#[tokio::test]
async fn the_quota_warning_is_raised_once_per_day() {
    let (clock, _student, mut ledger) = ledger("Asia/Kolkata", at(2026, 9, 13, 6, 0));
    ledger.voice_started().await.expect("voice on");

    clock.advance_ms((DAILY_QUOTA_MS - WARNING_REMAINING_MS - 1_000) as u64);
    let status = ledger.tick().await.expect("tick");
    assert_eq!(status.state, QuotaState::Ok);
    assert!(!status.warning_due);

    clock.advance_ms(1_000);
    let status = ledger.tick().await.expect("tick");
    assert_eq!(status.state, QuotaState::Warning);
    assert_eq!(status.remaining_ms, WARNING_REMAINING_MS);
    assert!(status.warning_due, "the band is entered here");

    clock.advance_ms(1_000);
    let status = ledger.tick().await.expect("tick");
    assert_eq!(status.state, QuotaState::Warning);
    assert!(!status.warning_due, "not repeated every second");
}

// ---------------------------------------------------------------------------
// The day boundary is the student's midnight.
// ---------------------------------------------------------------------------

/// UTC midnight must not reset a Los Angeles student — for them it is 17:00 the
/// previous afternoon, and a reset there would hand out a second allowance.
#[tokio::test]
async fn the_ledger_does_not_reset_at_utc_midnight() {
    // 23:30 UTC on the 13th is 16:30 local on the 13th.
    let (clock, _student, mut ledger) = ledger(NON_UTC_ZONE, at(2026, 9, 13, 23, 30));
    assert_eq!(ledger.current_day_key(), "20260913");

    ledger.voice_started().await.expect("voice on");
    clock.advance_ms(600_000);
    let before = ledger.tick().await.expect("tick");
    assert_eq!(before.used_ms, 600_000);
    ledger.voice_stopped().await.expect("voice off");

    // Cross UTC midnight: 00:30 UTC on the 14th is still 17:30 local on the 13th.
    clock.advance_ms(60 * 60 * 1_000);
    let after = ledger.tick().await.expect("tick");
    assert_eq!(after.day_key, "20260913", "still the same local day");
    assert_eq!(
        after.used_ms, 600_000,
        "the spent ten minutes survived UTC midnight"
    );
}

/// The reset happens at the student's own midnight: 07:00 UTC in September for
/// Los Angeles.
#[tokio::test]
async fn the_ledger_resets_at_midnight_in_the_student_timezone() {
    // 23:55 local on the 13th = 06:55 UTC on the 14th.
    let (clock, _student, mut ledger) = ledger(NON_UTC_ZONE, at(2026, 9, 14, 6, 55));
    assert_eq!(ledger.current_day_key(), "20260913");
    assert_eq!(ledger.next_reset_at(), at(2026, 9, 14, 7, 0));

    ledger.voice_started().await.expect("voice on");
    clock.advance_ms(300_000); // five minutes, ending 00:00 local on the 14th
    ledger.voice_stopped().await.expect("voice off");

    clock.advance_ms(600_000); // 00:10 local on the 14th
    let status = ledger.tick().await.expect("tick");
    assert_eq!(status.day_key, "20260914", "a new local day");
    assert_eq!(status.used_ms, 0, "the allowance reset at local midnight");
    assert_eq!(status.remaining_ms, DAILY_QUOTA_MS);
}

/// A session that is speaking across the student's midnight is charged to both
/// days in proportion — the first day is not billed for tomorrow's minutes.
#[tokio::test]
async fn a_session_spanning_local_midnight_is_charged_to_both_days() {
    // Start 23:55 local on the 13th (06:55 UTC on the 14th).
    let (clock, student, mut ledger) = ledger(NON_UTC_ZONE, at(2026, 9, 14, 6, 55));
    ledger.voice_started().await.expect("voice on");

    // Speak for ten unbroken minutes: five before local midnight, five after.
    clock.advance_ms(600_000);
    let status = ledger.tick().await.expect("tick");

    assert_eq!(status.day_key, "20260914");
    assert_eq!(status.used_ms, 300_000, "only the post-midnight half");
    assert_eq!(status.remaining_ms, DAILY_QUOTA_MS - 300_000);

    let tz = resolve_timezone(NON_UTC_ZONE);
    let yesterday = quota::ledger_key(student, local_date(at(2026, 9, 14, 6, 55), tz));
    let today = quota::ledger_key(student, local_date(at(2026, 9, 14, 7, 5), tz));
    let snapshot = ledger.store().snapshot().expect("snapshot");
    assert_ne!(yesterday, today);
    assert_eq!(snapshot.get(&yesterday).copied(), Some(300_000));
    assert_eq!(snapshot.get(&today).copied(), Some(300_000));
}

/// The ledger key is exactly the documented Redis key shape.
#[tokio::test]
async fn the_ledger_key_matches_the_documented_redis_key() {
    let student = StudentId::new();
    let tz = resolve_timezone("Asia/Kolkata");
    let date = local_date(at(2026, 9, 13, 6, 0), tz);
    assert_eq!(day_key_for(date), "20260913");
    assert_eq!(
        quota::ledger_key(student, date),
        format!("quota:{student}:20260913")
    );
}

/// Two students in different zones share nothing: their allowances reset at
/// different UTC instants.
#[tokio::test]
async fn students_in_different_zones_reset_at_different_instants() {
    let now = at(2026, 9, 13, 20, 0);
    let kolkata = next_local_midnight(now, resolve_timezone("Asia/Kolkata"));
    let los_angeles = next_local_midnight(now, resolve_timezone(NON_UTC_ZONE));
    // 20:00 UTC is 01:30 on the 14th in Kolkata, so its next midnight is the
    // 15th; it is 13:00 on the 13th in Los Angeles, whose midnight is 07:00 UTC.
    assert_eq!(kolkata, at(2026, 9, 14, 18, 30));
    assert_eq!(los_angeles, at(2026, 9, 14, 7, 0));
    assert_ne!(kolkata, los_angeles);
}

/// A bad `students.timezone` value falls back to the documented default rather
/// than failing the session or silently becoming UTC.
#[tokio::test]
async fn an_unrecognised_timezone_falls_back_to_the_default() {
    assert_eq!(resolve_timezone("Mars/Olympus_Mons"), DEFAULT_TIMEZONE);
    assert_eq!(resolve_timezone(""), DEFAULT_TIMEZONE);
    assert_eq!(resolve_timezone(" Asia/Kolkata "), DEFAULT_TIMEZONE);
    assert_ne!(DEFAULT_TIMEZONE, chrono_tz::UTC);
}

/// Two concurrent sockets for the same student share one ledger day, so the
/// allowance cannot be spent twice by opening a second device.
#[tokio::test]
async fn two_sessions_for_one_student_share_the_same_daily_allowance() {
    let clock = TestClock::starting_at(at(2026, 9, 13, 6, 0));
    let student = StudentId::new();
    let store = std::sync::Arc::new(MemoryStore::new());

    let mut first = QuotaLedger::new(student, DEFAULT_TIMEZONE, clock.clone(), store.clone());
    let mut second = QuotaLedger::new(student, DEFAULT_TIMEZONE, clock.clone(), store.clone());

    first.voice_started().await.expect("voice on");
    clock.advance_ms(600_000);
    let a = first.tick().await.expect("tick");
    assert_eq!(a.used_ms, 600_000);

    // The second socket sees the first socket's spend, not a fresh allowance.
    let b = second.tick().await.expect("tick");
    assert_eq!(b.day_key, a.day_key);
    assert_eq!(b.used_ms, 600_000);
    assert_eq!(b.remaining_ms, DAILY_QUOTA_MS - 600_000);

    // And spending on the second socket counts against the same total.
    second.voice_started().await.expect("voice on");
    clock.advance_ms(600_000);
    let b = second.tick().await.expect("tick");
    assert_eq!(
        b.used_ms, 1_200_000,
        "the second socket's ten minutes landed on the same day key as the first's"
    );
    assert!(b.is_exhausted(), "the shared allowance is spent");
}
