//! Opaque, high-entropy tokens for refresh tokens and password-reset links.
//!
//! The raw value is returned to the client (refresh token, as a cookie) or
//! emailed (reset token) exactly once; only its hash is ever persisted, so a
//! database read alone can never be replayed as a live credential.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use sha2::{Digest, Sha256};

/// Generate a fresh 256-bit random token, base64url-encoded (no padding).
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Hash a raw token for storage/lookup. SHA-256 (not argon2id) is
/// appropriate here: these tokens are already 256 bits of randomness, not a
/// low-entropy secret vulnerable to a dictionary attack — the hash exists
/// only so a raw DB read can't be replayed directly, and `secret` (the
/// process's `JWT_REFRESH_SECRET`) is mixed in so a leaked DB dump alone is
/// still not enough to forge a valid hash for a chosen token.
pub fn hash_token(secret: &str, raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(raw.as_bytes());
    format!("{:x}", hasher.finalize())
}
