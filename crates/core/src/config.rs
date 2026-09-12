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
    pub jwt_secret: String,
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
            jwt_secret: require_env("JWT_SECRET")?,
        })
    }
}

fn require_env(key: &'static str) -> Result<String, ConfigError> {
    std::env::var(key).map_err(|_| ConfigError(key))
}
