//! The injected clock for the NN-3 ledger.
//!
//! Two readings, deliberately separate:
//!
//! * `now_utc` answers **which day** the time is charged to. It is a wall
//!   clock because a calendar date is a wall-clock concept.
//! * `now_mono_ms` answers **how much** time elapsed. It is monotonic so an
//!   NTP step, a DST shift, or an operator changing the server clock cannot
//!   gift or steal a student's minutes.
//!
//! Neither reading ever comes from the client. `security.md`/NN-3: client
//! clocks are never trusted for the elapsed time or for the date.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, TimeZone, Utc};

/// A source of server-authoritative time.
pub trait Clock: Send + Sync + 'static {
    /// Wall-clock now, in UTC. Used only to derive the student's local date.
    fn now_utc(&self) -> DateTime<Utc>;
    /// Monotonic milliseconds since an arbitrary fixed epoch. Used for every
    /// duration the ledger charges.
    fn now_mono_ms(&self) -> u64;
}

/// The production clock.
#[derive(Debug, Clone)]
pub struct SystemClock {
    epoch: Instant,
}

impl SystemClock {
    /// Start a clock whose monotonic zero point is now.
    pub fn new() -> Self {
        Self {
            epoch: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now_utc(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn now_mono_ms(&self) -> u64 {
        self.epoch.elapsed().as_millis() as u64
    }
}

/// A hand-driven clock for tests. Cloning shares the same underlying time, so a
/// test can hold one handle while the ledger holds another.
///
/// [`TestClock::advance_ms`] moves the wall clock and the monotonic clock
/// together, which is the honest default; [`TestClock::advance_mono_only`] and
/// [`TestClock::set_utc`] exist so a test can pull them apart deliberately.
#[derive(Debug, Clone)]
pub struct TestClock {
    utc_ms: Arc<AtomicI64>,
    mono_ms: Arc<AtomicU64>,
}

impl TestClock {
    /// Start at a given wall-clock instant, with the monotonic clock at zero.
    pub fn starting_at(utc: DateTime<Utc>) -> Self {
        Self {
            utc_ms: Arc::new(AtomicI64::new(utc.timestamp_millis())),
            mono_ms: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Move both readings forward by the same amount.
    pub fn advance_ms(&self, delta: u64) {
        self.utc_ms.fetch_add(delta as i64, Ordering::SeqCst);
        self.mono_ms.fetch_add(delta, Ordering::SeqCst);
    }

    /// Move only the monotonic reading — models a wall clock that has been
    /// stepped backwards or is otherwise not advancing with real time.
    pub fn advance_mono_only(&self, delta: u64) {
        self.mono_ms.fetch_add(delta, Ordering::SeqCst);
    }

    /// Jump the wall clock without accruing monotonic time.
    pub fn set_utc(&self, utc: DateTime<Utc>) {
        self.utc_ms.store(utc.timestamp_millis(), Ordering::SeqCst);
    }
}

impl Clock for TestClock {
    fn now_utc(&self) -> DateTime<Utc> {
        let ms = self.utc_ms.load(Ordering::SeqCst);
        // `timestamp_millis_opt` cannot be out of range for any value a test
        // can reach here, but NN-3 code does not unwrap: fall back to the epoch.
        match Utc.timestamp_millis_opt(ms) {
            chrono::LocalResult::Single(dt) => dt,
            _ => DateTime::<Utc>::UNIX_EPOCH,
        }
    }

    fn now_mono_ms(&self) -> u64 {
        self.mono_ms.load(Ordering::SeqCst)
    }
}
