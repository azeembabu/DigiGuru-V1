//! Audit logging helper. Every admin mutation and every auth event writes an
//! `audit_logs` row through this function — `.claude/rules/security.md`:
//! "**PII never leaves the process**" applies here too, so `metadata` must
//! only ever carry entity ids and other non-identifying fields, never an
//! email, phone number, or full name.

use std::net::IpAddr;

use dg_core::UserId;
use serde_json::Value;

use crate::state::AppState;

/// Write one audit-log row. Errors are logged and swallowed — a failed
/// audit write must never fail (or roll back) the request it is describing.
pub async fn log(
    state: &AppState,
    user_id: Option<UserId>,
    action: &str,
    ip_address: Option<IpAddr>,
    device_info: Option<&str>,
    metadata: Option<Value>,
) {
    if let Err(err) =
        dg_db::models::audit_logs::insert(&state.pool, user_id, action, ip_address, device_info, metadata)
            .await
    {
        tracing::error!(error = %err, action = %action, "failed to write audit log");
    }
}
