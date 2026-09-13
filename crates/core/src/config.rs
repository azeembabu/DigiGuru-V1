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
    /// Hard cap on a document-upload request body, in bytes.
    ///
    /// `.claude/rules/security.md` requires an explicit size cap on uploads;
    /// without one the effective cap is axum's 2 MiB `DEFAULT_BODY_LIMIT`,
    /// which is a framework default nobody chose and which rejects every
    /// real textbook. Applied to the upload route only — raising it globally
    /// would widen the DoS surface on `/auth/*` and every JSON endpoint.
    pub max_upload_bytes: usize,
}

/// 64 MiB. A born-digital textbook PDF is a few MB; a 300-page scanned
/// Malayalam book at 300 dpi lands in the 30-50 MB range, so this clears the
/// realistic worst case with headroom while still bounding what one request
/// can make the gateway buffer in memory.
pub const DEFAULT_MAX_UPLOAD_BYTES: usize = 64 * 1024 * 1024;

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
            // Optional, like PORT: a deployment that needs a different cap
            // sets it, and an unparseable value falls back to the default
            // rather than aborting startup over a tuning knob.
            max_upload_bytes: std::env::var("MAX_UPLOAD_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|n| *n > 0)
                .unwrap_or(DEFAULT_MAX_UPLOAD_BYTES),
        })
    }
}

fn require_env(key: &'static str) -> Result<String, ConfigError> {
    std::env::var(key).map_err(|_| ConfigError(key))
}
