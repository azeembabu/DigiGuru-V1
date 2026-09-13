//! `/api/v1/reference/*` — unauthenticated, read-only academic lookups.
//!
//! Signup has to submit real `program_id` / `semester_id` / `lsc_id` values —
//! `IMPLEMENTATION_PLAN.md` §4.1 item 3 requires them to "resolve to existing
//! rows, not free text", and `auth::signup::SignupRequest` types them as
//! `Uuid`. An anonymous visitor on `/signup` therefore needs to read those
//! lists, but every existing list route lives under `/api/v1/admin/*` behind a
//! capability check, so the signup form had nothing it could call.
//!
//! These three routes close that gap and nothing more:
//!
//! - They are **read-only** and expose only `id` + display fields. No counts,
//!   no student data, no document or ingestion state.
//! - They return **active rows only**, so an archived program cannot be picked
//!   at signup — which also keeps them consistent with the existence checks
//!   `signup` already runs (`programs::exists`, `semesters::belongs_to_program`,
//!   `lscs::exists`), all of which require `status = 'active'`.
//! - Being public is deliberate and safe: a program/LSC catalogue is printed in
//!   the university prospectus. It is not PII and carries no authorisation
//!   meaning. `.claude/rules/security.md` restricts *administration* of these
//!   entities, not knowledge that they exist.
//!
//! Rate limiting for these sits with the `/auth/*` bucket described in
//! `.claude/rules/security.md`; they are cacheable and change rarely.

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use uuid::Uuid;

use dg_core::{EntityStatus, ProgramId, PublicError};
use dg_db::models::{lscs, programs, semesters};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/programs", get(list_programs))
        .route("/programs/{program_id}/semesters", get(list_semesters))
        .route("/lscs", get(list_lscs))
}

/// Minimal shape: an id to submit and a name to show. Deliberately not the
/// admin `ProgramResponse` — that one carries `status`, which has no meaning
/// to an anonymous caller and would only ever be `active` here.
#[derive(Debug, Serialize)]
pub struct ReferenceOption {
    pub id: Uuid,
    pub name: String,
}

async fn list_programs(
    State(state): State<AppState>,
) -> Result<Json<Vec<ReferenceOption>>, PublicError> {
    // `list_all`, not the paginated `list`: this is a signup form's option
    // list, not an admin console page. A `LIMIT` here would silently truncate
    // the programs a student can choose from.
    let rows = programs::list_all(&state.pool).await.map_err(PublicError::from)?;

    Ok(Json(
        rows.into_iter()
            .filter(|p| p.status == EntityStatus::Active)
            .map(|p| ReferenceOption {
                id: p.id.into_uuid(),
                name: p.name,
            })
            .collect(),
    ))
}

async fn list_semesters(
    State(state): State<AppState>,
    Path(program_id): Path<Uuid>,
) -> Result<Json<Vec<ReferenceOption>>, PublicError> {
    let rows = semesters::list_by_program(&state.pool, ProgramId(program_id))
        .await
        .map_err(PublicError::from)?;

    Ok(Json(
        rows.into_iter()
            .filter(|s| s.status == EntityStatus::Active)
            .map(|s| ReferenceOption {
                id: s.id.into_uuid(),
                name: s.name,
            })
            .collect(),
    ))
}

async fn list_lscs(State(state): State<AppState>) -> Result<Json<Vec<ReferenceOption>>, PublicError> {
    let rows = lscs::list_all(&state.pool).await.map_err(PublicError::from)?;

    Ok(Json(
        rows.into_iter()
            .filter(|l| l.status == EntityStatus::Active)
            .map(|l| ReferenceOption {
                id: l.id.into_uuid(),
                name: l.name,
            })
            .collect(),
    ))
}
