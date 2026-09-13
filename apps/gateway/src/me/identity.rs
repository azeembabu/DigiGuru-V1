//! `GET /api/v1/me` — who the caller is, for **any** authenticated role.
//!
//! Distinct from `/me/context`, which is the student's academic "Remember
//! Me" payload and `404`s for an admin (no `students` row). An admin shell
//! needs identity, not academic context: without this route it could only
//! learn its own role by decoding the access JWT client-side, which the
//! `HttpOnly` cookie exists to prevent.
//!
//! Self-only, so no new capability: the handler reads `actor.user_id` and
//! nothing else can be addressed through it. `scopes` is resolved with the
//! program `code`/`name` joined in — a bare `Vec<Uuid>` (what
//! `admin/users.rs::list_scopes` returns) would force the shell into a
//! round trip per id, and a sub-admin cannot call that route anyway.

use axum::{extract::State, Json};
use serde::Serialize;
use uuid::Uuid;

use dg_core::{PublicError, Role, UserStatus};
use dg_db::models::{admins, students, sub_admin_scopes, users};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct ScopeSummary {
    pub program_id: Uuid,
    pub code: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub id: Uuid,
    pub email: String,
    /// From the role's profile row (`admins` or `students`); `null` if the
    /// profile row is missing, which is a data-integrity problem rather than
    /// a reason to fail the whole identity lookup.
    pub full_name: Option<String>,
    pub role: Role,
    pub status: UserStatus,
    /// Always `[]` for a super-admin (unscoped by definition) and for a
    /// student (never holds a scoped capability).
    pub scopes: Vec<ScopeSummary>,
}

pub async fn get_me(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
) -> Result<Json<MeResponse>, PublicError> {
    let user = users::find_by_id(&state.pool, actor.user_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::Unauthorized)?;

    let full_name = match user.role {
        Role::SuperAdmin | Role::SubAdmin => admins::find_by_user_id(&state.pool, user.id)
            .await
            .map_err(PublicError::from)?
            .map(|a| a.full_name),
        Role::Student => students::find_by_user_id(&state.pool, user.id)
            .await
            .map_err(PublicError::from)?
            .map(|s| s.full_name),
    };

    let scopes = match user.role {
        Role::SubAdmin => sub_admin_scopes::list_with_programs(&state.pool, user.id)
            .await
            .map_err(PublicError::from)?
            .into_iter()
            .map(|s| ScopeSummary {
                program_id: s.program_id.into_uuid(),
                code: s.code,
                name: s.name,
            })
            .collect(),
        Role::SuperAdmin | Role::Student => Vec::new(),
    };

    Ok(Json(MeResponse {
        id: user.id.into_uuid(),
        email: user.email,
        full_name,
        role: user.role,
        status: user.status,
        scopes,
    }))
}
