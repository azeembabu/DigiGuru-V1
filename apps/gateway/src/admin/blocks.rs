//! `/api/v1/admin/blocks` — CRUD on `blocks`, the teaching unit inside a
//! `course` and the target of `students.current_block_id`.
//!
//! A block has no `program_id` of its own, so scope is resolved the same way
//! `admin/courses.rs` does it, one level deeper: `block -> course ->
//! program_id` for the single-block routes, `course -> program_id` for
//! create and list. As in `admin/documents.rs`, the owning program is
//! resolved *before* the capability check, so a sub-admin outside the
//! program's scope gets the same answer whether or not the row exists.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use dg_core::{BlockId, Capability, CourseId, EntityStatus, PublicError};
use dg_db::models::{blocks, courses};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct BlockResponse {
    pub id: Uuid,
    pub course_id: Uuid,
    /// Flat ancestry, so a console can render a breadcrumb from one
    /// response instead of walking back up the hierarchy a request at a
    /// time. Ids only — no nested objects, matching the rest of the API.
    pub semester_id: Uuid,
    pub program_id: Uuid,
    pub block_no: i16,
    pub title: String,
    pub description: Option<String>,
    pub status: EntityStatus,
}

impl From<blocks::Block> for BlockResponse {
    fn from(b: blocks::Block) -> Self {
        Self {
            id: b.id.into_uuid(),
            course_id: b.course_id.into_uuid(),
            semester_id: b.semester_id.into_uuid(),
            program_id: b.program_id.into_uuid(),
            block_no: b.block_no,
            title: b.title,
            description: b.description,
            status: b.status,
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateBlockRequest {
    pub course_id: Uuid,
    /// Position within the course. `UNIQUE (course_id, block_no)` in the
    /// schema, so a duplicate is a DB-level conflict, not a validation error.
    #[validate(range(min = 1, max = 999))]
    pub block_no: i16,
    #[validate(length(min = 2, max = 200))]
    pub title: String,
    pub description: Option<String>,
}

pub async fn create_block(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Json(payload): Json<CreateBlockRequest>,
) -> Result<Json<BlockResponse>, PublicError> {
    payload
        .validate()
        .map_err(|_| PublicError::validation("title", "invalid block fields"))?;

    let course_id = CourseId::from(payload.course_id);
    let course = courses::find_by_id(&state.pool, course_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManagePrograms, course.program_id)?;

    let block = blocks::create(
        &state.pool,
        course_id,
        payload.block_no,
        &payload.title,
        payload.description.as_deref(),
    )
    .await
    .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.block_created",
        None,
        None,
        Some(serde_json::json!({ "block_id": block.id, "course_id": course_id })),
    )
    .await;

    Ok(Json(block.into()))
}

pub async fn list_blocks_for_course(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(course_id): Path<Uuid>,
) -> Result<Json<Vec<BlockResponse>>, PublicError> {
    let course_id = CourseId::from(course_id);
    let course = courses::find_by_id(&state.pool, course_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManagePrograms, course.program_id)?;

    let rows = blocks::list_by_course(&state.pool, course_id)
        .await
        .map_err(PublicError::from)?;

    Ok(Json(rows.into_iter().map(BlockResponse::from).collect()))
}

pub async fn get_block(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<BlockResponse>, PublicError> {
    let block_id = BlockId::from(id);

    // Owning program resolved before the capability check, as in every
    // other route here — a sub-admin outside the scope must not be able to
    // tell an existing block from a missing one.
    let program_id = blocks::program_id_for_block(&state.pool, block_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManagePrograms, program_id)?;

    let block = blocks::find_by_id(&state.pool, block_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    Ok(Json(block.into()))
}

/// Partial update — every field is optional, and an omitted field is left
/// untouched (`dg_db::models::blocks::update` coalesces). `course_id` and
/// `block_no` are deliberately not editable here: moving a block between
/// courses would move it between programs, past the scope check that was
/// made against the program it started in.
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateBlockRequest {
    #[validate(length(min = 2, max = 200))]
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<EntityStatus>,
}

pub async fn update_block(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateBlockRequest>,
) -> Result<(), PublicError> {
    payload
        .validate()
        .map_err(|_| PublicError::validation("title", "invalid block fields"))?;

    let block_id = BlockId::from(id);
    let program_id = blocks::program_id_for_block(&state.pool, block_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManagePrograms, program_id)?;

    blocks::update(
        &state.pool,
        block_id,
        payload.title.as_deref(),
        payload.description.as_deref(),
        payload.status,
    )
    .await
    .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.block_updated",
        None,
        None,
        Some(serde_json::json!({ "block_id": block_id })),
    )
    .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use dg_core::{Actor, Capability, ProgramId, Role, UserId};

    /// The single authorization rule every route in this module applies,
    /// after resolving the owning program. Asserting it directly is what
    /// makes the role matrix below a test of *this* module rather than of
    /// `dg_core::Actor` in the abstract.
    fn authorize(actor: &Actor, program_id: ProgramId) -> Result<(), dg_core::PublicError> {
        actor.require_scoped(Capability::ManagePrograms, program_id)
    }

    #[test]
    fn super_admin_may_manage_blocks_in_any_program() {
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert!(authorize(&actor, ProgramId::new()).is_ok());
    }

    #[test]
    fn sub_admin_may_manage_blocks_only_in_scoped_programs() {
        let scoped = ProgramId::new();
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![scoped]);
        assert!(authorize(&actor, scoped).is_ok());
    }

    #[test]
    fn sub_admin_is_forbidden_blocks_outside_its_scope() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        let err = authorize(&actor, ProgramId::new()).expect_err("out of scope");
        assert_eq!(err.code(), "FORBIDDEN");
    }

    #[test]
    fn sub_admin_with_no_scopes_is_forbidden_every_block() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![]);
        assert!(authorize(&actor, ProgramId::new()).is_err());
    }

    #[test]
    fn block_response_carries_flat_ancestry_for_breadcrumbs() {
        use dg_core::{BlockId, CourseId, EntityStatus, SemesterId};

        let block = dg_db::models::blocks::Block {
            id: BlockId::new(),
            course_id: CourseId::new(),
            semester_id: SemesterId::new(),
            program_id: ProgramId::new(),
            block_no: 3,
            title: "Unit 3".into(),
            description: None,
            status: EntityStatus::Active,
        };
        let (course, semester, program) = (block.course_id, block.semester_id, block.program_id);

        let response = super::BlockResponse::from(block);

        assert_eq!(response.course_id, course.into_uuid());
        assert_eq!(response.semester_id, semester.into_uuid());
        assert_eq!(response.program_id, program.into_uuid());
    }

    #[test]
    fn student_is_forbidden_every_block_route() {
        let program_id = ProgramId::new();
        // A student holding the program in `scopes` still fails: `in_scope`
        // is false for `Role::Student` regardless of what the vector says.
        let actor = Actor::new(UserId::new(), Role::Student, vec![program_id]);
        let err = authorize(&actor, program_id).expect_err("students never manage blocks");
        assert_eq!(err.code(), "FORBIDDEN");
    }
}
