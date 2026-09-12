//! `GET /api/v1/auth/sessions` and `DELETE /api/v1/auth/sessions/:id` — list
//! and revoke the caller's own `auth_sessions` rows. Never another user's:
//! `auth_sessions::revoke_for_user` checks ownership at the SQL layer.

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use dg_core::{Capability, PublicError};
use dg_db::models::auth_sessions;

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub id: Uuid,
    pub device_info: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub async fn list_sessions(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
) -> Result<Json<Vec<SessionSummary>>, PublicError> {
    actor.require(Capability::ManageOwnSessions)?;

    let sessions = auth_sessions::list_active_for_user(&state.pool, actor.user_id)
        .await
        .map_err(PublicError::from)?;

    Ok(Json(
        sessions
            .into_iter()
            .map(|s| SessionSummary {
                id: s.id,
                device_info: s.device_info,
                ip_address: s.ip_address,
                created_at: s.created_at,
                expires_at: s.expires_at,
            })
            .collect(),
    ))
}

pub async fn revoke_session(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(session_id): Path<Uuid>,
) -> Result<(), PublicError> {
    actor.require(Capability::ManageOwnSessions)?;

    let revoked = auth_sessions::revoke_for_user(&state.pool, session_id, actor.user_id)
        .await
        .map_err(PublicError::from)?;

    if !revoked {
        return Err(PublicError::NotFound);
    }

    crate::audit::log(&state, Some(actor.user_id), "auth.session_revoked", None, None, None).await;
    Ok(())
}
