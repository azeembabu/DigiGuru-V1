//! `/api/v1/admin/programs` — CRUD on `programs`.
//!
//! Creation has no target `program_id` to scope against, so it is
//! super-admin only. Read/update are scope-checked once a `program_id`
//! exists — a sub-admin's `sub_admin_scopes` rows are the only thing that
//! can ever widen that.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use dg_core::{Capability, EntityStatus, PublicError, Role};
use dg_db::models::programs;

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct ProgramResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub status: EntityStatus,
}

impl From<programs::Program> for ProgramResponse {
    fn from(p: programs::Program) -> Self {
        Self { id: p.id.into_uuid(), code: p.code, name: p.name, description: p.description, status: p.status }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateProgramRequest {
    #[validate(length(min = 2, max = 20))]
    pub code: String,
    #[validate(length(min = 2, max = 200))]
    pub name: String,
    pub description: Option<String>,
}

pub async fn create_program(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Json(payload): Json<CreateProgramRequest>,
) -> Result<Json<ProgramResponse>, PublicError> {
    // No existing program_id to scope against yet — super-admin only.
    if actor.role != Role::SuperAdmin {
        return Err(PublicError::Forbidden);
    }
    payload.validate().map_err(|_| PublicError::validation("code", "invalid program fields"))?;

    let program = programs::create(&state.pool, &payload.code, &payload.name, payload.description.as_deref())
        .await
        .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.program_created",
        None,
        None,
        Some(serde_json::json!({ "program_id": program.id })),
    )
    .await;

    Ok(Json(program.into()))
}

pub async fn list_programs(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
) -> Result<Json<Vec<ProgramResponse>>, PublicError> {
    actor.require(Capability::ManagePrograms)?;

    let all = programs::list(&state.pool).await.map_err(PublicError::from)?;
    let visible = match actor.role {
        Role::SuperAdmin => all,
        Role::SubAdmin => all.into_iter().filter(|p| actor.in_scope(p.id)).collect(),
        Role::Student => return Err(PublicError::Forbidden),
    };

    Ok(Json(visible.into_iter().map(ProgramResponse::from).collect()))
}

pub async fn get_program(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<ProgramResponse>, PublicError> {
    let program_id = dg_core::ProgramId::from(id);
    actor.require_scoped(Capability::ManagePrograms, program_id)?;

    let program = programs::find_by_id(&state.pool, program_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    Ok(Json(program.into()))
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateProgramRequest {
    #[validate(length(min = 2, max = 200))]
    pub name: String,
    pub description: Option<String>,
    pub status: EntityStatus,
}

pub async fn update_program(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateProgramRequest>,
) -> Result<(), PublicError> {
    let program_id = dg_core::ProgramId::from(id);
    actor.require_scoped(Capability::ManagePrograms, program_id)?;
    payload.validate().map_err(|_| PublicError::validation("name", "invalid program fields"))?;

    programs::update(&state.pool, program_id, &payload.name, payload.description.as_deref(), payload.status)
        .await
        .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.program_updated",
        None,
        None,
        Some(serde_json::json!({ "program_id": program_id })),
    )
    .await;

    Ok(())
}
