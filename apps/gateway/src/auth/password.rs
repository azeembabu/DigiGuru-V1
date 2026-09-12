//! Argon2id password hashing/verification (`.claude/rules/security.md`).

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("failed to hash password")]
    Hash,
    #[error("invalid credentials")]
    Verify,
}

pub fn hash_password(plain: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| PasswordError::Hash)
}

/// Constant-time verification against a stored argon2id hash. Never logs
/// the plaintext or the hash on failure — only the caller's own audit-log
/// call (with no PII) records that a login attempt failed.
pub fn verify_password(plain: &str, hash: &str) -> Result<(), PasswordError> {
    let parsed = PasswordHash::new(hash).map_err(|_| PasswordError::Verify)?;
    Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .map_err(|_| PasswordError::Verify)
}
