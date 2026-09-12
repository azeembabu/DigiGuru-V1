//! `/api/v1/admin/courses` — CRUD on `courses`. Scope is checked against
//! the course's `program_id`.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use dg_core::{Capability, ProgramId, PublicError, SemesterId};
use dg_db::models::{courses, semesters};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct CourseResponse {
    pub id: Uuid,
    pub program_id: Uuid,
    pub semester_id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}

impl From<courses::Course> for CourseResponse {
    fn from(c: courses::Course) -> Self {
        Self {
            id: c.id.into_uuid(),
            program_id: c.program_id.into_uuid(),
            semester_id: c.semester_id.into_uuid(),
            code: c.code,
            name: c.name,
            description: c.description,
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateCourseRequest {
    pub program_id: Uuid,
    pub semester_id: Uuid,
    #[validate(length(min = 2, max = 20))]
    pub code: String,
    #[validate(length(min = 2, max = 200))]
    pub name: String,
    pub description: Option<String>,
}

pub async fn create_course(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Json(payload): Json<CreateCourseRequest>,
) -> Result<Json<CourseResponse>, PublicError> {
    payload.validate().map_err(|_| PublicError::validation("code", "invalid course fields"))?;

    let program_id = ProgramId::from(payload.program_id);
    let semester_id = SemesterId::from(payload.semester_id);
    actor.require_scoped(Capability::ManagePrograms, program_id)?;

    if !semesters::belongs_to_program(&state.pool, semester_id, program_id)
        .await
        .map_err(PublicError::from)?
    {
        return Err(PublicError::Unprocessable {
            code: "INVALID_SEMESTER",
            message: "Semester does not belong to the given program.".into(),
        });
    }

    let course = courses::create(
        &state.pool,
        program_id,
        semester_id,
        &payload.code,
        &payload.name,
        payload.description.as_deref(),
    )
    .await
    .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.course_created",
        None,
        None,
        Some(serde_json::json!({ "course_id": course.id, "program_id": program_id })),
    )
    .await;

    Ok(Json(course.into()))
}

pub async fn list_courses_for_semester(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(semester_id): Path<Uuid>,
) -> Result<Json<Vec<CourseResponse>>, PublicError> {
    let semester_id = SemesterId::from(semester_id);
    let semester = semesters::find_by_id(&state.pool, semester_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManagePrograms, semester.program_id)?;

    let rows = courses::list_by_semester(&state.pool, semester_id)
        .await
        .map_err(PublicError::from)?;

    Ok(Json(rows.into_iter().map(CourseResponse::from).collect()))
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateCourseRequest {
    #[validate(length(min = 2, max = 200))]
    pub name: String,
    pub description: Option<String>,
}

pub async fn update_course(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateCourseRequest>,
) -> Result<(), PublicError> {
    payload.validate().map_err(|_| PublicError::validation("name", "invalid course fields"))?;

    let course_id = dg_core::CourseId::from(id);
    let course = courses::find_by_id(&state.pool, course_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManagePrograms, course.program_id)?;

    courses::update(&state.pool, course_id, &payload.name, payload.description.as_deref())
        .await
        .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.course_updated",
        None,
        None,
        Some(serde_json::json!({ "course_id": course_id })),
    )
    .await;

    Ok(())
}
