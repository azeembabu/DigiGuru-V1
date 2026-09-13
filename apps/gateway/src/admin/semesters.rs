//! `/api/v1/admin/semesters` — CRUD on `semesters`. Scope is always checked
//! against the semester's `program_id`, never the semester's own id.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use dg_core::{Capability, EntityStatus, PublicError, ProgramId};
use dg_db::models::semesters;

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct SemesterResponse {
    pub id: Uuid,
    pub program_id: Uuid,
    pub semester_number: i16,
    pub name: String,
    pub status: EntityStatus,
}

impl From<semesters::Semester> for SemesterResponse {
    fn from(s: semesters::Semester) -> Self {
        Self {
            id: s.id.into_uuid(),
            program_id: s.program_id.into_uuid(),
            semester_number: s.semester_number,
            name: s.name,
            status: s.status,
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateSemesterRequest {
    pub program_id: Uuid,
    #[validate(range(min = 1, max = 12))]
    pub semester_number: i16,
    #[validate(length(min = 2, max = 100))]
    pub name: String,
}

pub async fn create_semester(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Json(payload): Json<CreateSemesterRequest>,
) -> Result<Json<SemesterResponse>, PublicError> {
    payload.validate().map_err(|_| PublicError::validation("semester_number", "must be 1-12"))?;

    let program_id = ProgramId::from(payload.program_id);
    actor.require_scoped(Capability::ManagePrograms, program_id)?;

    let semester = semesters::create(&state.pool, program_id, payload.semester_number, &payload.name)
        .await
        .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.semester_created",
        None,
        None,
        Some(serde_json::json!({ "semester_id": semester.id, "program_id": program_id })),
    )
    .await;

    Ok(Json(semester.into()))
}

pub async fn get_semester(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<SemesterResponse>, PublicError> {
    let semester = semesters::find_by_id(&state.pool, dg_core::SemesterId::from(id))
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManagePrograms, semester.program_id)?;

    Ok(Json(semester.into()))
}

pub async fn list_semesters_for_program(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(program_id): Path<Uuid>,
) -> Result<Json<Vec<SemesterResponse>>, PublicError> {
    let program_id = ProgramId::from(program_id);
    actor.require_scoped(Capability::ManagePrograms, program_id)?;

    let rows = semesters::list_by_program(&state.pool, program_id)
        .await
        .map_err(PublicError::from)?;

    Ok(Json(rows.into_iter().map(SemesterResponse::from).collect()))
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateSemesterRequest {
    #[validate(length(min = 2, max = 100))]
    pub name: String,
    pub status: EntityStatus,
}

pub async fn update_semester(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSemesterRequest>,
) -> Result<(), PublicError> {
    payload.validate().map_err(|_| PublicError::validation("name", "invalid semester fields"))?;

    let semester_id = dg_core::SemesterId::from(id);
    let semester = semesters::find_by_id(&state.pool, semester_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManagePrograms, semester.program_id)?;

    semesters::update(&state.pool, semester_id, &payload.name, payload.status)
        .await
        .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.semester_updated",
        None,
        None,
        Some(serde_json::json!({ "semester_id": semester_id })),
    )
    .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use dg_core::{Actor, Capability, ProgramId, Role, UserId};

    /// The rule `get_semester` applies once the row's owning program is
    /// resolved — identical to its create/list/update siblings, which is the
    /// property this matrix pins.
    fn authorize(actor: &Actor, program_id: ProgramId) -> Result<(), dg_core::PublicError> {
        actor.require_scoped(Capability::ManagePrograms, program_id)
    }

    #[test]
    fn super_admin_may_read_any_semester() {
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert!(authorize(&actor, ProgramId::new()).is_ok());
    }

    #[test]
    fn sub_admin_may_read_a_semester_in_its_scope() {
        let scoped = ProgramId::new();
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![scoped]);
        assert!(authorize(&actor, scoped).is_ok());
    }

    #[test]
    fn sub_admin_is_forbidden_a_semester_outside_its_scope() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        let err = authorize(&actor, ProgramId::new()).expect_err("out of scope");
        assert_eq!(err.code(), "FORBIDDEN");
    }

    #[test]
    fn student_is_forbidden_every_semester() {
        let program_id = ProgramId::new();
        let actor = Actor::new(UserId::new(), Role::Student, vec![program_id]);
        let err = authorize(&actor, program_id).expect_err("students never read admin rows");
        assert_eq!(err.code(), "FORBIDDEN");
    }
}
