//! `audit_logs` — one row per admin mutation and per auth event.
//!
//! **`metadata` must never carry PII** (H-43): no email, phone, or full
//! name. Log entity ids (`student_id`, `program_id`, ...) instead of the
//! values they identify. This module does not enforce that at the type
//! level — callers (`apps/gateway/src/audit.rs`) are responsible for only
//! ever constructing PII-free `serde_json::Value` payloads.

use chrono::{DateTime, Utc};
use dg_core::UserId;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct AuditLog {
    pub id: i64,
    pub user_id: Option<UserId>,
    pub action: String,
    pub ip_address: Option<String>,
    pub device_info: Option<String>,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
}

pub async fn insert(
    pool: &PgPool,
    user_id: Option<UserId>,
    action: &str,
    ip_address: Option<std::net::IpAddr>,
    device_info: Option<&str>,
    metadata: Option<Value>,
) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO audit_logs (user_id, action, ip_address, device_info, metadata)
        VALUES ($1, $2, $3::text::inet, $4, $5)
        "#,
        user_id.map(Uuid::from),
        action,
        ip_address.map(|ip| ip.to_string()),
        device_info,
        metadata
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}
