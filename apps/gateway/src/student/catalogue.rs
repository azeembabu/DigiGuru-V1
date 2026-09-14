//! `GET /api/v1/student/courses`, `.../courses/{id}/blocks`, `.../blocks/{id}/units`
//!
//! The student's own syllabus, in the order they walk it: course -> block ->
//! unit -> classroom. Before these, the classroom opened straight onto
//! `students.current_block_id` and there was no way to reach anything else —
//! a student could study one block and could not see the rest of their course.
//!
//! # Self-only, and scoped by enrolment rather than filtered by it
//!
//! No route takes a `student_id`; the subject comes from the access token.
//! Every query in `dg_db::models::student_catalogue` *starts* from
//! `student_courses` with that id bound, so a course, block or unit the student
//! is not enrolled in selects no row at all. A `course_id` or `block_id`
//! belonging to someone else's programme is therefore indistinguishable from
//! one that does not exist, and both answer `404` — never `403`, which would
//! confirm the id is real.
//!
//! # Not paginated
//!
//! A student has a handful of courses, a course has a handful of blocks, and a
//! block has a handful of units. These are navigation screens rendered whole,
//! not feeds; `limit`/`offset` here would be ceremony with no case that needs
//! it. The admin lists over the same tables stay paginated, because an admin
//! sees every student's worth of them.

use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

use dg_core::{BlockId, Capability, CourseId, DocumentId, PublicError};
use dg_db::models::student_catalogue;

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct StudentCourseResponse {
    pub course_id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub semester_number: i16,
    pub semester_name: String,
    pub block_count: i64,
    /// Blocks with at least one embedded unit. A course whose material has not
    /// been ingested yet reports `0` here and is still listed — the student is
    /// enrolled in it, and hiding it would read as a missing course.
    pub teachable_block_count: i64,
}

#[derive(Debug, Serialize)]
pub struct StudentBlockResponse {
    pub block_id: Uuid,
    pub block_no: i16,
    pub title: String,
    pub description: Option<String>,
    pub unit_count: i64,
    /// Units that are embedded. `0` means a session opened here would abstain
    /// on every question (NN-4 working correctly), so the client says so up
    /// front rather than letting the student find out mid-lesson.
    pub ready_unit_count: i64,
}

#[derive(Debug, Serialize)]
pub struct StudentUnitResponse {
    pub document_id: Uuid,
    pub title: String,
    pub page_count: i32,
    pub is_ready: bool,
    /// Ingestion stage — `pending|parsing|pending_review|embedded|failed`.
    /// Drives the progress indicator; `is_ready` stays the single field a
    /// client checks to decide whether the classroom may be opened.
    pub status: String,
    /// Whether a cover image exists at
    /// `GET /student/units/{document_id}/thumbnail`.
    ///
    /// A flag, not a URL: the path is derivable from `document_id`, and the
    /// server-side file path behind it never goes on the wire (the same rule
    /// that keeps `storage_key` off every document response). Its purpose is
    /// to stop the client requesting a cover that was never rendered, which
    /// would be one 404 per unit on every visit to the screen.
    pub has_thumbnail: bool,
}

pub async fn list_courses(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
) -> Result<Json<Vec<StudentCourseResponse>>, PublicError> {
    actor.require(Capability::ViewOwnContext)?;
    let student = super::own_student(&state, &actor).await?;

    let rows = student_catalogue::courses_for_student(&state.pool, student.id)
        .await
        .map_err(PublicError::from)?;

    Ok(Json(
        rows.into_iter()
            .map(|row| StudentCourseResponse {
                course_id: row.course_id.into_uuid(),
                code: row.code,
                name: row.name,
                description: row.description,
                semester_number: row.semester_number,
                semester_name: row.semester_name,
                block_count: row.block_count,
                teachable_block_count: row.teachable_block_count,
            })
            .collect(),
    ))
}

pub async fn list_blocks(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(course_id): Path<Uuid>,
) -> Result<Json<Vec<StudentBlockResponse>>, PublicError> {
    actor.require(Capability::ViewOwnContext)?;
    let student = super::own_student(&state, &actor).await?;

    let rows =
        student_catalogue::blocks_for_student(&state.pool, student.id, CourseId::from(course_id))
            .await
            .map_err(PublicError::from)?;

    // An empty list is ambiguous on its own — it could mean "not your course"
    // or "no blocks yet" — so a course the student is not enrolled in is
    // resolved to `404` here rather than served as an empty page they would
    // read as a bug.
    if rows.is_empty() && !enrolled_in_course(&state, &student, course_id).await? {
        return Err(PublicError::NotFound);
    }

    Ok(Json(
        rows.into_iter()
            .map(|row| StudentBlockResponse {
                block_id: row.block_id.into_uuid(),
                block_no: row.block_no,
                title: row.title,
                description: row.description,
                unit_count: row.unit_count,
                ready_unit_count: row.ready_unit_count,
            })
            .collect(),
    ))
}

pub async fn list_units(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(block_id): Path<Uuid>,
) -> Result<Json<Vec<StudentUnitResponse>>, PublicError> {
    actor.require(Capability::ViewOwnContext)?;
    let student = super::own_student(&state, &actor).await?;
    let block_id = BlockId::from(block_id);

    // Same reasoning as `list_blocks`: "no units uploaded yet" and "not your
    // block" are different answers and must not both render as an empty page.
    if !student_catalogue::may_study_block(&state.pool, student.id, block_id)
        .await
        .map_err(PublicError::from)?
    {
        return Err(PublicError::NotFound);
    }

    let rows = student_catalogue::units_for_student(&state.pool, student.id, block_id)
        .await
        .map_err(PublicError::from)?;

    Ok(Json(
        rows.into_iter()
            .map(|row| StudentUnitResponse {
                document_id: row.document_id.into_uuid(),
                title: row.title,
                page_count: row.page_count,
                is_ready: row.is_ready,
                status: row.status,
                has_thumbnail: row.has_thumbnail,
            })
            .collect(),
    ))
}

/// `GET /api/v1/student/units/{document_id}/thumbnail` — a unit's cover image.
///
/// The one route in this module that answers bytes rather than JSON. It is
/// still an enrolment-scoped read of the student's own syllabus, and it is
/// scoped the same way everything else here is: the query starts from
/// `student_courses` with the caller's id bound, so a `document_id` outside
/// their enrolments selects nothing and answers `404`, indistinguishable from
/// an id that does not exist.
///
/// # This is a cover, not the unit
///
/// What is served is a small rasterised image of page 1 — never the PDF, and
/// never `storage_key`. A student reads a unit through the tutor; being able
/// to recognise it in a list is a different thing from being handed it.
///
/// A missing cover is `404` and not an error: a document ingested by a build
/// with no pdfium library has none (`rag::thumbnail`). Clients are told which
/// units have one by `has_thumbnail` on the list above and should not be
/// asking, so reaching here without a cover is an unusual path, not a broken
/// one.
pub async fn get_unit_thumbnail(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(document_id): Path<Uuid>,
) -> Result<Response, PublicError> {
    actor.require(Capability::ViewOwnContext)?;
    let student = super::own_student(&state, &actor).await?;

    let key = student_catalogue::thumbnail_key_for_student(
        &state.pool,
        student.id,
        DocumentId::from(document_id),
    )
    .await
    .map_err(PublicError::from)?
    .ok_or(PublicError::NotFound)?;

    let bytes = tokio::fs::read(&key).await.map_err(|err| {
        // The row points at a file that is gone — a half-cleaned upload
        // directory, or a restore that missed it. Log the path internally;
        // the student is told only that there is no cover.
        tracing::warn!(error = %err, key = %key, "unit cover is recorded but unreadable");
        PublicError::NotFound
    })?;

    Ok((
        [
            (header::CONTENT_TYPE, "image/webp"),
            // Private: the response is scoped to one student's enrolments, so
            // a shared cache must never hand it to the next caller. Immutable
            // within that: a cover is re-rendered only by a re-ingestion,
            // which the client reaches through a changed `has_thumbnail`.
            (header::CACHE_CONTROL, "private, max-age=86400"),
        ],
        bytes,
    )
        .into_response())
}

/// Is the student enrolled in this course at all?
///
/// Only reached when the block list came back empty, to tell "your course, no
/// blocks" from "not your course".
async fn enrolled_in_course(
    state: &AppState,
    student: &dg_db::models::students::Student,
    course_id: Uuid,
) -> Result<bool, PublicError> {
    dg_db::models::student_courses::is_enrolled(
        &state.pool,
        student.id,
        CourseId::from(course_id),
    )
    .await
    .map_err(PublicError::from)
}
