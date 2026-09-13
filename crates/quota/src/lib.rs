//! `quota` — the NN-3 daily active-voice ledger.
//!
//! **Twenty minutes of active voice per student per calendar day**,
//! server-authoritative, Redis-backed, counted only while voice is actually
//! active. The day boundary is midnight in the **student's own timezone**
//! (`students.timezone`, default `Asia/Kolkata`) — not UTC and not the
//! server's. Client clocks are never trusted for the elapsed time or the date.
//!
//! When the allowance reaches zero the session ends with
//! [`END_REASON_QUOTA`] and WebSocket close code [`CLOSE_CODE_QUOTA`];
//! [`QuotaStatus::warning_due`] fires once per day so the gateway can emit
//! `quota_warning` before exhaustion.
//!
//! The ledger is pure logic over an injected [`Clock`] and a [`QuotaStore`]
//! trait, so the conformance tests run instantly against [`MemoryStore`] with
//! no Redis and no sleeps.
//!
//! ```
//! use chrono::{TimeZone, Utc};
//! use quota::{MemoryStore, QuotaLedger, QuotaState, TestClock, resolve_timezone};
//! use dg_core::StudentId;
//!
//! # tokio_test_stub(async {
//! let clock = TestClock::starting_at(Utc.with_ymd_and_hms(2026, 9, 13, 6, 0, 0).unwrap());
//! let tz = resolve_timezone("Asia/Kolkata");
//! let mut ledger = QuotaLedger::new(StudentId::new(), tz, clock.clone(), MemoryStore::new());
//!
//! ledger.voice_started().await?;
//! clock.advance_ms(60_000);
//! let status = ledger.tick().await?;
//! assert_eq!(status.used_ms, 60_000);
//! assert_eq!(status.state, QuotaState::Ok);
//! # Ok::<(), quota::QuotaError>(())
//! # });
//! # fn tokio_test_stub<F: std::future::Future>(_f: F) {}
//! ```

pub mod clock;
pub mod day;
pub mod ledger;
pub mod store;

pub use clock::{Clock, SystemClock, TestClock};
pub use day::{
    day_key_for, ledger_key, local_date, next_local_midnight, resolve_timezone,
    seconds_until_next_local_midnight, DEFAULT_TIMEZONE,
};
pub use ledger::{
    QuotaError, QuotaLedger, QuotaState, QuotaStatus, CLOSE_CODE_QUOTA, DAILY_QUOTA_MS,
    END_REASON_QUOTA, WARNING_REMAINING_MS,
};
pub use store::{MemoryStore, QuotaStore, StoreError};

#[cfg(feature = "redis-store")]
pub use store::RedisQuotaStore;
