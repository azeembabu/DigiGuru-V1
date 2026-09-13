//! `DELETE /api/v1/admin/{programs|courses|blocks|documents}/{id}` and the
//! `.../deletion-impact` previews that must be read first.
//!
//! # These routes destroy student data on purpose
//!
//! `blocks -> exams -> exam_attempts -> exam_attempt_answers` cascades the whole
//! way down, so removing a block permanently deletes every student's marks for
//! that block's exams. So does removing the course or programme above it. That
//! is the agreed behaviour — an admin who removes a block means it — but it is
//! not something anyone should discover afterwards, which is what the
//! `deletion-impact` routes are for: the console shows exactly what will be
//! lost, and requires the item's own name to be typed back, before it may call
//! `DELETE`.
//!
//! The impact preview is advisory, not a lock. Nothing stops a caller invoking
//! `DELETE` directly, and no server-side token ties one to the other — a
//! confirmation step that a script can skip is a UI affordance, and pretending
//! otherwise would be security theatre. The real protections are the capability
//! check, the scope check, and the audit row.
//!
//! # Scope
//!
//! Every route resolves the owning `program_id` and calls `require_scoped`
//! before doing any work, exactly as the rest of the admin surface does. A
//! sub-admin outside the programme's scope gets `403`; an id that does not
//! exist at all gets `404`. That is the established behaviour of every other
//! scoped admin route (`GET /admin/courses/{id}` answers the same way), and
//! these routes match it rather than inventing a second convention.
//!
//! It does mean a sub-admin can distinguish an out-of-scope row from a missing
//! one. That is a pre-existing property of the admin surface, not something
//! introduced here, and the student-facing routes — where it would matter —
//! deliberately answer `404` for both.
//!
//! # What removal never touches
//!
//! Student **accounts**. `students.program_id` is `NOT NULL ON DELETE
//! RESTRICT`, and that is kept: removing a programme is a content operation,
//! and deleting the people registered on it is a different decision. A
//! programme with students on it answers `409 PROGRAM_HAS_STUDENTS` and nothing
//! is changed.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use dg_core::{BlockId, Capability, CourseId, DocumentId, ProgramId, PublicError};
use dg_db::models::{blocks, courses, deletion, documents};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

/// `409` code for a programme that still has students registered on it.
const PROGRAM_HAS_STUDENTS: &str = "PROGRAM_HAS_STUDENTS";

/// Everything a confirmation dialogue needs to say what will be lost.
///
/// All counts, no ids: the dialogue reports scale ("12 attempts by 3
/// students"), and a list of every affected student would be both unusable at
/// size and a PII export from a screen that does not need one.
#[derive(Debug, Serialize)]
pub struct DeletionImpactResponse {
    pub semesters: i64,
    pub courses: i64,
    pub blocks: i64,
    pub units: i64,
    pub exams: i64,
    pub exam_attempts: i64,
    pub questions: i64,
    pub flashcards: i64,
    pub learning_sessions: i64,
    pub board_events: i64,
    pub enrollments: i64,
    pub students_affected: i64,
    /// `false` when the delete would be refused — currently only a programme
    /// with students registered. The console disables the confirm button and
    /// explains, instead of offering an action that will fail.
    pub can_delete: bool,
    /// Why not, in a sentence fit to show a human. `null` when `can_delete`.
    pub blocked_reason: Option<String>,
}

impl From<deletion::DeletionImpact> for DeletionImpactResponse {
    fn from(i: deletion::DeletionImpact) -> Self {
        Self {
            semesters: i.semesters,
            courses: i.courses,
            blocks: i.blocks,
            units: i.units,
            exams: i.exams,
            exam_attempts: i.exam_attempts,
            questions: i.questions,
            flashcards: i.flashcards,
            learning_sessions: i.learning_sessions,
            board_events: i.board_events,
            enrollments: i.enrollments,
            students_affected: i.students_affected,
            can_delete: true,
            blocked_reason: None,
        }
    }
}

/// What a completed removal destroyed. The same shape as the preview, so a
/// console can show "this is what went" with the code that showed "this is what
/// will go".
#[derive(Debug, Serialize)]
pub struct DeletedResponse {
    pub deleted: bool,
    pub impact: DeletionImpactResponse,
}

// ------------------------------------------------------------- programmes ---

pub async fn program_impact(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<DeletionImpactResponse>, PublicError> {
    let program_id = ProgramId::from(id);
    actor.require_scoped(Capability::ManagePrograms, program_id)?;
    ensure_program_exists(&state, program_id).await?;

    let impact = deletion::program_impact(&state.pool, program_id)
        .await
        .map_err(PublicError::from)?;
    let students = student_count(&state, program_id).await?;

    let mut response = DeletionImpactResponse::from(impact);
    if students > 0 {
        response.can_delete = false;
        response.blocked_reason = Some(format!(
            "{students} student{} registered on this programme. Move or remove them first — \
             removing a programme does not delete student accounts.",
            if students == 1 { " is" } else { "s are" }
        ));
    }
    Ok(Json(response))
}

pub async fn delete_program(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<DeletedResponse>, PublicError> {
    let program_id = ProgramId::from(id);
    actor.require_scoped(Capability::ManagePrograms, program_id)?;
    ensure_program_exists(&state, program_id).await?;

    let outcome = deletion::delete_program(&state.pool, program_id)
        .await
        .map_err(PublicError::from)?;

    match outcome {
        Err(deletion::DeleteBlocked::StudentsRegistered(n)) => Err(PublicError::Conflict {
            code: PROGRAM_HAS_STUDENTS,
            message: format!(
                "{n} student{} registered on this programme. Move or remove them first — \
                 removing a programme does not delete student accounts.",
                if n == 1 { " is" } else { "s are" }
            ),
        }),
        Ok(None) => Err(PublicError::NotFound),
        Ok(Some(deleted)) => finish(&state, &actor, "admin.program.delete", id, deleted).await,
    }
}

// ----------------------------------------------------------------- courses --

pub async fn course_impact(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<DeletionImpactResponse>, PublicError> {
    let course_id = CourseId::from(id);
    scope_course(&state, &actor, course_id).await?;
    let impact = deletion::course_impact(&state.pool, course_id)
        .await
        .map_err(PublicError::from)?;
    Ok(Json(DeletionImpactResponse::from(impact)))
}

pub async fn delete_course(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<DeletedResponse>, PublicError> {
    let course_id = CourseId::from(id);
    scope_course(&state, &actor, course_id).await?;

    match deletion::delete_course(&state.pool, course_id)
        .await
        .map_err(PublicError::from)?
    {
        None => Err(PublicError::NotFound),
        Some(deleted) => finish(&state, &actor, "admin.course.delete", id, deleted).await,
    }
}

// ------------------------------------------------------------------ blocks --

pub async fn block_impact(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<DeletionImpactResponse>, PublicError> {
    let block_id = BlockId::from(id);
    scope_block(&state, &actor, block_id).await?;
    let impact = deletion::block_impact(&state.pool, block_id)
        .await
        .map_err(PublicError::from)?;
    Ok(Json(DeletionImpactResponse::from(impact)))
}

pub async fn delete_block(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<DeletedResponse>, PublicError> {
    let block_id = BlockId::from(id);
    scope_block(&state, &actor, block_id).await?;

    match deletion::delete_block(&state.pool, block_id)
        .await
        .map_err(PublicError::from)?
    {
        None => Err(PublicError::NotFound),
        Some(deleted) => finish(&state, &actor, "admin.block.delete", id, deleted).await,
    }
}

// ------------------------------------------------------------------- units --

pub async fn unit_impact(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<DeletionImpactResponse>, PublicError> {
    let document_id = DocumentId::from(id);
    scope_unit(&state, &actor, document_id).await?;
    let impact = deletion::unit_impact(&state.pool, document_id)
        .await
        .map_err(PublicError::from)?;
    Ok(Json(DeletionImpactResponse::from(impact)))
}

pub async fn delete_unit(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<DeletedResponse>, PublicError> {
    let document_id = DocumentId::from(id);
    scope_unit(&state, &actor, document_id).await?;

    match deletion::delete_unit(&state.pool, document_id)
        .await
        .map_err(PublicError::from)?
    {
        None => Err(PublicError::NotFound),
        Some(deleted) => finish(&state, &actor, "admin.unit.delete", id, deleted).await,
    }
}

// ----------------------------------------------------------------- shared ---

/// Purges the removed documents' vectors, writes the audit row, and answers.
///
/// Qdrant is cleaned **after** the Postgres transaction has committed, not
/// before: a purge followed by a rollback would leave the tutor holding a
/// document row it can no longer retrieve anything for, which reads as an empty
/// syllabus rather than a deleted one. This way round the worst case is orphan
/// vectors, which are filtered out by `block_no` on every query anyway
/// (`rag-pipeline.md`) and are reported here rather than failing the request —
/// the content is gone from the student's view either way.
async fn finish(
    state: &AppState,
    actor: &dg_core::Actor,
    action: &str,
    id: Uuid,
    deleted: deletion::Deleted,
) -> Result<Json<DeletedResponse>, PublicError> {
    if !deleted.document_ids.is_empty() {
        // Built here rather than held in `AppState`, matching `ws::qdrant_client`:
        // an unreachable Qdrant must not be a boot failure for an admin console
        // that mostly does not need it.
        let url = rag::qdrant::grpc_url(&state.config.qdrant_url);
        match qdrant_client::Qdrant::from_url(&url).build() {
            Ok(qdrant) => {
                for document_id in &deleted.document_ids {
                    if let Err(err) =
                        rag::qdrant::delete_document_points(&qdrant, *document_id).await
                    {
                        tracing::error!(
                            %err,
                            %document_id,
                            "content was deleted but its vectors could not be purged from Qdrant"
                        );
                    }
                }
            }
            Err(err) => tracing::error!(
                %err,
                "content was deleted but no Qdrant client could be built to purge its vectors"
            ),
        }
    }

    crate::audit::log(
        state,
        Some(actor.user_id),
        action,
        None,
        None,
        Some(serde_json::json!({
            "id": id,
            "blocks": deleted.impact.blocks,
            "units": deleted.impact.units,
            "exams": deleted.impact.exams,
            "exam_attempts": deleted.impact.exam_attempts,
            "learning_sessions": deleted.impact.learning_sessions,
            "students_affected": deleted.impact.students_affected,
        })),
    )
    .await;

    Ok(Json(DeletedResponse {
        deleted: true,
        impact: DeletionImpactResponse::from(deleted.impact),
    }))
}

async fn ensure_program_exists(state: &AppState, id: ProgramId) -> Result<(), PublicError> {
    dg_db::models::programs::find_by_id(&state.pool, id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;
    Ok(())
}

async fn student_count(state: &AppState, id: ProgramId) -> Result<i64, PublicError> {
    dg_db::models::deletion::program_student_count(&state.pool, id)
        .await
        .map_err(PublicError::from)
}

/// `course -> program_id`, checked before existence is admitted.
async fn scope_course(
    state: &AppState,
    actor: &dg_core::Actor,
    course_id: CourseId,
) -> Result<(), PublicError> {
    let course = courses::find_by_id(&state.pool, course_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;
    actor.require_scoped(Capability::ManagePrograms, course.program_id)?;
    Ok(())
}

/// `block -> course -> program_id`.
async fn scope_block(
    state: &AppState,
    actor: &dg_core::Actor,
    block_id: BlockId,
) -> Result<(), PublicError> {
    let block = blocks::find_by_id(&state.pool, block_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;
    actor.require_scoped(Capability::ManagePrograms, block.program_id)?;
    Ok(())
}

/// `document -> block -> course -> program_id`.
async fn scope_unit(
    state: &AppState,
    actor: &dg_core::Actor,
    document_id: DocumentId,
) -> Result<(), PublicError> {
    let document = documents::find_by_id(&state.pool, document_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;
    scope_block(state, actor, document.block_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use dg_core::{Actor, Role, UserId};

    /// Every count the preview promises must survive the mapping. A field that
    /// silently stayed 0 would under-report what a removal destroys, which is
    /// the one failure mode of this screen that matters.
    #[test]
    fn the_impact_response_carries_every_count() {
        let impact = deletion::DeletionImpact {
            semesters: 1,
            courses: 2,
            blocks: 3,
            units: 4,
            exams: 5,
            exam_attempts: 6,
            questions: 7,
            flashcards: 8,
            learning_sessions: 9,
            board_events: 10,
            enrollments: 11,
            students_affected: 12,
        };
        let json = serde_json::to_value(DeletionImpactResponse::from(impact)).expect("serialises");
        for (field, expected) in [
            ("semesters", 1),
            ("courses", 2),
            ("blocks", 3),
            ("units", 4),
            ("exams", 5),
            ("exam_attempts", 6),
            ("questions", 7),
            ("flashcards", 8),
            ("learning_sessions", 9),
            ("board_events", 10),
            ("enrollments", 11),
            ("students_affected", 12),
        ] {
            assert_eq!(json[field], expected, "{field} was not carried through");
        }
        // Nothing is blocked unless a handler says so.
        assert_eq!(json["can_delete"], true);
        assert!(json["blocked_reason"].is_null());
    }

    /// A student cannot reach these routes at all: removal is an admin
    /// capability, and the check is the extractor's, not a role test inside the
    /// handler (`security.md` — no implicit allow).
    #[test]
    fn a_student_may_not_remove_anything() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        let err = actor
            .require_scoped(Capability::ManagePrograms, ProgramId::new())
            .expect_err("a student manages no programmes");
        assert_eq!(err.code(), "FORBIDDEN");
    }

    /// A sub-admin may remove inside its own programmes and nowhere else. The
    /// out-of-scope answer is `403`, matching every other scoped admin route
    /// (see the module docs).
    #[test]
    fn a_sub_admin_is_confined_to_its_scoped_programmes() {
        let mine = ProgramId::new();
        let theirs = ProgramId::new();
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![mine]);

        assert!(actor.require_scoped(Capability::ManagePrograms, mine).is_ok());
        assert_eq!(
            actor
                .require_scoped(Capability::ManagePrograms, theirs)
                .expect_err("out of scope")
                .code(),
            "FORBIDDEN"
        );
    }

    /// The refusal code is part of the contract: a console matches on it to
    /// explain that student accounts are not deleted by removing a programme.
    #[test]
    fn the_students_registered_refusal_has_a_stable_code() {
        assert_eq!(PROGRAM_HAS_STUDENTS, "PROGRAM_HAS_STUDENTS");
    }
}
