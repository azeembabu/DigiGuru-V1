//! `/api/v1/admin/students` — read and admin-only academic-field updates.
//! Student self-registration is `POST /api/v1/auth/signup`, not here.
//!
//! Sub-admins only ever see/touch students inside their `sub_admin_scopes`
//! programs — enforced by filtering `list_students` at the SQL layer
//! (`dg_db::models::students::list`) and by scope-checking every
//! single-student handler against that student's `program_id`.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::{BlockId, Capability, LscId, ProgramId, PublicError, Role, SemesterId, StudentId};
use dg_db::models::{blocks, lscs, programs, semesters, students};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct StudentResponse {
    pub id: Uuid,
    pub user_id: Uuid,
    pub full_name: String,
    pub roll_number: String,
    pub phone_number: String,
    pub program_id: Uuid,
    pub semester_id: Uuid,
    pub lsc_id: Uuid,
    pub current_block_id: Option<Uuid>,
    pub is_first_login: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<students::Student> for StudentResponse {
    fn from(s: students::Student) -> Self {
        Self {
            id: s.id.into_uuid(),
            user_id: s.user_id.into_uuid(),
            full_name: s.full_name,
            roll_number: s.roll_number,
            phone_number: s.phone_number,
            program_id: s.program_id.into_uuid(),
            semester_id: s.semester_id.into_uuid(),
            lsc_id: s.lsc_id.into_uuid(),
            current_block_id: s.current_block_id.map(BlockId::into_uuid),
            is_first_login: s.is_first_login,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListStudentsQuery {
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

pub async fn list_students(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<ListStudentsQuery>,
) -> Result<Json<Vec<StudentResponse>>, PublicError> {
    actor.require(Capability::ManageStudents)?;

    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);

    let program_scope: Option<&[ProgramId]> = match actor.role {
        Role::SuperAdmin => None,
        Role::SubAdmin => Some(&actor.scopes),
        Role::Student => return Err(PublicError::Forbidden),
    };

    let rows = students::list(&state.pool, program_scope, limit, offset)
        .await
        .map_err(PublicError::from)?;

    Ok(Json(rows.into_iter().map(StudentResponse::from).collect()))
}

pub async fn get_student(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<StudentResponse>, PublicError> {
    actor.require(Capability::ManageStudents)?;

    let student = students::find_by_id(&state.pool, StudentId::from(id))
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManageStudents, student.program_id)?;

    Ok(Json(student.into()))
}

#[derive(Debug, Deserialize)]
pub struct UpdateAcademicRequest {
    pub program_id: Uuid,
    pub semester_id: Uuid,
    pub lsc_id: Uuid,
}

/// Admin-only academic-field update — `program_id`/`semester_id`/`lsc_id`
/// are never reachable from the student self-service `PATCH /me/profile`
/// path (`IMPLEMENTATION_PLAN.md` §4.1 item 3).
///
/// Per §4.1 item 6, this must return `409 SESSION_ACTIVE` while a learning
/// session is live; see the same stub note as `me/profile.rs` — the
/// `learning_sessions` write-path doesn't exist yet, so the check below is a
/// placeholder that always returns `false`, and the response contract is
/// exercised for real once Phase 3 lands session-open.
pub async fn update_student_academic(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAcademicRequest>,
) -> Result<(), PublicError> {
    actor.require(Capability::ManageStudents)?;

    let student_id = StudentId::from(id);
    let student = students::find_by_id(&state.pool, student_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    // Must be in scope for both the student's current program and the
    // target program — otherwise a sub-admin could move a student they own
    // into (or out of) a program they don't.
    actor.require_scoped(Capability::ManageStudents, student.program_id)?;
    let new_program_id = ProgramId::from(payload.program_id);
    actor.require_scoped(Capability::ManageStudents, new_program_id)?;

    if super::session_active_stub(&state, student_id).await? {
        return Err(PublicError::session_active());
    }

    if !programs::exists(&state.pool, new_program_id).await.map_err(PublicError::from)? {
        return Err(PublicError::Unprocessable {
            code: "INVALID_PROGRAM",
            message: "Selected program does not exist.".into(),
        });
    }
    let new_semester_id = SemesterId::from(payload.semester_id);
    if !semesters::belongs_to_program(&state.pool, new_semester_id, new_program_id)
        .await
        .map_err(PublicError::from)?
    {
        return Err(PublicError::Unprocessable {
            code: "INVALID_SEMESTER",
            message: "Selected semester does not belong to the selected program.".into(),
        });
    }
    let new_lsc_id = LscId::from(payload.lsc_id);
    if lscs::find_by_id(&state.pool, new_lsc_id).await.map_err(PublicError::from)?.is_none() {
        return Err(PublicError::Unprocessable {
            code: "INVALID_LSC",
            message: "Selected learner support centre does not exist.".into(),
        });
    }

    students::update_academic(&state.pool, student_id, new_program_id, new_semester_id, new_lsc_id)
        .await
        .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.student_academic_updated",
        None,
        None,
        Some(serde_json::json!({ "student_id": student_id, "program_id": new_program_id })),
    )
    .await;

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct SetCurrentBlockRequest {
    pub block_id: Uuid,
}

pub async fn set_current_block(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
    Json(payload): Json<SetCurrentBlockRequest>,
) -> Result<(), PublicError> {
    actor.require(Capability::ManageStudents)?;

    let student_id = StudentId::from(id);
    let student = students::find_by_id(&state.pool, student_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManageStudents, student.program_id)?;

    let block_id = BlockId::from(payload.block_id);
    if blocks::find_by_id(&state.pool, block_id).await.map_err(PublicError::from)?.is_none() {
        return Err(PublicError::Unprocessable {
            code: "INVALID_BLOCK",
            message: "Selected block does not exist.".into(),
        });
    }

    students::set_current_block(&state.pool, student_id, block_id)
        .await
        .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.student_block_updated",
        None,
        None,
        Some(serde_json::json!({ "student_id": student_id, "block_id": block_id })),
    )
    .await;

    Ok(())
}
