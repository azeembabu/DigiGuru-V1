//! `/api/v1/admin/users` — super-admin only: create admin users, list/
//! suspend users, and manage `sub_admin_scopes`.

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use dg_core::{ProgramId, PublicError, Role, UserId, UserStatus};
use dg_db::models::{admins, sub_admin_scopes, users};

use crate::auth::password::hash_password;
use crate::extractors::AuthenticatedActor;
use crate::me::identity::ScopeSummary;
use crate::state::AppState;

fn require_super_admin(actor: &dg_core::Actor) -> Result<(), PublicError> {
    if actor.role == Role::SuperAdmin {
        Ok(())
    } else {
        Err(PublicError::Forbidden)
    }
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub role: Role,
    pub status: UserStatus,
    pub email: String,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<users::User> for UserResponse {
    fn from(u: users::User) -> Self {
        Self {
            id: u.id.into_uuid(),
            role: u.role,
            status: u.status,
            email: u.email,
            last_login_at: u.last_login_at,
            created_at: u.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
    /// Case-insensitive substring search over `email` — the only
    /// identifying field on a `users` row; display names live on the
    /// role-specific `admins`/`students` rows.
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub role: Option<Role>,
    #[serde(default)]
    pub status: Option<UserStatus>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

pub async fn list_users(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<ListUsersQuery>,
) -> Result<(HeaderMap, Json<Vec<UserResponse>>), PublicError> {
    require_super_admin(&actor)?;

    let page = super::page(query.limit, query.offset, 50)?;
    let q = super::search_term(query.q.as_deref());

    let total = users::count(&state.pool, q, query.role, query.status)
        .await
        .map_err(PublicError::from)?;
    let rows = users::list(
        &state.pool,
        q,
        query.role,
        query.status,
        page.limit,
        page.offset,
    )
    .await
    .map_err(PublicError::from)?;

    Ok((
        super::total_count(total),
        Json(rows.into_iter().map(UserResponse::from).collect()),
    ))
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateAdminRequest {
    #[validate(email(message = "must be a valid email address"))]
    pub email: String,
    #[validate(length(min = 8, max = 128, message = "must be 8-128 characters"))]
    pub password: String,
    #[validate(length(min = 2, max = 120))]
    pub full_name: String,
    /// Only `super_admin` or `sub_admin` may be created here — `student`
    /// accounts are only ever created via `/auth/signup`.
    pub role: Role,
    /// Initial `sub_admin_scopes`. Ignored (must be empty) for `super_admin`.
    #[serde(default)]
    pub program_scopes: Vec<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct CreateAdminResponse {
    pub user_id: Uuid,
}

pub async fn create_admin(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Json(payload): Json<CreateAdminRequest>,
) -> Result<Json<CreateAdminResponse>, PublicError> {
    require_super_admin(&actor)?;
    payload.validate().map_err(|_| PublicError::validation("email", "invalid admin fields"))?;

    if payload.role == Role::Student {
        return Err(PublicError::validation("role", "student accounts are created via signup, not here"));
    }
    if payload.role == Role::SuperAdmin && !payload.program_scopes.is_empty() {
        return Err(PublicError::validation("program_scopes", "super_admin is not scoped"));
    }

    if users::find_by_email(&state.pool, &payload.email)
        .await
        .map_err(PublicError::from)?
        .is_some()
    {
        return Err(PublicError::validation("email", "already registered"));
    }

    let password_hash = hash_password(&payload.password).map_err(|err| {
        tracing::error!(error = %err, "password hashing failed");
        PublicError::Internal
    })?;

    let user = users::create(&state.pool, payload.role, &payload.email, &password_hash)
        .await
        .map_err(PublicError::from)?;

    admins::create(&state.pool, user.id, &payload.full_name)
        .await
        .map_err(PublicError::from)?;

    for raw_program_id in &payload.program_scopes {
        sub_admin_scopes::add_scope(&state.pool, user.id, ProgramId::from(*raw_program_id))
            .await
            .map_err(PublicError::from)?;
    }

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.user_created",
        None,
        None,
        Some(serde_json::json!({ "created_user_id": user.id, "role": payload.role })),
    )
    .await;

    Ok(Json(CreateAdminResponse { user_id: user.id.into_uuid() }))
}

#[derive(Debug, Deserialize)]
pub struct SetStatusRequest {
    pub status: UserStatus,
}

pub async fn set_user_status(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
    Json(payload): Json<SetStatusRequest>,
) -> Result<(), PublicError> {
    require_super_admin(&actor)?;

    let target = UserId::from(id);
    users::set_status(&state.pool, target, payload.status).await.map_err(PublicError::from)?;

    if payload.status != UserStatus::Active {
        dg_db::models::auth_sessions::revoke_all_for_user(&state.pool, target)
            .await
            .map_err(PublicError::from)?;
    }

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.user_status_changed",
        None,
        None,
        Some(serde_json::json!({ "target_user_id": target, "status": payload.status })),
    )
    .await;

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ScopeRequest {
    pub program_id: Uuid,
}

pub async fn add_scope(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
    Json(payload): Json<ScopeRequest>,
) -> Result<(), PublicError> {
    require_super_admin(&actor)?;

    let target = UserId::from(id);
    let program_id = ProgramId::from(payload.program_id);

    if !dg_db::models::programs::exists(&state.pool, program_id).await.map_err(PublicError::from)? {
        return Err(PublicError::Unprocessable {
            code: "INVALID_PROGRAM",
            message: "Selected program does not exist.".into(),
        });
    }

    sub_admin_scopes::add_scope(&state.pool, target, program_id).await.map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.scope_added",
        None,
        None,
        Some(serde_json::json!({ "target_user_id": target, "program_id": program_id })),
    )
    .await;

    Ok(())
}

pub async fn remove_scope(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path((id, program_id)): Path<(Uuid, Uuid)>,
) -> Result<(), PublicError> {
    require_super_admin(&actor)?;

    let target = UserId::from(id);
    let program_id = ProgramId::from(program_id);

    sub_admin_scopes::remove_scope(&state.pool, target, program_id).await.map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.scope_removed",
        None,
        None,
        Some(serde_json::json!({ "target_user_id": target, "program_id": program_id })),
    )
    .await;

    Ok(())
}

/// Returns each scope with its program `code`/`name` joined in, reusing the
/// exact shape `GET /api/v1/me` already returns for the caller's own scopes
/// — one scope shape across the API, not two.
///
/// This replaced a bare `Vec<Uuid>` response. A breaking change to a shipped
/// path, taken deliberately while the endpoint is unreleased and has a
/// single caller: resolving ids cost the console a client-side join against
/// the programs list, and any scope whose program it had not loaded
/// rendered as an unresolved placeholder.
pub async fn list_scopes(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ScopeSummary>>, PublicError> {
    require_super_admin(&actor)?;

    let target = UserId::from(id);
    let scopes = sub_admin_scopes::list_with_programs(&state.pool, target)
        .await
        .map_err(PublicError::from)?;

    Ok(Json(
        scopes
            .into_iter()
            .map(|s| ScopeSummary {
                program_id: s.program_id.into_uuid(),
                code: s.code,
                name: s.name,
            })
            .collect(),
    ))
}
