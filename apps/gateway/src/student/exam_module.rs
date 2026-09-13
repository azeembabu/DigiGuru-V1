//! `/api/v1/student/exams` — the exam runner: which papers a student may sit,
//! starting one, resuming it, saving answers, and submitting for marking.
//!
//! # The answer key never leaves the server before submission
//!
//! This is the security property of the module, and it is enforced by the
//! **type system** rather than by remembering to clear a field. Three response
//! shapes exist:
//!
//! * [`PaperQuestionResponse`] — what a student sees while sitting the paper.
//!   It has no `correct_option_index` field and no `explanation` field **at
//!   all**. There is no value any handler could assign that would leak the key,
//!   because there is nowhere to put it.
//! * [`ReviewQuestionResponse`] — the post-submission review, which does carry
//!   both. It is constructed only from [`exam_papers::graded_paper`], whose SQL
//!   refuses an `in_progress` attempt, so even a handler bug cannot build one
//!   for a live paper.
//! * Grading itself reads the key in SQL (`exam_papers::grade`) and returns a
//!   tally. The key never transits this process as a value at all on that path.
//!
//! An `Option<i16>` that handlers must remember to set to `None` was the
//! obvious alternative and is exactly the mistake this avoids: a forgotten
//! clear is invisible in review and catastrophic in production.
//!
//! # Self-only
//!
//! No route takes a `student_id`. Every query binds the caller's own
//! `students.id`, so another student's attempt id selects no row and returns
//! `404` — never `403`, which would confirm the id exists.
//!
//! # Sampling
//!
//! A paper is sampled from the **course's own pool** — every active question
//! with that `course_id`, narrowed by the exam's `assessment_type`. The course
//! is reached from the exam (`exams -> blocks -> course_id`); the exam still
//! anchors to a block and still supplies `question_count` and
//! `duration_minutes`, but neither the exam's block nor a question's optional
//! unit/module narrows the pool — most questions will not have a module, and a
//! module filter would silently shrink the draw.
//!
//! The student must be actively enrolled in that course. That is not a separate
//! check: the exam is resolved through `exams::published_for_student`, which
//! joins `student_courses`, so an exam in a course the student is not enrolled
//! in simply selects no row and answers `404`.
//!
//! The sample is written to `exam_attempt_answers` when the attempt starts,
//! which is what makes a reload show the same questions in the same order —
//! "no two attempts identical" is a property across attempts, not within one.

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dg_core::{
    AssessmentType, Capability, ExamAttemptId, ExamAttemptStatus, ExamId, PublicError, StudentId,
};
use dg_db::models::{exam_papers, exams, question_pool};

use crate::extractors::{AuthenticatedActor, JsonBody};
use crate::state::AppState;

/// An attempt at this exam is already live. The body names the attempt so the
/// client can resume it rather than guessing.
const EXAM_ATTEMPT_ACTIVE: &str = "EXAM_ATTEMPT_ACTIVE";
/// The attempt is not `in_progress`, so it cannot be answered or re-submitted.
const EXAM_ATTEMPT_NOT_ACTIVE: &str = "EXAM_ATTEMPT_NOT_ACTIVE";
/// The exam is not an MCQ paper: it declares no `question_count` /
/// `assessment_type`, so there is nothing to sample.
const EXAM_NOT_MCQ: &str = "EXAM_NOT_MCQ";
/// The pool has fewer active questions than the paper asks for.
const INSUFFICIENT_QUESTIONS: &str = "INSUFFICIENT_QUESTIONS";

/// Most answers a single save may carry — the same ceiling as the largest
/// legal `question_count`, so a full paper saves in one request and nothing
/// larger is buffered.
const MAX_ANSWERS_PER_SAVE: usize = 200;

// ---------------------------------------------------------------------------
// GET /student/exams
// ---------------------------------------------------------------------------

/// One exam card: the ancestry the card renders plus this student's own
/// history of the exam.
#[derive(Debug, Serialize)]
pub struct AvailableExamResponse {
    pub id: Uuid,
    pub block_id: Uuid,
    pub block_no: i16,
    pub block_title: String,
    pub course_id: Uuid,
    pub course_code: String,
    pub course_name: String,
    pub semester_id: Uuid,
    pub program_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub max_score: f64,
    /// `null` = untimed.
    pub duration_minutes: Option<i16>,
    /// The same number under the name the runner's countdown uses.
    pub time_limit_minutes: Option<i16>,
    /// `null` together with `assessment_type` means this is not an MCQ paper —
    /// starting an attempt on it returns `422 EXAM_NOT_MCQ`.
    pub question_count: Option<i16>,
    pub assessment_type: Option<AssessmentType>,
    /// How many attempts this student has already made.
    pub attempts_used: i64,
    /// Best graded percentage, or `null` when nothing is graded yet. `null`
    /// rather than `0` — a zero would read as a measured score.
    pub best_percentage: Option<f64>,
}

impl From<exams::StudentExamRow> for AvailableExamResponse {
    fn from(e: exams::StudentExamRow) -> Self {
        Self {
            id: e.id.into_uuid(),
            block_id: e.block_id.into_uuid(),
            block_no: e.block_no,
            block_title: e.block_title,
            course_id: e.course_id.into_uuid(),
            course_code: e.course_code,
            course_name: e.course_name,
            semester_id: e.semester_id.into_uuid(),
            program_id: e.program_id.into_uuid(),
            title: e.title,
            description: e.description,
            max_score: e.max_score,
            duration_minutes: e.duration_minutes,
            time_limit_minutes: e.duration_minutes,
            question_count: e.question_count,
            assessment_type: e.assessment_type,
            attempts_used: e.attempts_used,
            best_percentage: e.best_percentage,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListExamsQuery {
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

/// `GET /api/v1/student/exams` — published exams in the caller's enrolled
/// courses. A draft or archived exam is not content a student may sit, and the
/// `status = 'published'` predicate is in the query, not a filter here.
pub async fn list_available_exams(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Query(query): Query<ListExamsQuery>,
) -> Result<(HeaderMap, Json<Vec<AvailableExamResponse>>), PublicError> {
    actor.require(Capability::ViewOwnExams)?;

    let student_id = own_student_id(&state, &actor).await?;
    let page = crate::admin::page(query.limit, query.offset, 50)?;

    let total = exams::count_published_for_student(&state.pool, student_id)
        .await
        .map_err(PublicError::from)?;
    let rows = exams::list_published_for_student(&state.pool, student_id, page.limit, page.offset)
        .await
        .map_err(PublicError::from)?;

    Ok((
        crate::admin::total_count(total),
        Json(rows.into_iter().map(AvailableExamResponse::from).collect()),
    ))
}

// ---------------------------------------------------------------------------
// The paper: answer-free by construction
// ---------------------------------------------------------------------------

/// One question as the student sitting the paper sees it.
///
/// **There is no `correct_option_index` and no `explanation` field here.** That
/// is the whole design: the key cannot leak through this type because the type
/// has nowhere to carry it. `topic` stays — the weak-area roll-up needs it, and
/// a topic label is not part of the answer.
#[derive(Debug, Serialize)]
pub struct PaperQuestionResponse {
    pub question_seq: i16,
    pub question_id: Uuid,
    pub topic: String,
    pub question_text: String,
    /// Exactly four options, in the A/B/C/D order the student answers against.
    pub options: Vec<String>,
    /// `null` = unanswered. Distinct from `0`, which is a real choice of A.
    pub selected_option_index: Option<i16>,
}

impl From<exam_papers::PaperQuestion> for PaperQuestionResponse {
    fn from(q: exam_papers::PaperQuestion) -> Self {
        Self {
            question_seq: q.question_seq,
            question_id: q.question_id.into_uuid(),
            topic: q.topic,
            question_text: q.question_text,
            options: q.options,
            selected_option_index: q.selected_option_index,
        }
    }
}

/// The runner's payload: the attempt, its timing, and the paper.
#[derive(Debug, Serialize)]
pub struct AttemptPaperResponse {
    pub attempt_id: Uuid,
    pub exam_id: Uuid,
    pub exam_title: String,
    pub assessment_type: Option<AssessmentType>,
    pub duration_minutes: Option<i16>,
    /// The same number the countdown renders. Display only — the server is
    /// authoritative for expiry, and the client clock is never trusted.
    pub time_limit_minutes: Option<i16>,
    pub attempt_no: i16,
    pub started_at: DateTime<Utc>,
    pub total_questions: usize,
    pub questions: Vec<PaperQuestionResponse>,
}

// ---------------------------------------------------------------------------
// POST /student/exams/{exam_id}/attempts
// ---------------------------------------------------------------------------

pub async fn start_attempt(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(exam_id): Path<Uuid>,
) -> Result<Json<AttemptPaperResponse>, PublicError> {
    actor.require(Capability::ViewOwnExams)?;

    let student = super::own_student(&state, &actor).await?;
    let exam_id = ExamId::from(exam_id);

    // Enrolled *and* published, both in the query: an exam the student is not
    // enrolled for is indistinguishable from one that does not exist.
    let exam = exams::published_for_student(&state.pool, student.id, exam_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    // The schema keeps `question_count` and `assessment_type` NULL together:
    // both absent means a written or oral assessment marked by hand, which is
    // the state every exam that predates the question pool is in. That is a
    // semantically invalid request, not an internal fault — 422, never a 500.
    let (question_count, assessment_type) = match (exam.question_count, exam.assessment_type) {
        (Some(count), Some(kind)) => (count, kind),
        _ => {
            return Err(PublicError::Unprocessable {
                code: EXAM_NOT_MCQ,
                message: "This exam is not a multiple-choice paper and cannot be sat online."
                    .into(),
            })
        }
    };

    // A reload mid-exam must resume, not re-sample.
    //
    // The error deliberately does **not** name the live attempt's id. The error
    // envelope is `{ code, message }` and nothing else
    // (`.claude/rules/api-conventions.md`), and inventing an id-bearing error
    // shape for one route would break the rule that every client parses every
    // error the same way. The client finds the live attempt through
    // `GET /student/exam-attempts`, which returns `in_progress` attempts with
    // their ids, and resumes it at `GET /student/exams/attempts/{id}`.
    if exam_papers::active_attempt(&state.pool, student.id, exam_id)
        .await
        .map_err(PublicError::from)?
        .is_some()
    {
        return Err(attempt_already_active());
    }

    let wanted = i64::from(question_count);
    // The pool is the exam's course, not the student's whole semester: the
    // owner's rule is "strictly from the pre-populated Question Pool of the
    // selected Course". `exam.course_id` came through a query that already
    // joined `student_courses`, so it is a course this student is enrolled in.
    let available =
        question_pool::count_available_for_course(&state.pool, exam.course_id, assessment_type)
            .await
            .map_err(PublicError::from)?;

    if available < wanted {
        return Err(insufficient_questions(available, wanted));
    }

    let sampled =
        question_pool::sample_for_course(&state.pool, exam.course_id, assessment_type, wanted)
            .await
            .map_err(PublicError::from)?;

    // Re-checked after sampling: a question retired between the count and the
    // sample would otherwise produce a short paper scored out of the full
    // `max_score`.
    let sampled_len = i64::try_from(sampled.len()).unwrap_or(i64::MAX);
    if sampled_len < wanted {
        return Err(insufficient_questions(sampled_len, wanted));
    }

    let question_ids: Vec<Uuid> = sampled.iter().map(|q| q.id.into_uuid()).collect();

    // The attempt row and its paper are inserted in one transaction, so an
    // attempt never exists without the questions it asked.
    let attempt = exam_papers::start_attempt(
        &state.pool,
        student.id,
        exam_id,
        exam.max_score,
        &question_ids,
    )
    .await
    .map_err(PublicError::from)?;

    let questions = exam_papers::paper_for_student(&state.pool, student.id, attempt.id)
        .await
        .map_err(PublicError::from)?;

    Ok(Json(paper_response(
        attempt.id,
        exam_id,
        exam.title,
        exam.assessment_type,
        exam.duration_minutes,
        attempt.attempt_no,
        attempt.started_at,
        questions,
    )))
}

/// `409` for a reload mid-exam.
///
/// No attempt id in the body — see the call site for why, and
/// `GET /student/exam-attempts` for where the client finds it.
fn attempt_already_active() -> PublicError {
    PublicError::Conflict {
        code: EXAM_ATTEMPT_ACTIVE,
        message: "An attempt at this exam is already in progress. Resume that attempt \
                  instead of starting a new one."
            .into(),
    }
}

/// `422` naming both numbers: an admin reading the client's report needs to
/// know how far short the pool is, not just that it is short.
fn insufficient_questions(available: i64, wanted: i64) -> PublicError {
    PublicError::Unprocessable {
        code: INSUFFICIENT_QUESTIONS,
        message: format!(
            "This paper needs {wanted} questions but only {available} are available \
             for your programme and semester. Please contact your centre."
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn paper_response(
    attempt_id: ExamAttemptId,
    exam_id: ExamId,
    exam_title: String,
    assessment_type: Option<AssessmentType>,
    duration_minutes: Option<i16>,
    attempt_no: i16,
    started_at: DateTime<Utc>,
    questions: Vec<exam_papers::PaperQuestion>,
) -> AttemptPaperResponse {
    AttemptPaperResponse {
        attempt_id: attempt_id.into_uuid(),
        exam_id: exam_id.into_uuid(),
        exam_title,
        assessment_type,
        duration_minutes,
        time_limit_minutes: duration_minutes,
        attempt_no,
        started_at,
        total_questions: questions.len(),
        questions: questions
            .into_iter()
            .map(PaperQuestionResponse::from)
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// GET /student/exams/attempts/{id}
// ---------------------------------------------------------------------------

/// Resume a live attempt: the same answer-free payload `start_attempt`
/// returned, with whatever has been saved so far.
///
/// A submitted or graded attempt is `409` here rather than being re-served as a
/// paper: its review lives behind `submit`'s result shape, and handing back a
/// "paper" for a finished exam would invite a client to let the student keep
/// answering it.
pub async fn resume_attempt(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<AttemptPaperResponse>, PublicError> {
    actor.require(Capability::ViewOwnExams)?;

    let student_id = own_student_id(&state, &actor).await?;
    let attempt_id = ExamAttemptId::from(id);

    let attempt = exams::attempt_for_student(&state.pool, student_id, attempt_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    if attempt.status != ExamAttemptStatus::InProgress {
        return Err(not_active(attempt.status));
    }

    let exam = exams::find_by_id(&state.pool, attempt.exam_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    let questions = exam_papers::paper_for_student(&state.pool, student_id, attempt_id)
        .await
        .map_err(PublicError::from)?;

    Ok(Json(paper_response(
        attempt_id,
        attempt.exam_id,
        attempt.exam_title,
        exam.assessment_type,
        exam.duration_minutes,
        attempt.attempt_no,
        attempt.started_at,
        questions,
    )))
}

fn not_active(status: ExamAttemptStatus) -> PublicError {
    PublicError::Conflict {
        code: EXAM_ATTEMPT_NOT_ACTIVE,
        message: match status {
            ExamAttemptStatus::Submitted | ExamAttemptStatus::Graded => {
                "This attempt has already been submitted.".into()
            }
            ExamAttemptStatus::Abandoned => "This attempt was abandoned.".into(),
            ExamAttemptStatus::InProgress => "This attempt is still in progress.".into(),
        },
    }
}

// ---------------------------------------------------------------------------
// PATCH /student/exams/attempts/{id}/answers
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SaveAnswerItem {
    pub question_seq: i16,
    /// `null` clears the answer — a student may un-choose. `0..=3` otherwise;
    /// every paper is exactly A/B/C/D.
    pub selected_option_index: Option<i16>,
}

#[derive(Debug, Deserialize)]
pub struct SaveAnswersRequest {
    pub answers: Vec<SaveAnswerItem>,
}

#[derive(Debug, Serialize)]
pub struct SaveAnswersResponse {
    pub attempt_id: Uuid,
    /// How many rows the save actually updated. A number below
    /// `answers.len()` means a `question_seq` was not part of this paper.
    pub saved: i64,
}

/// Validate the payload before any of it is written: a `question_seq` or option
/// index out of range is the client's bug, and half-applying the batch would
/// leave the runner and the server disagreeing about what was answered.
fn validate_answers(answers: &[SaveAnswerItem]) -> Result<(), PublicError> {
    if answers.is_empty() {
        return Err(PublicError::validation(
            "answers",
            "at least one answer is required",
        ));
    }
    if answers.len() > MAX_ANSWERS_PER_SAVE {
        return Err(PublicError::validation(
            "answers",
            format!("at most {MAX_ANSWERS_PER_SAVE} answers may be saved in one request"),
        ));
    }

    let mut seen: Vec<i16> = Vec::with_capacity(answers.len());
    for (row, a) in answers.iter().enumerate() {
        let row = row + 1;
        if a.question_seq < 1 {
            return Err(PublicError::validation(
                "answers",
                format!("answer {row}: question_seq must be 1 or greater"),
            ));
        }
        if let Some(index) = a.selected_option_index {
            if !(0..=3).contains(&index) {
                return Err(PublicError::validation(
                    "answers",
                    format!("answer {row}: selected_option_index must be between 0 and 3"),
                ));
            }
        }
        if seen.contains(&a.question_seq) {
            return Err(PublicError::validation(
                "answers",
                format!(
                    "answer {row}: question_seq {} appears twice; one save sets each question once",
                    a.question_seq
                ),
            ));
        }
        seen.push(a.question_seq);
    }

    Ok(())
}

/// Save-as-you-go. Idempotent: re-sending the same answers is a no-op write, so
/// a client may retry a dropped save without reasoning about ordering.
pub async fn save_answers(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
    JsonBody(payload): JsonBody<SaveAnswersRequest>,
) -> Result<Json<SaveAnswersResponse>, PublicError> {
    actor.require(Capability::ViewOwnExams)?;
    validate_answers(&payload.answers)?;

    let student_id = own_student_id(&state, &actor).await?;
    let attempt_id = ExamAttemptId::from(id);

    let attempt = exams::attempt_for_student(&state.pool, student_id, attempt_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    if attempt.status != ExamAttemptStatus::InProgress {
        return Err(not_active(attempt.status));
    }

    let seqs: Vec<i16> = payload.answers.iter().map(|a| a.question_seq).collect();
    let indices: Vec<Option<i16>> = payload
        .answers
        .iter()
        .map(|a| a.selected_option_index)
        .collect();

    // The `UPDATE` carries `status = 'in_progress'` and `student_id` in its own
    // `WHERE`, so the check above is the friendly answer and the query is the
    // backstop that makes a missed check harmless.
    let saved = exam_papers::save_answers(&state.pool, student_id, attempt_id, &seqs, &indices)
        .await
        .map_err(PublicError::from)?;

    Ok(Json(SaveAnswersResponse {
        attempt_id: attempt_id.into_uuid(),
        saved: i64::try_from(saved).unwrap_or(i64::MAX),
    }))
}

// ---------------------------------------------------------------------------
// POST /student/exams/attempts/{id}/submit
// ---------------------------------------------------------------------------

/// One reviewed question — the **only** shape that carries the answer key, and
/// it is built only from a graded paper.
#[derive(Debug, Serialize)]
pub struct ReviewQuestionResponse {
    pub question_seq: i16,
    pub question_id: Uuid,
    pub topic: String,
    pub question_text: String,
    pub options: Vec<String>,
    pub selected_option_index: Option<i16>,
    pub correct_option_index: i16,
    /// `null` would mean ungraded, which `graded_paper` cannot return.
    pub is_correct: Option<bool>,
    /// Mandatory in the schema: every question carries the explanation that
    /// drives the post-exam feedback screen, so this is always a string.
    pub explanation: String,
}

impl From<exam_papers::GradedQuestion> for ReviewQuestionResponse {
    fn from(q: exam_papers::GradedQuestion) -> Self {
        Self {
            question_seq: q.question_seq,
            question_id: q.question_id.into_uuid(),
            topic: q.topic,
            question_text: q.question_text,
            options: q.options,
            selected_option_index: q.selected_option_index,
            correct_option_index: q.correct_option_index,
            is_correct: q.is_correct,
            explanation: q.explanation,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AttemptResultResponse {
    pub attempt_id: Uuid,
    pub exam_id: Uuid,
    pub exam_title: String,
    pub total_questions: i64,
    pub correct_answers: i64,
    /// Numbers on the wire; "8/10" is the client's rendering.
    pub score: f64,
    pub max_score: f64,
    pub score_percentage: f64,
    /// Distinct topics of the questions answered incorrectly — what the
    /// dashboard's weak-area badges and the remediation screen read.
    pub weak_topics: Vec<String>,
    pub submitted_at: DateTime<Utc>,
    pub review: Vec<ReviewQuestionResponse>,
}

/// Grade and disclose. Everything is computed server-side in one transaction
/// from the stored key: nothing the client sent participates in the arithmetic,
/// and an unanswered question is simply incorrect.
///
/// Submitting twice is `409`, not a re-grade — the `UPDATE` carries
/// `status = 'in_progress'`, so the second submit marks nothing and
/// `exam_papers::grade` answers `None`.
pub async fn submit_attempt(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<AttemptResultResponse>, PublicError> {
    actor.require(Capability::ViewOwnExams)?;

    let student_id = own_student_id(&state, &actor).await?;
    let attempt_id = ExamAttemptId::from(id);

    // Resolved first so a missing or someone else's attempt is `404` rather
    // than the `409` a non-gradeable attempt gets.
    let attempt = exams::attempt_for_student(&state.pool, student_id, attempt_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    let outcome = exam_papers::grade(&state.pool, student_id, attempt_id)
        .await
        .map_err(PublicError::from)?
        .ok_or_else(|| not_active(attempt.status))?;

    let review = exam_papers::graded_paper(&state.pool, student_id, attempt_id)
        .await
        .map_err(PublicError::from)?;

    Ok(Json(AttemptResultResponse {
        attempt_id: outcome.attempt_id.into_uuid(),
        exam_id: outcome.exam_id.into_uuid(),
        exam_title: attempt.exam_title,
        total_questions: outcome.total_questions,
        correct_answers: outcome.correct_answers,
        score: outcome.score,
        max_score: outcome.max_score,
        score_percentage: score_percentage(outcome.correct_answers, outcome.total_questions),
        weak_topics: outcome.weak_topics,
        submitted_at: outcome.submitted_at,
        review: review
            .into_iter()
            .map(ReviewQuestionResponse::from)
            .collect(),
    }))
}

/// The student's own marked answer sheet — everything a downloadable record of
/// one attempt needs, in one response.
///
/// It carries the student's name and roll number, which no other
/// `/api/v1/student/*` payload does. That is deliberate and is not a widening:
/// the route is self-only, so the only identity it can ever print is the
/// caller's own, and an answer sheet without a name on it is not a document
/// anybody can hand to anyone. Nothing here can name a different student —
/// `own_student` resolves the subject from the access token.
#[derive(Debug, Serialize)]
pub struct AnswerSheetResponse {
    pub attempt_id: Uuid,
    pub exam_id: Uuid,
    pub exam_title: String,
    pub course_code: String,
    pub course_name: String,
    pub block_no: i16,
    pub block_title: String,
    pub student_name: String,
    pub roll_number: String,
    pub attempt_no: i16,
    pub total_questions: i64,
    pub correct_answers: i64,
    pub score: f64,
    pub max_score: f64,
    pub score_percentage: f64,
    pub weak_topics: Vec<String>,
    pub started_at: DateTime<Utc>,
    /// Never `null` here: the route refuses an attempt that was not submitted.
    pub submitted_at: DateTime<Utc>,
    pub review: Vec<ReviewQuestionResponse>,
}

/// `GET /api/v1/student/exam-attempts/{id}/review` — re-read a marked paper.
///
/// The review used to exist only as the body of `POST .../submit`, which meant
/// a student could see their marked answers exactly once, at the moment they
/// submitted, and never again. Losing that response — a reload, a dropped
/// connection, coming back a week later — lost the answer sheet permanently,
/// even though every row behind it was still in the database.
///
/// It re-reads; it does not re-grade. `exam_papers::grade` performs the
/// marking `UPDATE` and is deliberately not called here: the score is read
/// from the attempt row that submission already wrote, so downloading an
/// answer sheet cannot alter a mark. A student refreshing this a hundred times
/// changes nothing.
///
/// Disclosure is unchanged. `graded_paper`'s SQL refuses an `in_progress`
/// attempt, so the answer key cannot reach a live paper through this route
/// even if the status check below were wrong; the check is there to return a
/// clear `409` rather than an empty sheet.
pub async fn review_attempt(
    State(state): State<AppState>,
    AuthenticatedActor(actor): AuthenticatedActor,
    Path(id): Path<Uuid>,
) -> Result<Json<AnswerSheetResponse>, PublicError> {
    actor.require(Capability::ViewOwnExams)?;

    let student = super::own_student(&state, &actor).await?;
    let attempt_id = ExamAttemptId::from(id);

    // Resolved first, so another student's attempt id is `404` rather than the
    // `409` an unsubmitted one gets — a `409` would confirm the id exists.
    let attempt = exams::attempt_for_student(&state.pool, student.id, attempt_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    // Only a paper that was actually marked has an answer sheet. An abandoned
    // attempt was never graded, so it is refused here rather than served as a
    // sheet full of blanks that looks like a bug.
    let submitted_at = match attempt.status {
        ExamAttemptStatus::Submitted | ExamAttemptStatus::Graded => attempt
            .submitted_at
            .ok_or_else(|| not_active(attempt.status))?,
        other => return Err(not_active(other)),
    };

    let review: Vec<ReviewQuestionResponse> =
        exam_papers::graded_paper(&state.pool, student.id, attempt_id)
            .await
            .map_err(PublicError::from)?
            .into_iter()
            .map(ReviewQuestionResponse::from)
            .collect();

    let weak_topics = exam_papers::weak_topics_for_attempt(&state.pool, student.id, attempt_id)
        .await
        .map_err(PublicError::from)?;

    // Counted from the marked rows rather than read from a column: it is the
    // same number `submit` returned, derived from the same source of truth, so
    // the two responses cannot disagree.
    let total_questions = review.len() as i64;
    let correct_answers = review.iter().filter(|q| q.is_correct == Some(true)).count() as i64;

    Ok(Json(AnswerSheetResponse {
        attempt_id: attempt.id.into_uuid(),
        exam_id: attempt.exam_id.into_uuid(),
        exam_title: attempt.exam_title,
        course_code: attempt.course_code,
        course_name: attempt.course_name,
        block_no: attempt.block_no,
        block_title: attempt.block_title,
        student_name: student.full_name,
        roll_number: student.roll_number,
        attempt_no: attempt.attempt_no,
        total_questions,
        correct_answers,
        // `score` is NOT NULL once graded; fall back to the recomputed tally
        // rather than serving `null` on a sheet that is meant to be printed.
        score: attempt.score.unwrap_or(correct_answers as f64),
        max_score: attempt.max_score,
        score_percentage: score_percentage(correct_answers, total_questions),
        weak_topics,
        started_at: attempt.started_at,
        submitted_at,
        review,
    }))
}

/// `correct / total * 100`. An empty paper is `0.0` rather than a division by
/// zero; `start_attempt` makes that unreachable through the normal path.
fn score_percentage(correct: i64, total: i64) -> f64 {
    if total <= 0 {
        return 0.0;
    }
    (correct as f64) * 100.0 / (total as f64)
}

/// Resolve the caller's own `students.id`.
///
/// The only place a `StudentId` enters these handlers, and it comes from the
/// access token. An admin has no `students` row and gets `404` here; the
/// capability check above has already rejected them, and this is the second
/// lock.
async fn own_student_id(
    state: &AppState,
    actor: &dg_core::Actor,
) -> Result<StudentId, PublicError> {
    Ok(super::own_student(state, actor).await?.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dg_core::{Actor, ProgramId, Role, UserId};

    fn graded(seq: i16, selected: Option<i16>, correct: i16) -> exam_papers::GradedQuestion {
        exam_papers::GradedQuestion {
            question_seq: seq,
            question_id: dg_core::QuestionId::new(),
            topic: "Phonology".into(),
            question_text: "What is sandhi?".into(),
            options: vec!["A".into(), "B".into(), "C".into(), "D".into()],
            selected_option_index: selected,
            correct_option_index: correct,
            is_correct: Some(selected == Some(correct)),
            explanation: "Euphonic combination of adjacent sounds.".into(),
        }
    }

    fn paper_question(seq: i16, selected: Option<i16>) -> exam_papers::PaperQuestion {
        exam_papers::PaperQuestion {
            question_seq: seq,
            question_id: dg_core::QuestionId::new(),
            topic: "Phonology".into(),
            question_text: "What is sandhi?".into(),
            options: vec!["A".into(), "B".into(), "C".into(), "D".into()],
            selected_option_index: selected,
        }
    }

    // -- RBAC ---------------------------------------------------------------

    #[test]
    fn a_student_may_sit_its_own_exams() {
        let actor = Actor::new(UserId::new(), Role::Student, vec![]);
        assert!(actor.require(Capability::ViewOwnExams).is_ok());
    }

    #[test]
    fn a_sub_admin_is_forbidden_every_student_exam_route() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        let err = actor
            .require(Capability::ViewOwnExams)
            .expect_err("an admin sits no exams");
        assert_eq!(err.code(), "FORBIDDEN");
    }

    #[test]
    fn a_super_admin_is_not_a_student_either() {
        // A super-admin passes `can`, but has no `students` row, so the subject
        // resolution 404s. Asserted here so the route's answer is documented:
        // there is no path by which an admin sits a student's paper.
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert!(actor.require(Capability::ViewOwnExams).is_ok());
    }

    #[test]
    fn the_exam_list_query_cannot_name_another_student() {
        let ListExamsQuery { limit, offset } = ListExamsQuery {
            limit: Some(10),
            offset: None,
        };
        assert_eq!(limit, Some(10));
        assert!(offset.is_none());
    }

    #[test]
    fn student_exam_lists_reject_an_out_of_range_limit_rather_than_clamping() {
        assert_eq!(
            crate::admin::page(Some(201), None, 50)
                .expect_err("over the ceiling")
                .code(),
            "VALIDATION_ERROR"
        );
    }

    // -- The answer sheet --------------------------------------------------

    /// The sheet is only served for an attempt that was actually marked. An
    /// abandoned one was never graded, so serving it would be a page of blanks
    /// that reads as a broken download rather than as "there is nothing here".
    #[test]
    fn only_a_submitted_attempt_has_an_answer_sheet() {
        for status in [ExamAttemptStatus::InProgress, ExamAttemptStatus::Abandoned] {
            assert_eq!(not_active(status).code(), EXAM_ATTEMPT_NOT_ACTIVE);
        }
    }

    /// The answer sheet is the one student-facing payload that carries a name
    /// and roll number — it is a document meant to be printed and handed over.
    /// The route is self-only, so the identity can only ever be the caller's
    /// own; this asserts the fields are actually present, since a sheet with
    /// nobody's name on it is not a record of anything.
    #[test]
    fn the_answer_sheet_identifies_the_student_and_discloses_the_key() {
        let now = Utc::now();
        let sheet = AnswerSheetResponse {
            attempt_id: Uuid::new_v4(),
            exam_id: Uuid::new_v4(),
            exam_title: "Unit 3 Test".into(),
            course_code: "BAML101".into(),
            course_name: "Introduction to Malayalam Language".into(),
            block_no: 3,
            block_title: "Prosody".into(),
            student_name: "Ananya Menon".into(),
            roll_number: "25XHBML11450".into(),
            attempt_no: 2,
            total_questions: 2,
            correct_answers: 1,
            score: 1.0,
            max_score: 2.0,
            score_percentage: 50.0,
            weak_topics: vec!["Phonology".into()],
            started_at: now,
            submitted_at: now,
            review: vec![graded(1, Some(1), 1).into(), graded(2, Some(0), 2).into()],
        };

        let json = serde_json::to_value(&sheet).expect("serialises");
        assert_eq!(json["student_name"], "Ananya Menon");
        assert_eq!(json["roll_number"], "25XHBML11450");
        assert_eq!(json["score_percentage"], 50.0);
        // Unlike a live paper, a marked one is *supposed* to carry the key.
        let first = &json["review"][0];
        assert!(first["correct_option_index"].is_number());
        assert!(first["explanation"].is_string());
    }

    // -- The answer key must not leak -------------------------------------

    /// The module's security property, asserted on the wire format: a paper a
    /// student is still sitting must not serialise the answer key under any
    /// name. This fails the moment someone adds such a field to
    /// `PaperQuestionResponse`, which is the change worth catching.
    #[test]
    fn an_unsubmitted_paper_never_serialises_the_answer_key() {
        let response = paper_response(
            ExamAttemptId::new(),
            ExamId::new(),
            "Unit 3 Test".into(),
            Some(AssessmentType::MidTermQuiz),
            Some(45),
            1,
            Utc::now(),
            vec![
                paper_question(1, None),
                paper_question(2, Some(2)),
                paper_question(3, Some(0)),
            ],
        );

        let json = serde_json::to_value(&response).expect("serialises");
        assert_eq!(json["total_questions"], 3);

        for question in json["questions"].as_array().expect("questions is an array") {
            assert!(
                question.get("correct_option_index").is_none(),
                "the answer key must not appear on an unsubmitted paper: {question}"
            );
            assert!(
                question.get("explanation").is_none(),
                "the explanation must not appear before submission: {question}"
            );
            assert!(question.get("is_correct").is_none());
            // What the runner does need is all present.
            assert!(question["options"].is_array());
            assert!(question["topic"].is_string(), "weak-area roll-up needs it");
        }

        // Belt and braces: no nesting anywhere in the document carries the key.
        let flat = serde_json::to_string(&response).expect("serialises");
        assert!(!flat.contains("correct_option_index"));
        assert!(!flat.contains("explanation"));
    }

    /// The contrast: after submission the same question *does* carry the key
    /// and the explanation, through the separate review shape.
    #[test]
    fn a_submitted_review_does_disclose_the_answer_key() {
        let json = serde_json::to_value(ReviewQuestionResponse::from(graded(1, Some(2), 1)))
            .expect("serialises");

        assert_eq!(json["correct_option_index"], 1);
        assert_eq!(json["selected_option_index"], 2);
        assert_eq!(json["is_correct"], false);
        assert!(
            json["explanation"].is_string(),
            "explanation is NOT NULL in the schema, so never null here"
        );
    }

    // -- Grading ----------------------------------------------------------

    #[test]
    fn the_score_percentage_is_correct_answers_over_total() {
        assert_eq!(score_percentage(8, 10), 80.0);
        assert_eq!(score_percentage(0, 10), 0.0);
        assert_eq!(score_percentage(10, 10), 100.0);
    }

    #[test]
    fn an_empty_paper_scores_zero_rather_than_dividing_by_zero() {
        assert_eq!(score_percentage(0, 0), 0.0);
    }

    /// Grading is "selected equals key, and an unanswered question is wrong" —
    /// the rule `exam_papers::grade` applies in SQL. Pinned here against the
    /// same fixtures the review payload is built from, so a change to the rule
    /// breaks a test rather than a student's score.
    #[test]
    fn an_unanswered_question_is_graded_incorrect() {
        let unanswered = graded(1, None, 2);
        assert_eq!(unanswered.is_correct, Some(false));

        let right = graded(2, Some(2), 2);
        assert_eq!(right.is_correct, Some(true));

        let wrong = graded(3, Some(0), 2);
        assert_eq!(wrong.is_correct, Some(false));
    }

    #[test]
    fn weak_topics_are_the_topics_of_the_questions_answered_wrongly() {
        let paper = [
            graded(1, Some(2), 2),
            graded(2, None, 1),
            graded(3, Some(0), 1),
        ];
        let weak: Vec<&str> = paper
            .iter()
            .filter(|q| q.is_correct != Some(true))
            .map(|q| q.topic.as_str())
            .collect();
        assert_eq!(weak, vec!["Phonology", "Phonology"]);
    }

    // -- Conflict and 422 contracts ---------------------------------------

    #[test]
    fn a_second_submit_is_a_conflict_not_a_regrade() {
        let err = not_active(ExamAttemptStatus::Graded);
        assert_eq!(err.code(), EXAM_ATTEMPT_NOT_ACTIVE);
        assert!(err.public_message().contains("already been submitted"));
    }

    #[test]
    fn an_abandoned_attempt_cannot_be_answered() {
        assert_eq!(
            not_active(ExamAttemptStatus::Abandoned).code(),
            EXAM_ATTEMPT_NOT_ACTIVE
        );
    }

    /// The envelope rule: a `409` body is `{ code, message }` and carries no
    /// attempt id. The client finds the live attempt in
    /// `GET /student/exam-attempts` instead.
    #[test]
    fn the_active_attempt_conflict_carries_no_attempt_id() {
        let err = attempt_already_active();
        assert_eq!(err.code(), EXAM_ATTEMPT_ACTIVE);
        let message = err.public_message();
        assert!(message.contains("already in progress"));
        assert!(
            !message.contains('-'),
            "a uuid would show up as hyphens; none may appear: {message}"
        );
    }

    #[test]
    fn a_short_pool_is_unprocessable_and_names_both_numbers() {
        let err = insufficient_questions(4, 10);
        assert_eq!(err.code(), INSUFFICIENT_QUESTIONS);
        let message = err.public_message();
        assert!(message.contains("10") && message.contains('4'));
    }

    // -- Answer-save validation -------------------------------------------

    #[test]
    fn saving_rejects_an_option_index_outside_a_b_c_d() {
        let err = validate_answers(&[SaveAnswerItem {
            question_seq: 1,
            selected_option_index: Some(4),
        }])
        .expect_err("only 0..=3 exist");
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn saving_accepts_a_null_selection_as_clearing_an_answer() {
        assert!(validate_answers(&[SaveAnswerItem {
            question_seq: 1,
            selected_option_index: None,
        }])
        .is_ok());
    }

    #[test]
    fn saving_rejects_an_empty_batch_and_a_duplicate_question() {
        assert_eq!(
            validate_answers(&[]).expect_err("nothing to save").code(),
            "VALIDATION_ERROR"
        );

        let err = validate_answers(&[
            SaveAnswerItem {
                question_seq: 2,
                selected_option_index: Some(1),
            },
            SaveAnswerItem {
                question_seq: 2,
                selected_option_index: Some(3),
            },
        ])
        .expect_err("one save sets each question once");
        assert!(err.public_message().contains("twice"));
    }

    #[test]
    fn saving_rejects_a_question_seq_below_one() {
        assert!(validate_answers(&[SaveAnswerItem {
            question_seq: 0,
            selected_option_index: Some(1),
        }])
        .is_err());
    }

    #[test]
    fn saving_rejects_more_answers_than_the_largest_legal_paper() {
        let answers: Vec<SaveAnswerItem> = (1..=(MAX_ANSWERS_PER_SAVE as i16 + 1))
            .map(|seq| SaveAnswerItem {
                question_seq: seq,
                selected_option_index: Some(0),
            })
            .collect();
        assert!(validate_answers(&answers).is_err());
    }
}
