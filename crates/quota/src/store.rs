//! The ledger backing store.
//!
//! The production implementation is Redis (`quota:{student_id}:{yyyymmdd}`,
//! String of milliseconds, TTL until the student's local midnight — see the
//! Redis key map in `IMPLEMENTATION_PLAN.md` §3.3). The trait exists so the
//! NN-3 conformance tests can run against [`MemoryStore`] with no container
//! and no network.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

/// A backing-store failure. Never reaches a client — the gateway maps it to a
/// `PublicError` like any other internal fault.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("quota store unavailable: {0}")]
    Backend(String),
    /// The key held something that is not an integer millisecond count.
    #[error("quota key `{key}` holds a non-numeric value")]
    Corrupt { key: String },
    /// A lock was poisoned by a panic elsewhere. Only reachable in the
    /// in-memory store.
    #[error("quota store lock poisoned")]
    Poisoned,
}

/// An atomic counter keyed by `quota:{student_id}:{yyyymmdd}`.
///
/// `add_ms` must be atomic: two gateway nodes ticking the same student must not
/// lose an increment, or the daily cap becomes advisory.
#[async_trait]
pub trait QuotaStore: Send + Sync {
    /// Add `delta_ms` (may be zero) and return the new total for `key`, setting
    /// the key's expiry to `ttl_seconds` from now.
    async fn add_ms(&self, key: &str, delta_ms: i64, ttl_seconds: i64) -> Result<i64, StoreError>;

    /// Read the current total without changing it. A missing key reads as `0`.
    async fn get_ms(&self, key: &str) -> Result<i64, StoreError>;
}

/// Shared ownership of a store, so several sockets for the same student can
/// charge one ledger. In production the sharing is Redis itself; in tests it is
/// an `Arc<MemoryStore>`.
#[async_trait]
impl<T: QuotaStore + ?Sized> QuotaStore for std::sync::Arc<T> {
    async fn add_ms(&self, key: &str, delta_ms: i64, ttl_seconds: i64) -> Result<i64, StoreError> {
        (**self).add_ms(key, delta_ms, ttl_seconds).await
    }

    async fn get_ms(&self, key: &str) -> Result<i64, StoreError> {
        (**self).get_ms(key).await
    }
}

/// An in-process store for unit tests and single-node development.
///
/// Not a production backend: it is per-process, so two gateway nodes would keep
/// two separate ledgers and a student could spend the quota twice.
#[derive(Debug, Default)]
pub struct MemoryStore {
    values: Mutex<HashMap<String, i64>>,
}

impl MemoryStore {
    /// An empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Every key that has been written, for assertions in tests.
    pub fn snapshot(&self) -> Result<HashMap<String, i64>, StoreError> {
        let guard = self.values.lock().map_err(|_| StoreError::Poisoned)?;
        Ok(guard.clone())
    }
}

#[async_trait]
impl QuotaStore for MemoryStore {
    async fn add_ms(&self, key: &str, delta_ms: i64, _ttl_seconds: i64) -> Result<i64, StoreError> {
        let mut guard = self.values.lock().map_err(|_| StoreError::Poisoned)?;
        let entry = guard.entry(key.to_owned()).or_insert(0);
        *entry = entry.saturating_add(delta_ms);
        Ok(*entry)
    }

    async fn get_ms(&self, key: &str) -> Result<i64, StoreError> {
        let guard = self.values.lock().map_err(|_| StoreError::Poisoned)?;
        Ok(guard.get(key).copied().unwrap_or(0))
    }
}

#[cfg(feature = "redis-store")]
mod redis_store {
    use super::{QuotaStore, StoreError};
    use async_trait::async_trait;
    use redis::aio::ConnectionManager;
    use redis::AsyncCommands;

    /// The production ledger: a Redis string of milliseconds per
    /// `quota:{student_id}:{yyyymmdd}`.
    #[derive(Clone)]
    pub struct RedisQuotaStore {
        conn: ConnectionManager,
    }

    impl RedisQuotaStore {
        /// Wrap an existing connection manager (the gateway owns the pool).
        pub fn new(conn: ConnectionManager) -> Self {
            Self { conn }
        }
    }

    impl std::fmt::Debug for RedisQuotaStore {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("RedisQuotaStore")
        }
    }

    fn backend(e: redis::RedisError) -> StoreError {
        StoreError::Backend(e.to_string())
    }

    #[async_trait]
    impl QuotaStore for RedisQuotaStore {
        async fn add_ms(
            &self,
            key: &str,
            delta_ms: i64,
            ttl_seconds: i64,
        ) -> Result<i64, StoreError> {
            let mut conn = self.conn.clone();
            // INCRBY is atomic across nodes, which is the whole reason the
            // ledger lives in Redis rather than in the socket task.
            let total: i64 = conn.incr(key, delta_ms).await.map_err(backend)?;
            // Refreshed on every tick: the TTL is recomputed against the
            // student's *next* local midnight, so a session that crosses the
            // boundary leaves the new day's key expiring correctly too.
            let ttl = ttl_seconds.max(1);
            let _: () = conn.expire(key, ttl).await.map_err(backend)?;
            Ok(total)
        }

        async fn get_ms(&self, key: &str) -> Result<i64, StoreError> {
            let mut conn = self.conn.clone();
            let raw: Option<String> = conn.get(key).await.map_err(backend)?;
            match raw {
                None => Ok(0),
                Some(s) => s.parse::<i64>().map_err(|_| StoreError::Corrupt {
                    key: key.to_owned(),
                }),
            }
        }
    }
}

#[cfg(feature = "redis-store")]
pub use redis_store::RedisQuotaStore;
