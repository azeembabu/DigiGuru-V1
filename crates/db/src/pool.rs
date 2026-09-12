//! Postgres connection-pool construction.

use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::error::{Error, Result};

/// Build a `PgPool` from a `DATABASE_URL`. Callers (the gateway's `main.rs`)
/// should hold exactly one pool per process and share it via application
/// state — `sqlx`'s pool is already an internally-pooled `Arc`-like handle.
///
/// Sized conservatively for a single gateway node behind PgBouncer in
/// transaction mode (`.claude/rules/security.md`, H-46): keep this at or
/// below the PgBouncer pool size, not the raw Postgres `max_connections`.
pub async fn create_pool(database_url: &str) -> Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
        .map_err(Error::from_sqlx)
}
