//! `/api/v1/admin/lscs` — CRUD on `lscs`.
//!
//! LSCs have no `program_id` and are not covered by `sub_admin_scopes` (the
//! schema does not relate an LSC to a program) — so there is nothing to
//! scope. Any admin (super or sub) may list LSCs as reference data; only a
//! super-admin may create or update one, matching the pattern used for
//! program creation in `admin/programs.rs`.

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use dg_core::{Capability, EntityStatus, PublicError, Role};
use dg_db::models::lscs;

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct LscResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub location: Option<String>,
    pub status: EntityStatus,
}

impl From<lscs::Lsc> for LscResponse {
    fn from(l: lscs::Lsc) -> Self {
        Self { id: l.id.into_uuid(), code: l.code, name: l.name, location: l.location, status: l.status }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateLscRequest {
    #[validate(length(min = 2, max = 20))]
    pub code: String,
    #[validate(length(min = 2, max = 200))]
    pub name: String,
    pub location: Option<String>,
}

pub async fn create_lsc(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Json(payload): Json<CreateLscRequest>,
) -> Result<Json<LscResponse>, PublicError> {
    if actor.role != Role::SuperAdmin {
        return Err(PublicError::Forbidden);
    }
    payload.validate().map_err(|_| PublicError::validation("code", "invalid LSC fields"))?;

    let lsc = lscs::create(&state.pool, &payload.code, &payload.name, payload.location.as_deref())
        .await
        .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.lsc_created",
        None,
        None,
        Some(serde_json::json!({ "lsc_id": lsc.id })),
    )
    .await;

    Ok(Json(lsc.into()))
}

/// Same reasoning as `admin/programs.rs`: this route shipped unpaginated,
/// so its default page is the ceiling rather than 50.
const DEFAULT_LSC_PAGE: i64 = super::MAX_PAGE_LIMIT;

#[derive(Debug, Deserialize)]
pub struct ListLscsQuery {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

pub async fn list_lscs(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<ListLscsQuery>,
) -> Result<(HeaderMap, Json<Vec<LscResponse>>), PublicError> {
    // Reference data, unscoped — any admin capability suffices.
    actor.require(Capability::ManagePrograms)?;

    let page = super::page(query.limit, query.offset, DEFAULT_LSC_PAGE)?;
    let q = super::search_term(query.q.as_deref());

    let total = lscs::count(&state.pool, q).await.map_err(PublicError::from)?;
    let rows = lscs::list(&state.pool, q, page.limit, page.offset)
        .await
        .map_err(PublicError::from)?;

    Ok((
        super::total_count(total),
        Json(rows.into_iter().map(LscResponse::from).collect()),
    ))
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLscRequest {
    #[validate(length(min = 2, max = 200))]
    pub name: String,
    pub location: Option<String>,
    pub status: EntityStatus,
}

pub async fn update_lsc(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateLscRequest>,
) -> Result<(), PublicError> {
    if actor.role != Role::SuperAdmin {
        return Err(PublicError::Forbidden);
    }
    payload.validate().map_err(|_| PublicError::validation("name", "invalid LSC fields"))?;

    let lsc_id = dg_core::LscId::from(id);
    lscs::update(&state.pool, lsc_id, &payload.name, payload.location.as_deref(), payload.status)
        .await
        .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.lsc_updated",
        None,
        None,
        Some(serde_json::json!({ "lsc_id": lsc_id })),
    )
    .await;

    Ok(())
}
