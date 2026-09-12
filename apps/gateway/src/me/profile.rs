//! `PATCH /api/v1/me/profile` — student self-service update.
//!
//! **Allow-list, not a filter**: `ProfileUpdateRequest` uses
//! `#[serde(deny_unknown_fields)]` so a request carrying any key other than
//! `full_name`/`phone_number` (e.g. `program_id`) is rejected as a `400`
//! validation error at deserialization time — it is never silently dropped
//! (`IMPLEMENTATION_PLAN.md` §4.1 item 3). Academic fields are admin-only and
//! have no path through this handler at all.

use axum::{extract::State, Json};
use serde::Deserialize;

use dg_core::{Capability, PublicError};
use dg_db::models::students;

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;
use crate::validation::normalize_indian_phone;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileUpdateRequest {
    pub full_name: String,
    pub phone_number: String,
}

pub async fn update_profile(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Json(payload): Json<ProfileUpdateRequest>,
) -> Result<(), PublicError> {
    actor.require(Capability::UpdateOwnProfile)?;

    let student = students::find_by_user_id(&state.pool, actor.user_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    if is_session_active(&state, student.id).await? {
        return Err(PublicError::session_active());
    }

    if payload.full_name.trim().len() < 2 {
        return Err(PublicError::validation("full_name", "must be at least 2 characters"));
    }
    let phone_number = normalize_indian_phone(&payload.phone_number)
        .map_err(|fe| PublicError::Validation(vec![fe]))?;

    students::update_self_service(&state.pool, student.id, payload.full_name.trim(), &phone_number)
        .await
        .map_err(PublicError::from)?;

    crate::audit::log(&state, Some(actor.user_id), "me.profile_updated", None, None, None).await;

    Ok(())
}

/// Whether `student_id` has a live `learning_sessions` row.
///
/// **Stub**: `learning_sessions` writes don't exist yet (Phase 3 opens
/// them) — this always returns `false` for now so the endpoint is callable
/// in Phase 1, but the `409 SESSION_ACTIVE` response contract above is
/// real and exercised once Phase 3 lands a genuine query here. Kept as its
/// own function specifically so that swap is a one-line change.
async fn is_session_active(
    _state: &AppState,
    _student_id: dg_core::StudentId,
) -> Result<bool, PublicError> {
    Ok(false)
}
