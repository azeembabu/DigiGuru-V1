//! `/api/v1/admin/programs` — CRUD on `programs`.
//!
//! Creation has no target `program_id` to scope against, so it is
//! super-admin only. Read/update are scope-checked once a `program_id`
//! exists — a sub-admin's `sub_admin_scopes` rows are the only thing that
//! can ever widen that.

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
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

/// Default page size for the programs catalogue. Higher than the 50 used
/// for `students`/`users` deliberately: this route shipped unpaginated, and
/// a realistic programs table fits in one page — so an existing caller that
/// sends no `limit` keeps seeing everything, and `X-Total-Count` tells it
/// when that stops being true.
const DEFAULT_PROGRAM_PAGE: i64 = super::MAX_PAGE_LIMIT;

#[derive(Debug, Deserialize)]
pub struct ListProgramsQuery {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

pub async fn list_programs(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<ListProgramsQuery>,
) -> Result<(HeaderMap, Json<Vec<ProgramResponse>>), PublicError> {
    actor.require(Capability::ManagePrograms)?;

    let page = super::page(query.limit, query.offset, DEFAULT_PROGRAM_PAGE)?;
    let q = super::search_term(query.q.as_deref());

    // The scope filter goes into the query, not over its result: filtering a
    // page after fetching it would give a sub-admin short pages that do not
    // line up with any offset into its own visible set.
    let program_scope: Option<&[dg_core::ProgramId]> = match actor.role {
        Role::SuperAdmin => None,
        Role::SubAdmin => Some(&actor.scopes),
        Role::Student => return Err(PublicError::Forbidden),
    };

    let total = programs::count(&state.pool, program_scope, q)
        .await
        .map_err(PublicError::from)?;
    let rows = programs::list(&state.pool, program_scope, q, page.limit, page.offset)
        .await
        .map_err(PublicError::from)?;

    Ok((
        super::total_count(total),
        Json(rows.into_iter().map(ProgramResponse::from).collect()),
    ))
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
