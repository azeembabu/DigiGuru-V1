//! Shared application state, injected into every handler via axum's
//! `State` extractor and into the `Actor` extractor for JWT verification +
//! sub-admin scope loading.

use std::sync::Arc;

use dg_core::config::Config;
use dg_db::PgPool;
use redis::aio::ConnectionManager;

#[derive(Clone)]
pub struct AppState(pub Arc<AppStateInner>);

pub struct AppStateInner {
    pub pool: PgPool,
    // Connected at startup and held for Phase 3: the NN-3 quota ledger and the
    // `sess:{session_id}` state both live in Redis. Nothing reads it yet, so
    // clippy calls it dead -- but dropping it would mean re-establishing the
    // connection manager later for no gain, and a failure to reach Redis is
    // worth discovering at boot rather than on the first classroom session.
    #[allow(dead_code)]
    pub redis: ConnectionManager,
    pub config: Config,
}

impl AppState {
    pub fn new(pool: PgPool, redis: ConnectionManager, config: Config) -> Self {
        Self(Arc::new(AppStateInner { pool, redis, config }))
    }
}

impl std::ops::Deref for AppState {
    type Target = AppStateInner;

    fn deref(&self) -> &AppStateInner {
        &self.0
    }
}
