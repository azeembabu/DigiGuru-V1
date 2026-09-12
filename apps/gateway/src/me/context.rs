//! `GET /api/v1/me/context` — the "Remember Me" payload
//! (`IMPLEMENTATION_PLAN.md` §4.1 item 5, §3.3 `ctx:{student_id}`).
//!
//! This scaffold reads straight from Postgres. The Redis `ctx:{student_id}`
//! cache described in the plan is a Phase 3 performance optimisation (the
//! classroom session-init path hydrates it) — reading through to Postgres
//! here is correct for Phase 1 and gives the cache something authoritative
//! to be populated from later.

use axum::extract::State;
use axum::Json;

use dg_core::{
    context::{BlockSummary, ProgramSummary, SemesterSummary, StudentContext},
    Capability, PublicError,
};
use dg_db::models::{blocks, programs, semesters, students};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

pub async fn get_context(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
) -> Result<Json<StudentContext>, PublicError> {
    actor.require(Capability::ViewOwnContext)?;

    let student = students::find_by_user_id(&state.pool, actor.user_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    let program = programs::find_by_id(&state.pool, student.program_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::Internal)?;

    let semester = semesters::find_by_id(&state.pool, student.semester_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::Internal)?;

    let current_block = match student.current_block_id {
        Some(block_id) => blocks::find_by_id(&state.pool, block_id)
            .await
            .map_err(PublicError::from)?
            .map(|b| BlockSummary {
                id: b.id,
                course_id: b.course_id,
                block_no: b.block_no,
                title: b.title,
            }),
        None => None,
    };

    Ok(Json(StudentContext {
        program: ProgramSummary { id: program.id, code: program.code, name: program.name },
        semester: SemesterSummary {
            id: semester.id,
            semester_number: semester.semester_number,
            name: semester.name,
        },
        current_block,
        is_first_login: student.is_first_login,
    }))
}
