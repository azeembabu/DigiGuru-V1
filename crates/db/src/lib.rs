//! `db` — sqlx models and query modules for Digi Guru's PostgreSQL schema
//! (users, RBAC, academic hierarchy, sessions, audit log).
//!
//! Migrations live in `migrations/` at the repo root and are applied with
//! `sqlx migrate run` (sqlx-cli), not from code — this crate only queries an
//! already-migrated database. See `pool::create_pool` for the connection
//! setup and `error::Error` for this crate's error type.
//!
//! Compile-time query checking (`sqlx::query!`/`query_as!`) requires either
//! a live, migrated Postgres reachable via `DATABASE_URL`, or a committed
//! `.sqlx` offline query cache produced once by `cargo sqlx prepare`.

pub mod error;
pub mod models;
pub mod pool;

pub use error::Error;
pub use pool::create_pool;
pub use sqlx::postgres::PgPool;
