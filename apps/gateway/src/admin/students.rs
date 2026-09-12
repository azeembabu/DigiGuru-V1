//! `/api/v1/admin/students` — read and admin-only academic-field updates.
//! Student self-registration is `POST /api/v1/auth/signup`, not here.
//!
//! Sub-admins only ever see/touch students inside their `sub_admin_scopes`
//! programs — enforced by filtering `list_students` at the SQL layer
//! (`dg_db::models::students::list`) and by scope-checking every
//! single-student handler against that student's `program_id`.

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::{
    BlockId, Capability, LscId, ProgramId, PublicError, Role, SemesterId, StudentId, UserStatus,
};
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
    /// The semester's number and name, denormalised from the join this
    /// query already makes. Semesters are otherwise listable only per
    /// program, so a students table would need a fan-out across the whole
    /// catalogue to name the semester on each row.
    pub semester_number: i16,
    pub semester_name: String,
    pub lsc_id: Uuid,
    pub current_block_id: Option<Uuid>,
    pub is_first_login: bool,
    /// The linked account's lifecycle (`users.status`) — the same field the
    /// `status` query parameter filters on, so the console can display what
    /// it filtered by.
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<students::StudentDetail> for StudentResponse {
    fn from(s: students::StudentDetail) -> Self {
        Self {
            id: s.id.into_uuid(),
            user_id: s.user_id.into_uuid(),
            full_name: s.full_name,
            roll_number: s.roll_number,
            phone_number: s.phone_number,
            program_id: s.program_id.into_uuid(),
            semester_id: s.semester_id.into_uuid(),
            semester_number: s.semester_number,
            semester_name: s.semester_name,
            lsc_id: s.lsc_id.into_uuid(),
            current_block_id: s.current_block_id.map(BlockId::into_uuid),
            is_first_login: s.is_first_login,
            status: s.status,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListStudentsQuery {
    /// Case-insensitive substring search over full name, roll number, and
    /// login email.
    #[serde(default)]
    pub q: Option<String>,
    /// Account lifecycle filter. `students` has no status column — this
    /// filters the linked `users.status`, which is where a student's
    /// active/inactive/suspended state actually lives.
    #[serde(default)]
    pub status: Option<UserStatus>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

pub async fn list_students(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<ListStudentsQuery>,
) -> Result<(HeaderMap, Json<Vec<StudentResponse>>), PublicError> {
    actor.require(Capability::ManageStudents)?;

    let page = super::page(query.limit, query.offset, 50)?;
    let q = super::search_term(query.q.as_deref());

    let program_scope: Option<&[ProgramId]> = match actor.role {
        Role::SuperAdmin => None,
        Role::SubAdmin => Some(&actor.scopes),
        Role::Student => return Err(PublicError::Forbidden),
    };

    let total = students::count(&state.pool, program_scope, q, query.status)
        .await
        .map_err(PublicError::from)?;
    let rows = students::list(
        &state.pool,
        program_scope,
        q,
        query.status,
        page.limit,
        page.offset,
    )
    .await
    .map_err(PublicError::from)?;

    Ok((
        super::total_count(total),
        Json(rows.into_iter().map(StudentResponse::from).collect()),
    ))
}

pub async fn get_student(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<StudentResponse>, PublicError> {
    actor.require(Capability::ManageStudents)?;

    let student = students::find_detail_by_id(&state.pool, StudentId::from(id))
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

    // Same guard as `update_student_academic`: a block change is an academic
    // context change, so both mutation paths must answer `409 SESSION_ACTIVE`
    // identically or a console cannot state one rule for either. Stub today
    // (see `admin/mod.rs`), real once Phase 3 writes `learning_sessions`.
    if super::session_active_stub(&state, student_id).await? {
        return Err(PublicError::session_active());
    }

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
