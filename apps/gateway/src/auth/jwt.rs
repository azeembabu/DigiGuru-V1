//! Access-token JWT: short-lived (15 min), signed HS256, carries `sub`
//! (user id) and `role` only — never PII (`.claude/rules/security.md`).

use chrono::{Duration, Utc};
use dg_core::Role;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const ACCESS_TOKEN_TTL_MINUTES: i64 = 15;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessClaims {
    /// `users.id`.
    pub sub: Uuid,
    pub role: Role,
    pub iat: i64,
    pub exp: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum JwtError {
    #[error("failed to encode access token")]
    Encode,
    #[error("access token missing, malformed, or expired")]
    Invalid,
}

pub fn issue_access_token(user_id: Uuid, role: Role, secret: &str) -> Result<String, JwtError> {
    let now = Utc::now();
    let claims = AccessClaims {
        sub: user_id,
        role,
        iat: now.timestamp(),
        exp: (now + Duration::minutes(ACCESS_TOKEN_TTL_MINUTES)).timestamp(),
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| JwtError::Encode)
}

pub fn verify_access_token(token: &str, secret: &str) -> Result<AccessClaims, JwtError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    decode::<AccessClaims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation)
        .map(|data| data.claims)
        .map_err(|_| JwtError::Invalid)
}
