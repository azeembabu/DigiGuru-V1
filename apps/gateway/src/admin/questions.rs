//! `/api/v1/admin/question-pool` — authoring the MCQ bank the exam module
//! samples from.
//!
//! A pool belongs to a **course**: the owner's taxonomy is
//! Program > Semester > Course > Unit/Module, and the pool is pre-populated per
//! course with the unit/module link explicitly optional. So `course_id` is
//! required on every write, `block_id` is the optional module pointer, and
//! admin scope is resolved `question -> course -> program_id` — one hop shorter
//! than it was when a question hung off a block. As in `admin/exams.rs`, the
//! owning program is resolved **before** the capability check, so a sub-admin
//! outside the scope cannot tell an existing question from a missing one.
//!
//! When `block_id` is present it must belong to the given `course_id`. That is
//! checked here rather than by a CHECK constraint, because the constraint would
//! need a subquery (`blocks.course_id`) and cannot have one. Doing it in the
//! handler also means the bulk path can report the offending **row number**,
//! which a trigger could not.
//!
//! Admin responses *do* carry `correct_option_index` and `explanation`: an
//! author has to see the key they are authoring. The student-facing paper is a
//! different type in a different module that has no field for either
//! (`student/exam_module.rs`).
//!
//! Parsing for the four bulk formats, and the field validation all five write
//! paths share, live in `question_import.rs`.

use axum::{
    extract::{FromRequest, Path, Query, Request, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::{
    AssessmentType, BlockId, Capability, CourseId, DifficultyLevel, ProgramId, PublicError,
    QuestionId, QuestionStatus, Role, SemesterId,
};
use dg_db::models::{blocks, courses, question_pool};

use super::question_import::{
    self as import, option_label, validate_fields, QuestionFields, OPTION_COUNT,
};
use crate::extractors::{AuthenticatedActor, JsonBody};
use crate::state::AppState;

pub(crate) use super::question_import::MAX_BULK_IMPORT_BYTES;

// ---------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------

/// A question plus the flat ancestry of its course, so the console renders a
/// breadcrumb from one response — the same choice `ExamResponse` makes.
#[derive(Debug, Serialize)]
pub struct QuestionResponse {
    pub id: Uuid,
    pub course_id: Uuid,
    pub course_code: String,
    pub semester_id: Uuid,
    pub program_id: Uuid,
    /// The optional unit/module. `null` is the normal shape of a course-wide
    /// question, not a missing value — and the three module fields are `null`
    /// together.
    pub block_id: Option<Uuid>,
    pub block_no: Option<i16>,
    pub block_title: Option<String>,
    pub topic: String,
    pub question_text: String,
    /// Exactly four, in the A/B/C/D order a student answers against.
    pub options: Vec<String>,
    /// `0..=3`. Admin-only: the student-facing paper type has no such field.
    pub correct_option_index: i16,
    /// Mandatory — it drives the post-exam feedback screen, so a question
    /// without one could not be remediated.
    pub explanation: String,
    pub assessment_type: AssessmentType,
    pub difficulty_level: DifficultyLevel,
    pub status: QuestionStatus,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

impl From<question_pool::Question> for QuestionResponse {
    fn from(q: question_pool::Question) -> Self {
        Self {
            id: q.id.into_uuid(),
            course_id: q.course_id.into_uuid(),
            course_code: q.course_code,
            semester_id: q.semester_id.into_uuid(),
            program_id: q.program_id.into_uuid(),
            block_id: q.block_id.map(|b| b.into_uuid()),
            block_no: q.block_no,
            block_title: q.block_title,
            topic: q.topic,
            question_text: q.question_text,
            options: q.options,
            correct_option_index: q.correct_option_index,
            explanation: q.explanation,
            assessment_type: q.assessment_type,
            difficulty_level: q.difficulty_level,
            status: q.status,
            created_by: q.created_by.into_uuid(),
            created_at: q.created_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Authorization and the block/course consistency rule
// ---------------------------------------------------------------------------

/// Resolve the program owning `course_id`, then authorize against it.
///
/// Order matters and is the point: the program is resolved first, so the
/// capability check answers `403` only for a course that exists, and a course
/// outside a sub-admin's scope is rejected before anything is read or written.
async fn authorize_course(
    state: &AppState,
    actor: &dg_core::Actor,
    course_id: CourseId,
) -> Result<ProgramId, PublicError> {
    let course = courses::find_by_id(&state.pool, course_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManagePrograms, course.program_id)?;
    Ok(course.program_id)
}

/// A `block_id` that does not belong to `course_id` is a `400` on `block_id`.
///
/// The schema cannot express this (a CHECK may not contain the `blocks`
/// subquery), so it is enforced on every write path. Left unchecked, a question
/// could be filed under a module of *another* course — and since the pool is
/// sampled by course, it would then appear in a paper whose module breadcrumb
/// points somewhere else entirely.
async fn verify_block_in_course(
    state: &AppState,
    course_id: CourseId,
    block_id: BlockId,
) -> Result<(), PublicError> {
    let block = blocks::find_by_id(&state.pool, block_id)
        .await
        .map_err(PublicError::from)?
        .ok_or_else(|| PublicError::validation("block_id", "that unit/module does not exist"))?;

    if block.course_id != course_id {
        return Err(PublicError::validation(
            "block_id",
            "that unit/module belongs to a different course; leave block_id out for a \
             course-wide question",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// POST /admin/question-pool
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateQuestionRequest {
    /// The pool this question joins. Required: a pool is per course.
    pub course_id: Uuid,
    /// The optional unit/module. Omit it for a course-wide question.
    #[serde(default)]
    pub block_id: Option<Uuid>,
    pub topic: String,
    pub question_text: String,
    pub options: Vec<String>,
    pub correct_option_index: i16,
    pub explanation: String,
    pub assessment_type: AssessmentType,
    /// Omitted means `beginner`, matching the column default and the tutor's
    /// tier 0 (`.claude/rules/pedagogy.md`).
    #[serde(default)]
    pub difficulty_level: Option<DifficultyLevel>,
}

impl From<CreateQuestionRequest> for QuestionFields {
    fn from(r: CreateQuestionRequest) -> Self {
        Self {
            course_id: r.course_id,
            block_id: r.block_id,
            topic: r.topic,
            question_text: r.question_text,
            options: r.options,
            correct_option_index: r.correct_option_index,
            explanation: r.explanation,
            assessment_type: r.assessment_type,
            difficulty_level: r.difficulty_level.unwrap_or(DifficultyLevel::Beginner),
        }
    }
}

pub async fn create_question(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    JsonBody(payload): JsonBody<CreateQuestionRequest>,
) -> Result<Json<QuestionResponse>, PublicError> {
    let fields = QuestionFields::from(payload);
    let course_id = CourseId::from(fields.course_id);

    // Scope before anything else is read or written.
    authorize_course(&state, &actor, course_id).await?;

    validate_fields(&fields).map_err(|m| PublicError::validation("question", m))?;

    let block_id = fields.block_id.map(BlockId::from);
    if let Some(block_id) = block_id {
        verify_block_in_course(&state, course_id, block_id).await?;
    }

    let question = question_pool::create(
        &state.pool,
        course_id,
        block_id,
        fields.topic.trim(),
        fields.question_text.trim(),
        &fields.options,
        fields.correct_option_index,
        fields.explanation.trim(),
        fields.assessment_type,
        fields.difficulty_level,
        actor.user_id,
    )
    .await
    .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.question_created",
        None,
        None,
        Some(serde_json::json!({ "question_id": question.id, "course_id": course_id })),
    )
    .await;

    Ok(Json(question.into()))
}

// ---------------------------------------------------------------------------
// POST /admin/question-pool/bulk
// ---------------------------------------------------------------------------

/// The raw import body, with the bulk route's own cap reported inside the error
/// envelope.
///
/// Mirrors `extractors::UploadBody` and reuses its rejection mapping, but names
/// [`MAX_BULK_IMPORT_BYTES`] rather than the 64 MiB PDF cap — a 413 that quotes
/// the wrong limit tells the admin to cut the wrong amount.
pub struct BulkImportBody(pub axum::body::Bytes);

impl FromRequest<AppState> for BulkImportBody {
    type Rejection = PublicError;

    async fn from_request(req: Request, state: &AppState) -> Result<Self, Self::Rejection> {
        match axum::body::Bytes::from_request(req, state).await {
            Ok(bytes) => Ok(BulkImportBody(bytes)),
            Err(rejection) => Err(crate::extractors::upload_body::map_rejection(
                rejection.into_response().status(),
                MAX_BULK_IMPORT_BYTES,
            )),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BulkImportResponse {
    /// Rows written. Equal to the rows submitted — the import is atomic, so a
    /// partial number is not a state this can return.
    pub imported: i64,
    /// Which channel the body was sniffed as, echoed so an author who meant to
    /// send a spreadsheet and sent something else can see what happened.
    pub format: &'static str,
}

pub async fn bulk_import_questions(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    BulkImportBody(body): BulkImportBody,
) -> Result<Json<BulkImportResponse>, PublicError> {
    // A capability check before the body is parsed at all: an actor that may not
    // author questions should not get parse feedback on its payload.
    actor.require(Capability::ManagePrograms)?;

    let (format, rows) = import::parse_import(&body)?;

    // Authorize every distinct course, and check every module link, *before*
    // writing anything. A sub-admin slipping one out-of-scope course into a
    // 500-row import must have the whole import rejected, not 499 questions
    // written — and a module belonging to another course must fail the same way,
    // naming its row.
    let mut checked_courses: Vec<Uuid> = Vec::new();
    let mut checked_blocks: Vec<(Uuid, Uuid)> = Vec::new();
    let mut row_problems: Vec<String> = Vec::new();

    for (index, fields) in rows.iter().enumerate() {
        let row = index + 1;
        let course_id = CourseId::from(fields.course_id);

        if !checked_courses.contains(&fields.course_id) {
            // A scope or existence failure aborts the import immediately rather
            // than being collected: it is an authorization answer, not a row
            // the author can fix by editing a cell.
            authorize_course(&state, &actor, course_id).await?;
            checked_courses.push(fields.course_id);
        }

        if let Some(block_uuid) = fields.block_id {
            let pair = (fields.course_id, block_uuid);
            if !checked_blocks.contains(&pair) {
                if let Err(err) =
                    verify_block_in_course(&state, course_id, BlockId::from(block_uuid)).await
                {
                    row_problems.push(format!("row {row}: {}", err.public_message()));
                    continue;
                }
                checked_blocks.push(pair);
            }
        }
    }

    if !row_problems.is_empty() {
        return Err(import::row_errors(row_problems));
    }

    // One statement, so the import is all-or-nothing at the database level too
    // and not merely in this handler's control flow.
    let course_ids: Vec<Uuid> = rows.iter().map(|r| r.course_id).collect();
    let block_ids: Vec<Option<Uuid>> = rows.iter().map(|r| r.block_id).collect();
    let topics: Vec<String> = rows.iter().map(|r| r.topic.trim().to_string()).collect();
    let texts: Vec<String> = rows
        .iter()
        .map(|r| r.question_text.trim().to_string())
        .collect();
    let options: Vec<serde_json::Value> =
        rows.iter().map(|r| serde_json::json!(r.options)).collect();
    let indices: Vec<i16> = rows.iter().map(|r| r.correct_option_index).collect();
    let explanations: Vec<String> = rows
        .iter()
        .map(|r| r.explanation.trim().to_string())
        .collect();
    let assessment_types: Vec<String> = rows
        .iter()
        .map(|r| r.assessment_type.as_db_str().to_string())
        .collect();
    let difficulties: Vec<String> = rows
        .iter()
        .map(|r| r.difficulty_level.as_db_str().to_string())
        .collect();

    let imported = question_pool::create_bulk(
        &state.pool,
        &course_ids,
        &block_ids,
        &topics,
        &texts,
        &options,
        &indices,
        &explanations,
        &assessment_types,
        &difficulties,
        actor.user_id,
    )
    .await
    .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.question_pool_bulk_import",
        None,
        None,
        Some(serde_json::json!({ "imported": imported, "format": format.as_str() })),
    )
    .await;

    Ok(Json(BulkImportResponse {
        imported,
        format: format.as_str(),
    }))
}

// ---------------------------------------------------------------------------
// GET /admin/question-pool
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListQuestionsQuery {
    #[serde(default)]
    pub program_id: Option<Uuid>,
    #[serde(default)]
    pub semester_id: Option<Uuid>,
    #[serde(default)]
    pub course_id: Option<Uuid>,
    /// Filtering on the module returns only the questions filed under it, never
    /// the course-wide ones.
    #[serde(default)]
    pub block_id: Option<Uuid>,
    #[serde(default)]
    pub assessment_type: Option<AssessmentType>,
    #[serde(default)]
    pub difficulty_level: Option<DifficultyLevel>,
    #[serde(default)]
    pub status: Option<QuestionStatus>,
    /// Case-insensitive substring over `topic` and `question_text`.
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

/// The sub-admin scope to apply to a list query, or `None` for the unscoped one.
///
/// The caller's role decides *which query runs*, as in `analytics.rs` and
/// `sessions.rs` — never a filter over a platform-wide result, so
/// `X-Total-Count` and the page always agree. An empty scope list yields an
/// empty page, which is the correct answer for a sub-admin with no scopes, not a
/// reason to fall back to the unscoped query.
fn list_scope(actor: &dg_core::Actor) -> Result<Option<Vec<Uuid>>, PublicError> {
    match actor.role {
        Role::SuperAdmin => Ok(None),
        Role::SubAdmin => Ok(Some(actor.scopes.iter().map(|p| p.into_uuid()).collect())),
        // Unreachable via the capability check; matched explicitly so a future
        // capability change cannot silently hand a student the unscoped query.
        Role::Student => Err(PublicError::Forbidden),
    }
}

pub async fn list_questions(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<ListQuestionsQuery>,
) -> Result<(HeaderMap, Json<Vec<QuestionResponse>>), PublicError> {
    actor.require(Capability::ManagePrograms)?;

    let page = super::page(query.limit, query.offset, 50)?;
    let scope = list_scope(&actor)?;

    let filter = question_pool::QuestionFilter {
        program_id: query.program_id.map(ProgramId::from),
        semester_id: query.semester_id.map(SemesterId::from),
        course_id: query.course_id.map(CourseId::from),
        block_id: query.block_id.map(BlockId::from),
        assessment_type: query.assessment_type,
        difficulty_level: query.difficulty_level,
        status: query.status,
        q: super::search_term(query.q.as_deref()),
        scope_program_ids: scope.as_deref(),
    };

    let total = question_pool::count(&state.pool, &filter)
        .await
        .map_err(PublicError::from)?;
    let rows = question_pool::list(&state.pool, &filter, page.limit, page.offset)
        .await
        .map_err(PublicError::from)?;

    Ok((
        super::total_count(total),
        Json(rows.into_iter().map(QuestionResponse::from).collect()),
    ))
}

// ---------------------------------------------------------------------------
// GET /admin/programs/{program_id}/question-pool-counts
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct CoursePoolCountResponse {
    pub course_id: Uuid,
    pub course_code: String,
    pub course_name: String,
    pub semester_id: Uuid,
    pub semester_number: i16,
    pub active_questions: i64,
}

impl From<question_pool::CoursePoolCount> for CoursePoolCountResponse {
    fn from(c: question_pool::CoursePoolCount) -> Self {
        Self {
            course_id: c.course_id.into_uuid(),
            course_code: c.course_code,
            course_name: c.course_name,
            semester_id: c.semester_id.into_uuid(),
            semester_number: c.semester_number,
            active_questions: c.active_questions,
        }
    }
}

/// Every course of one program with its active question count, **including the
/// empty ones** — a console navigating Program -> Semester -> Course needs to
/// see which pools are still empty, which is precisely what a `GROUP BY` over
/// the pool would hide.
///
/// Not paginated: a program has tens of courses, not thousands, and the screen
/// is a single tree.
pub async fn list_pool_counts(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(program_id): Path<Uuid>,
) -> Result<Json<Vec<CoursePoolCountResponse>>, PublicError> {
    let program_id = ProgramId::from(program_id);
    actor.require_scoped(Capability::ManagePrograms, program_id)?;

    let rows = question_pool::pool_counts_by_course(&state.pool, program_id)
        .await
        .map_err(PublicError::from)?;

    Ok(Json(
        rows.into_iter()
            .map(CoursePoolCountResponse::from)
            .collect(),
    ))
}

// ---------------------------------------------------------------------------
// PATCH /admin/question-pool/{id}
// ---------------------------------------------------------------------------

/// Partial edit. Omitted fields are untouched.
///
/// `course_id` is deliberately absent: moving a question to another course would
/// move it past the program scope its author was authorized under, and would
/// move it into a different paper's pool. `block_id` *is* editable — it is the
/// optional module pointer within the question's own course — and is validated
/// against that course. Retiring is `status: "retired"`, never a delete:
/// attempts that already asked the question keep referencing the row so a graded
/// paper stays reviewable.
#[derive(Debug, Deserialize)]
pub struct UpdateQuestionRequest {
    #[serde(default)]
    pub block_id: Option<Uuid>,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub question_text: Option<String>,
    #[serde(default)]
    pub options: Option<Vec<String>>,
    #[serde(default)]
    pub correct_option_index: Option<i16>,
    #[serde(default)]
    pub explanation: Option<String>,
    #[serde(default)]
    pub assessment_type: Option<AssessmentType>,
    #[serde(default)]
    pub difficulty_level: Option<DifficultyLevel>,
    #[serde(default)]
    pub status: Option<QuestionStatus>,
}

impl UpdateQuestionRequest {
    /// Reject the edits that are invalid on their own terms before the update
    /// runs.
    ///
    /// `correct_option_index` is checked against A-D whether or not new options
    /// are supplied, since there are always exactly four — a key of 4 would
    /// otherwise only fail as a schema CHECK, which reaches the client as an
    /// opaque conflict.
    fn validate(&self) -> Result<(), PublicError> {
        let field = |name: &'static str, message: &str| PublicError::validation(name, message);

        if let Some(topic) = &self.topic {
            if topic.trim().is_empty() {
                return Err(field("topic", "topic cannot be blank"));
            }
        }
        if let Some(text) = &self.question_text {
            if text.trim().is_empty() {
                return Err(field("question_text", "question_text cannot be blank"));
            }
        }
        if let Some(explanation) = &self.explanation {
            if explanation.trim().is_empty() {
                return Err(field("explanation", "explanation cannot be blank"));
            }
        }
        if let Some(options) = &self.options {
            if options.len() != OPTION_COUNT {
                return Err(field(
                    "options",
                    &format!("exactly {OPTION_COUNT} options are required"),
                ));
            }
            if let Some(position) = options.iter().position(|o| o.trim().is_empty()) {
                return Err(field(
                    "options",
                    &format!("option {} is blank", option_label(position)),
                ));
            }
        }
        if let Some(index) = self.correct_option_index {
            if !(0..OPTION_COUNT as i16).contains(&index) {
                return Err(field(
                    "correct_option_index",
                    &format!(
                        "correct_option_index must be between 0 and {}",
                        OPTION_COUNT - 1
                    ),
                ));
            }
        }
        Ok(())
    }
}

pub async fn update_question(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
    JsonBody(payload): JsonBody<UpdateQuestionRequest>,
) -> Result<Json<QuestionResponse>, PublicError> {
    let question_id = QuestionId::from(id);

    // Program first, capability second: a sub-admin outside the scope cannot
    // distinguish an existing question from a missing one.
    let program_id = question_pool::program_id_for_question(&state.pool, question_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::ManagePrograms, program_id)?;

    payload.validate()?;

    // A new module link is validated against the question's **own** course,
    // read back from the row rather than taken from the request: the request
    // cannot name a course, so there is nothing here to trust.
    if let Some(block_uuid) = payload.block_id {
        let existing = question_pool::find_by_id(&state.pool, question_id)
            .await
            .map_err(PublicError::from)?
            .ok_or(PublicError::NotFound)?;
        verify_block_in_course(&state, existing.course_id, BlockId::from(block_uuid)).await?;
    }

    let options = payload.options.as_ref().map(|o| serde_json::json!(o));

    let question = question_pool::update(
        &state.pool,
        question_id,
        payload.block_id.map(BlockId::from),
        payload.topic.as_deref().map(str::trim),
        payload.question_text.as_deref().map(str::trim),
        options.as_ref(),
        payload.correct_option_index,
        payload.explanation.as_deref().map(str::trim),
        payload.assessment_type,
        payload.difficulty_level,
        payload.status,
    )
    .await
    .map_err(PublicError::from)?
    .ok_or(PublicError::NotFound)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.question_updated",
        None,
        None,
        Some(serde_json::json!({ "question_id": question.id })),
    )
    .await;

    Ok(Json(question.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dg_core::{Actor, UserId};

    /// The authorization rule every route here applies once
    /// `question -> course -> program_id` is resolved.
    fn authorize(actor: &Actor, program_id: ProgramId) -> Result<(), PublicError> {
        actor.require_scoped(Capability::ManagePrograms, program_id)
    }

    // -- RBAC matrix -------------------------------------------------------

    #[test]
    fn super_admin_may_author_questions_in_any_program() {
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert!(authorize(&actor, ProgramId::new()).is_ok());
    }

    #[test]
    fn sub_admin_may_author_questions_only_in_scoped_programs() {
        let scoped = ProgramId::new();
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![scoped]);
        assert!(authorize(&actor, scoped).is_ok());
        assert_eq!(
            authorize(&actor, ProgramId::new())
                .expect_err("out of scope")
                .code(),
            "FORBIDDEN"
        );
    }

    #[test]
    fn sub_admin_with_no_scopes_is_forbidden_every_question() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![]);
        assert!(authorize(&actor, ProgramId::new()).is_err());
    }

    #[test]
    fn a_student_is_forbidden_every_question_pool_route() {
        let program_id = ProgramId::new();
        // Even holding the program in `scopes`: `in_scope` is false for a
        // student regardless of what the vector says.
        let actor = Actor::new(UserId::new(), Role::Student, vec![program_id]);
        assert_eq!(
            authorize(&actor, program_id)
                .expect_err("students never author questions")
                .code(),
            "FORBIDDEN"
        );
        assert!(actor.require(Capability::ManagePrograms).is_err());
    }

    #[test]
    fn a_student_cannot_reach_the_bulk_import() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert_eq!(
            actor
                .require(Capability::ManagePrograms)
                .expect_err("bulk import is admin-only")
                .code(),
            "FORBIDDEN"
        );
    }

    #[test]
    fn a_sub_admin_with_no_scopes_gets_an_empty_list_not_the_unscoped_query() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![]);
        let scope = list_scope(&actor).expect("a sub-admin may list");
        assert_eq!(
            scope,
            Some(Vec::new()),
            "an empty scope vector is the filter, not a reason to drop it"
        );
    }

    #[test]
    fn a_super_admin_list_is_unscoped() {
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert_eq!(list_scope(&actor).expect("unscoped"), None);
    }

    #[test]
    fn list_scope_refuses_a_student_outright() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert_eq!(
            list_scope(&actor).expect_err("never reachable").code(),
            "FORBIDDEN"
        );
    }

    // -- PATCH validation --------------------------------------------------

    #[test]
    fn an_empty_patch_is_accepted_as_a_no_op() {
        let payload = UpdateQuestionRequest {
            block_id: None,
            topic: None,
            question_text: None,
            options: None,
            correct_option_index: None,
            explanation: None,
            assessment_type: None,
            difficulty_level: None,
            status: None,
        };
        assert!(payload.validate().is_ok());
    }

    #[test]
    fn a_patch_cannot_blank_a_mandatory_field_or_break_the_answer_key() {
        let mut blank = UpdateQuestionRequest {
            block_id: None,
            topic: Some("  ".into()),
            question_text: None,
            options: None,
            correct_option_index: None,
            explanation: None,
            assessment_type: None,
            difficulty_level: None,
            status: None,
        };
        assert_eq!(
            blank.validate().expect_err("blank topic").code(),
            "VALIDATION_ERROR"
        );

        blank.topic = None;
        blank.correct_option_index = Some(4);
        assert!(blank.validate().is_err());

        blank.correct_option_index = None;
        blank.options = Some(vec!["A".into(), "B".into()]);
        assert!(blank.validate().is_err());
    }

    /// A patch has no `course_id`: exhaustive destructuring so adding one stops
    /// this compiling, because moving a question between courses would move it
    /// past the scope check it was authorized under *and* into another paper's
    /// pool.
    #[test]
    fn a_patch_cannot_move_a_question_to_another_course() {
        let UpdateQuestionRequest {
            block_id,
            topic,
            question_text,
            options,
            correct_option_index,
            explanation,
            assessment_type,
            difficulty_level,
            status,
        } = UpdateQuestionRequest {
            block_id: None,
            topic: None,
            question_text: None,
            options: None,
            correct_option_index: None,
            explanation: None,
            assessment_type: None,
            difficulty_level: None,
            status: Some(QuestionStatus::Retired),
        };
        assert!(block_id.is_none() && topic.is_none() && question_text.is_none());
        assert!(options.is_none() && correct_option_index.is_none() && explanation.is_none());
        assert!(assessment_type.is_none() && difficulty_level.is_none());
        assert_eq!(status, Some(QuestionStatus::Retired));
    }

    // -- Admin response shape ---------------------------------------------

    /// The mirror image of the student test: an *admin* response must carry the
    /// key, because an author has to see what they authored.
    #[test]
    fn an_admin_response_does_carry_the_answer_key() {
        let json = serde_json::to_value(QuestionResponse {
            id: Uuid::nil(),
            course_id: Uuid::nil(),
            course_code: "ML101".into(),
            semester_id: Uuid::nil(),
            program_id: Uuid::nil(),
            block_id: None,
            block_no: None,
            block_title: None,
            topic: "Phonology".into(),
            question_text: "What is sandhi?".into(),
            options: vec!["A".into(), "B".into(), "C".into(), "D".into()],
            correct_option_index: 1,
            explanation: "Euphony.".into(),
            assessment_type: AssessmentType::Assignment,
            difficulty_level: DifficultyLevel::Beginner,
            status: QuestionStatus::Active,
            created_by: Uuid::nil(),
            created_at: Utc::now(),
        })
        .expect("serialises");

        assert_eq!(json["correct_option_index"], 1);
        assert!(json["explanation"].is_string());
        assert_eq!(json["options"].as_array().map(Vec::len), Some(4));
        // The unit/module is optional, and its three fields are null together.
        assert!(json["block_id"].is_null());
        assert!(json["block_no"].is_null());
        assert!(json["block_title"].is_null());
        // The pool's own course is never null.
        assert!(json["course_id"].is_string());
    }

    #[test]
    fn a_course_wide_question_is_the_normal_create_shape() {
        let payload: CreateQuestionRequest = serde_json::from_value(serde_json::json!({
            "course_id": Uuid::nil(),
            "topic": "Phonology",
            "question_text": "What is sandhi?",
            "options": ["A", "B", "C", "D"],
            "correct_option_index": 1,
            "explanation": "Euphony.",
            "assessment_type": "assignment"
        }))
        .expect("block_id and difficulty_level are optional");

        let fields = QuestionFields::from(payload);
        assert!(fields.block_id.is_none());
        assert_eq!(fields.difficulty_level, DifficultyLevel::Beginner);
        assert!(validate_fields(&fields).is_ok());
    }

    #[test]
    fn a_create_without_a_course_is_rejected_by_deserialisation() {
        let err = serde_json::from_value::<CreateQuestionRequest>(serde_json::json!({
            "topic": "Phonology",
            "question_text": "What is sandhi?",
            "options": ["A", "B", "C", "D"],
            "correct_option_index": 1,
            "explanation": "Euphony.",
            "assessment_type": "assignment"
        }));
        assert!(err.is_err(), "course_id is required: a pool is per course");
    }
}
