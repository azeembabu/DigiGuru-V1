//! A monotonic clock abstraction so every hold-timer test runs instantly.
//!
//! `testing.md` forbids sleeps for synchronisation, so the `SyncGate` never
//! reads the wall clock directly — it asks a `Clock` for monotonic
//! milliseconds and the tests drive a `TestClock` by hand.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// A source of monotonic milliseconds.
///
/// Only elapsed time matters to the gate, so the epoch is arbitrary; the one
/// guarantee an implementation must make is that the value never goes
/// backwards. Wall-clock time is deliberately absent: a hold ceiling must not
/// be affected by an NTP step.
pub trait Clock: Send + Sync + 'static {
    /// Monotonic milliseconds since an arbitrary, fixed epoch.
    fn now_ms(&self) -> u64;
}

/// The production clock: `std::time::Instant` measured from process start.
#[derive(Debug, Clone)]
pub struct SystemClock {
    epoch: Instant,
}

impl SystemClock {
    /// Start a clock whose zero point is now.
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
    fn now_ms(&self) -> u64 {
        // `saturating_sub`-free by construction: `Instant` is monotonic, so
        // `elapsed()` cannot be negative. The cast is lossless for any
        // realistic process lifetime.
        self.epoch.elapsed().as_millis() as u64
    }
}

/// A hand-driven clock for tests. Cheap to clone; all clones share one value.
#[derive(Debug, Clone, Default)]
pub struct TestClock {
    now_ms: Arc<AtomicU64>,
}

impl TestClock {
    /// Start at zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Move the clock forward. Never moves it backwards.
    pub fn advance_ms(&self, delta: u64) {
        self.now_ms.fetch_add(delta, Ordering::SeqCst);
    }
}

impl Clock for TestClock {
    fn now_ms(&self) -> u64 {
        self.now_ms.load(Ordering::SeqCst)
    }
}
