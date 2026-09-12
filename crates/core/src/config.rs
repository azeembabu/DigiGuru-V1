//! Process configuration loaded from environment variables.
//!
//! No `.env` loading here on purpose — that's a binary-level concern
//! (`apps/gateway` decides whether to load a `.env` file via `dotenvy`).
//! This module only defines what variables are required and produces a
//! clear, specific error naming the missing one.

/// Required process configuration, read once at startup.
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub qdrant_url: String,
    /// Signs/verifies the short-lived (15 min) access-token JWT.
    pub jwt_access_secret: String,
    /// Signs/verifies... actually verifies nothing by itself: rotating
    /// refresh tokens are opaque random values, hashed with SHA-256 before
    /// storage in `auth_sessions.refresh_token_hash`. This secret is mixed
    /// into that hash (HMAC-style) so a raw DB read of the hash column alone
    /// cannot be replayed without it.
    pub jwt_refresh_secret: String,
    pub port: u16,
    /// Gemini API key for `gemini-embedding-001` (Phase 2 ingestion) and,
    /// later, Gemini Live (Phase 3). Optional for now: the ingestion
    /// pipeline runs against `rag::embed::StubEmbedder` until a real key is
    /// available in this environment (`// TODO(phase2-api-key)` in
    /// `crates/rag/src/embed.rs`).
    pub gemini_api_key: Option<String>,
}

/// A single missing or invalid environment variable.
#[derive(Debug, thiserror::Error)]
#[error("missing or invalid environment variable: {0}")]
pub struct ConfigError(pub &'static str);

impl Config {
    /// Load configuration from the process environment.
    ///
    /// Returns a `ConfigError` naming the first missing variable rather than
    /// panicking, so callers can fail startup with a clear message.
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Config {
            database_url: require_env("DATABASE_URL")?,
            redis_url: require_env("REDIS_URL")?,
            qdrant_url: require_env("QDRANT_URL")?,
            jwt_access_secret: require_env("JWT_ACCESS_SECRET")?,
            jwt_refresh_secret: require_env("JWT_REFRESH_SECRET")?,
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            gemini_api_key: std::env::var("GEMINI_API_KEY").ok(),
        })
    }
}

fn require_env(key: &'static str) -> Result<String, ConfigError> {
    std::env::var(key).map_err(|_| ConfigError(key))
}
