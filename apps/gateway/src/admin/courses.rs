//! `/api/v1/admin/courses` — CRUD on `courses`. Scope is checked against
//! the course's `program_id`.

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use dg_core::{Capability, ProgramId, PublicError, SemesterId};
use dg_db::models::{courses, programs, semesters};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct CourseResponse {
    pub id: Uuid,
    pub program_id: Uuid,
    pub semester_id: Uuid,
    /// The semester's number and name, denormalised from the join every
    /// course query already makes. Additive, and on *every* endpoint that
    /// returns a course, so there is one course shape: a flattened
    /// program-level course list would otherwise have to re-fetch the
    /// semester list purely to label its rows — the exact N+1 that list
    /// exists to remove.
    pub semester_number: i16,
    pub semester_name: String,
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
            semester_number: c.semester_number,
            semester_name: c.semester_name,
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

pub async fn get_course(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<CourseResponse>, PublicError> {
    let course = courses::find_by_id(&state.pool, dg_core::CourseId::from(id))
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManagePrograms, course.program_id)?;

    Ok(Json(course.into()))
}

#[derive(Debug, Deserialize)]
pub struct ListCoursesQuery {
    /// Case-insensitive substring match over course code and name.
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

/// Every course in a program, across all of its semesters — the flattened
/// content list the admin console drills into from a program.
///
/// This is purely a read-side convenience. The semester model is unchanged:
/// each row still carries its own `semester_id`, and the per-semester list
/// below stays exactly as it was.
pub async fn list_courses_for_program(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(program_id): Path<Uuid>,
    Query(query): Query<ListCoursesQuery>,
) -> Result<(HeaderMap, Json<Vec<CourseResponse>>), PublicError> {
    let program_id = ProgramId::from(program_id);
    actor.require_scoped(Capability::ManagePrograms, program_id)?;

    // `404` for a program that does not exist, matching the semesters
    // sibling — an empty array would otherwise be indistinguishable from a
    // typo'd id. The scope check runs first, so a sub-admin still cannot use
    // this to probe for programs outside its scope.
    if programs::find_by_id(&state.pool, program_id)
        .await
        .map_err(PublicError::from)?
        .is_none()
    {
        return Err(PublicError::NotFound);
    }

    let page = super::page(query.limit, query.offset, super::MAX_PAGE_LIMIT)?;
    let q = super::search_term(query.q.as_deref());

    let total = courses::count_by_program(&state.pool, program_id, q)
        .await
        .map_err(PublicError::from)?;
    let rows = courses::list_by_program(&state.pool, program_id, q, page.limit, page.offset)
        .await
        .map_err(PublicError::from)?;

    Ok((
        super::total_count(total),
        Json(rows.into_iter().map(CourseResponse::from).collect()),
    ))
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

#[cfg(test)]
mod tests {
    use dg_core::{Actor, Capability, ProgramId, Role, UserId};

    /// The rule every route in this module applies once the owning program
    /// is known — `get_course`, `list_courses_for_program` (against the path
    /// `program_id`), and the create/list/update siblings alike. That they
    /// are all the same check is the property this matrix pins.
    fn authorize(actor: &Actor, program_id: ProgramId) -> Result<(), dg_core::PublicError> {
        actor.require_scoped(Capability::ManagePrograms, program_id)
    }

    #[test]
    fn super_admin_may_read_any_course() {
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert!(authorize(&actor, ProgramId::new()).is_ok());
    }

    #[test]
    fn sub_admin_may_read_a_course_in_its_scope() {
        let scoped = ProgramId::new();
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![scoped]);
        assert!(authorize(&actor, scoped).is_ok());
    }

    #[test]
    fn sub_admin_is_forbidden_a_course_outside_its_scope() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        let err = authorize(&actor, ProgramId::new()).expect_err("out of scope");
        assert_eq!(err.code(), "FORBIDDEN");
    }

    #[test]
    fn student_is_forbidden_every_course() {
        let program_id = ProgramId::new();
        let actor = Actor::new(UserId::new(), Role::Student, vec![program_id]);
        let err = authorize(&actor, program_id).expect_err("students never read admin rows");
        assert_eq!(err.code(), "FORBIDDEN");
    }
}
