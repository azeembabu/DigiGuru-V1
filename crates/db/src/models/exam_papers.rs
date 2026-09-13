//! `exam_attempt_answers` — the per-attempt sampled paper, the save-as-you-go
//! answer sheet, and server-side grading
//! (`migrations/0006_question_pool_and_flashcards.sql`).
//!
//! The rows are written when the attempt starts, which is what makes a reload
//! show the same questions in the same order: the sample is a stored fact about
//! the attempt, not something re-drawn per request. "No two attempts identical"
//! is a property *across* attempts, supplied by
//! [`crate::models::question_pool::sample_for_semester`].
//!
//! ## Why there are two row types
//!
//! [`PaperQuestion`] is what a running attempt reads and has **no**
//! `correct_option_index` and **no** `explanation` field — not omitted from the
//! serialisation, absent from the type. [`GradedQuestion`] carries both and is
//! only ever produced by [`graded_paper`], which refuses to return anything for
//! an attempt that is still `in_progress`. The answer key therefore cannot
//! reach an unsubmitted client even through a handler bug, because there is no
//! value of the wrong type in scope to serialise.
//!
//! Every function here takes the `StudentId` and binds it into the query, so
//! there is no call shape that touches another student's attempt.

use chrono::{DateTime, Utc};
use dg_core::{ExamAttemptId, ExamAttemptStatus, ExamId, QuestionId, StudentId};
use sqlx::PgPool;

use crate::error::{Error, Result};

/// One question as a *running* attempt sees it: the stem, the four options, and
/// whatever the student has selected so far. Deliberately answer-free.
#[derive(Debug, Clone)]
pub struct PaperQuestion {
    pub question_seq: i16,
    pub question_id: QuestionId,
    /// Kept on the wire because the post-exam weak-area roll-up is per topic;
    /// dropping it would break that requirement.
    pub topic: String,
    pub question_text: String,
    pub options: Vec<String>,
    /// `None` = unanswered. Distinct from `Some(0)`, which is a real choice of
    /// option A.
    pub selected_option_index: Option<i16>,
}

/// One question as the *results* screen sees it, answer key included. Only
/// [`graded_paper`] produces these, and only for a submitted attempt.
#[derive(Debug, Clone)]
pub struct GradedQuestion {
    pub question_seq: i16,
    pub question_id: QuestionId,
    pub topic: String,
    pub question_text: String,
    pub options: Vec<String>,
    pub selected_option_index: Option<i16>,
    pub correct_option_index: i16,
    pub is_correct: Option<bool>,
    pub explanation: String,
}

/// The outcome of grading one attempt, computed entirely server-side.
#[derive(Debug, Clone)]
pub struct GradeOutcome {
    pub attempt_id: ExamAttemptId,
    pub exam_id: ExamId,
    pub total_questions: i64,
    pub correct_answers: i64,
    pub score: f64,
    pub max_score: f64,
    pub submitted_at: DateTime<Utc>,
    /// Distinct topics of the incorrect answers, alphabetical so two renders of
    /// the same result read the same.
    pub weak_topics: Vec<String>,
}

/// Write the sampled paper for an attempt in one statement.
///
/// `question_ids` is in the order the student will see them; `question_seq` is
/// derived from the array position rather than taken from the caller, so the
/// sequence is dense and 1-based by construction and cannot be passed in
/// inconsistently with the ids.
///
/// Takes an executor rather than a pool so the caller can run this inside the
/// same transaction that inserts the `exam_attempts` row — an attempt that
/// exists without its paper is not a state any reader can handle.
pub async fn insert_paper<'e, E>(
    executor: E,
    attempt_id: ExamAttemptId,
    question_ids: &[uuid::Uuid],
) -> Result<u64>
where
    E: sqlx::PgExecutor<'e>,
{
    let result = sqlx::query!(
        r#"
        INSERT INTO exam_attempt_answers (attempt_id, question_id, question_seq)
        SELECT $1, r.question_id, r.ord::smallint
        FROM unnest($2::uuid[]) WITH ORDINALITY AS r(question_id, ord)
        "#,
        attempt_id.into_uuid(),
        question_ids
    )
    .execute(executor)
    .await
    .map_err(Error::from_sqlx)?;

    Ok(result.rows_affected())
}

/// The paper for one attempt, **only** if the attempt belongs to `student_id`.
///
/// Ordered by `question_seq`, which is the address the runner pages by. The
/// ownership predicate is in the query, not a check on the returned rows: that
/// is what makes another student's attempt id select nothing rather than data
/// the handler then has to remember to reject.
pub async fn paper_for_student(
    pool: &PgPool,
    student_id: StudentId,
    attempt_id: ExamAttemptId,
) -> Result<Vec<PaperQuestion>> {
    sqlx::query_as!(
        PaperQuestion,
        r#"
        SELECT
            a.question_seq          as "question_seq!",
            a.question_id           as "question_id!: QuestionId",
            q.topic                 as "topic!",
            q.question_text         as "question_text!",
            (SELECT array_agg(o.value ORDER BY o.ord)
               FROM jsonb_array_elements_text(q.options) WITH ORDINALITY AS o(value, ord))
                                    as "options!: Vec<String>",
            a.selected_option_index as "selected_option_index"
        FROM exam_attempt_answers a
        JOIN question_pool  q  ON q.id = a.question_id
        JOIN exam_attempts  ea ON ea.id = a.attempt_id
        WHERE a.attempt_id = $2 AND ea.student_id = $1
        ORDER BY a.question_seq ASC
        "#,
        student_id.into_uuid(),
        attempt_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// The graded paper, answer key included — **only** for this student and
/// **only** once the attempt is no longer `in_progress`.
///
/// The status predicate is part of the `WHERE` clause rather than a guard the
/// handler is trusted to write: disclosing the key before submission is a
/// security failure (contract item 5), so the database refuses it for every
/// caller, present and future.
pub async fn graded_paper(
    pool: &PgPool,
    student_id: StudentId,
    attempt_id: ExamAttemptId,
) -> Result<Vec<GradedQuestion>> {
    sqlx::query_as!(
        GradedQuestion,
        r#"
        SELECT
            a.question_seq          as "question_seq!",
            a.question_id           as "question_id!: QuestionId",
            q.topic                 as "topic!",
            q.question_text         as "question_text!",
            (SELECT array_agg(o.value ORDER BY o.ord)
               FROM jsonb_array_elements_text(q.options) WITH ORDINALITY AS o(value, ord))
                                    as "options!: Vec<String>",
            a.selected_option_index as "selected_option_index",
            q.correct_option_index  as "correct_option_index!",
            a.is_correct            as "is_correct",
            q.explanation           as "explanation!"
        FROM exam_attempt_answers a
        JOIN question_pool  q  ON q.id = a.question_id
        JOIN exam_attempts  ea ON ea.id = a.attempt_id
        WHERE a.attempt_id = $2
          AND ea.student_id = $1
          AND ea.status <> 'in_progress'
        ORDER BY a.question_seq ASC
        "#,
        student_id.into_uuid(),
        attempt_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Save-as-you-go, idempotent: set the selected option for each
/// `question_seq`, leaving every other row untouched.
///
/// One statement over two unnested arrays, so a 10-question save is one round
/// trip. Writing the same answers twice produces the same rows, which is what
/// lets the client retry a dropped save without reasoning about ordering.
///
/// Returns the number of rows actually updated. A `question_seq` that is not in
/// this attempt updates nothing — it cannot bleed into another attempt, because
/// `attempt_id` is the leading predicate — so a caller comparing the count with
/// the input length detects a bad payload without a second query.
///
/// The status predicate means a submitted attempt silently updates nothing; the
/// handler checks the status itself to answer `409`, and this is the backstop
/// that makes a missed check harmless rather than a re-write of a graded paper.
pub async fn save_answers(
    pool: &PgPool,
    student_id: StudentId,
    attempt_id: ExamAttemptId,
    question_seqs: &[i16],
    selected_option_indices: &[Option<i16>],
) -> Result<u64> {
    let result = sqlx::query!(
        r#"
        UPDATE exam_attempt_answers a
        SET selected_option_index = r.selected_option_index
        FROM unnest($3::smallint[], $4::smallint[])
             AS r(question_seq, selected_option_index)
        WHERE a.attempt_id = $2
          AND a.question_seq = r.question_seq
          AND EXISTS (
            SELECT 1 FROM exam_attempts ea
            WHERE ea.id = $2 AND ea.student_id = $1 AND ea.status = 'in_progress'
          )
        "#,
        student_id.into_uuid(),
        attempt_id.into_uuid(),
        question_seqs,
        selected_option_indices as &[Option<i16>]
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;

    Ok(result.rows_affected())
}

/// Grade one attempt in a single transaction and return the outcome.
///
/// Marking is server-side and happens exactly once. The `exam_attempts` update
/// carries `status = 'in_progress'` in its own `WHERE` clause, so two
/// concurrent submits cannot both grade: the second updates no row and gets
/// `Ok(None)`, which the handler answers `409` — a re-grade, not a second
/// opinion, is what that would otherwise produce.
///
/// `score` is `max_score * correct / total`, computed in SQL from the stored
/// answer key. Nothing the client sends participates in the arithmetic.
/// An unanswered question is simply incorrect — `selected_option_index IS NULL`
/// is never equal to the key — so an abandoned paper scores what it earned.
pub async fn grade(
    pool: &PgPool,
    student_id: StudentId,
    attempt_id: ExamAttemptId,
) -> Result<Option<GradeOutcome>> {
    let mut tx = pool.begin().await.map_err(Error::from_sqlx)?;

    // Ownership and liveness in one round trip. `FOR UPDATE` on the attempt
    // row serialises two concurrent submits behind each other rather than
    // letting both read `in_progress`.
    let attempt = sqlx::query!(
        r#"
        SELECT ea.exam_id as "exam_id!: ExamId", ea.max_score as "max_score!"
        FROM exam_attempts ea
        WHERE ea.id = $2 AND ea.student_id = $1 AND ea.status = 'in_progress'
        FOR UPDATE
        "#,
        student_id.into_uuid(),
        attempt_id.into_uuid()
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(Error::from_sqlx)?;

    let Some(attempt) = attempt else {
        return Ok(None);
    };

    // Mark every row from the stored key.
    sqlx::query!(
        r#"
        UPDATE exam_attempt_answers a
        SET is_correct = (a.selected_option_index IS NOT NULL
                          AND a.selected_option_index = q.correct_option_index)
        FROM question_pool q
        WHERE q.id = a.question_id AND a.attempt_id = $1
        "#,
        attempt_id.into_uuid()
    )
    .execute(&mut *tx)
    .await
    .map_err(Error::from_sqlx)?;

    let tally = sqlx::query!(
        r#"
        SELECT count(*) as "total!",
               count(*) FILTER (WHERE a.is_correct) as "correct!"
        FROM exam_attempt_answers a
        WHERE a.attempt_id = $1
        "#,
        attempt_id.into_uuid()
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(Error::from_sqlx)?;

    // A paper with no questions cannot be scored out of anything; zero rather
    // than a division by zero. `insert_paper` makes this unreachable for an
    // attempt started through the normal path.
    let score = if tally.total > 0 {
        attempt.max_score * (tally.correct as f64) / (tally.total as f64)
    } else {
        0.0
    };

    let submitted = sqlx::query!(
        r#"
        UPDATE exam_attempts
        SET score = $2, status = 'graded', submitted_at = now()
        WHERE id = $1 AND status = 'in_progress'
        RETURNING submitted_at as "submitted_at!"
        "#,
        attempt_id.into_uuid(),
        score
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(Error::from_sqlx)?;

    let Some(submitted) = submitted else {
        return Ok(None);
    };

    let weak_topics = sqlx::query_scalar!(
        r#"
        SELECT DISTINCT q.topic as "topic!"
        FROM exam_attempt_answers a
        JOIN question_pool q ON q.id = a.question_id
        WHERE a.attempt_id = $1 AND a.is_correct IS NOT TRUE
        ORDER BY q.topic ASC
        "#,
        attempt_id.into_uuid()
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(Error::from_sqlx)?;

    tx.commit().await.map_err(Error::from_sqlx)?;

    Ok(Some(GradeOutcome {
        attempt_id,
        exam_id: attempt.exam_id,
        total_questions: tally.total,
        correct_answers: tally.correct,
        score,
        max_score: attempt.max_score,
        submitted_at: submitted.submitted_at,
        weak_topics,
    }))
}

/// Distinct topics this student got wrong in one already-graded attempt — what
/// a dashboard score card's weak-area badges render.
///
/// Scoped to the student and to a non-`in_progress` attempt for the same reason
/// [`graded_paper`] is: before submission, which questions are wrong is part of
/// the answer key.
pub async fn weak_topics_for_attempt(
    pool: &PgPool,
    student_id: StudentId,
    attempt_id: ExamAttemptId,
) -> Result<Vec<String>> {
    sqlx::query_scalar!(
        r#"
        SELECT DISTINCT q.topic as "topic!"
        FROM exam_attempt_answers a
        JOIN question_pool  q  ON q.id = a.question_id
        JOIN exam_attempts  ea ON ea.id = a.attempt_id
        WHERE a.attempt_id = $2
          AND ea.student_id = $1
          AND ea.status <> 'in_progress'
          AND a.is_correct IS FALSE
        ORDER BY q.topic ASC
        "#,
        student_id.into_uuid(),
        attempt_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// The student's live attempt at one exam, if any.
///
/// `POST /student/exams/{id}/attempts` answers `409 EXAM_ATTEMPT_ACTIVE` with
/// this id rather than starting a second paper, so a reload during an exam
/// resumes instead of re-sampling.
pub async fn active_attempt(
    pool: &PgPool,
    student_id: StudentId,
    exam_id: ExamId,
) -> Result<Option<ExamAttemptId>> {
    let rec = sqlx::query!(
        r#"
        SELECT id as "id: ExamAttemptId"
        FROM exam_attempts
        WHERE student_id = $1 AND exam_id = $2 AND status = 'in_progress'
        ORDER BY started_at DESC, attempt_no DESC
        LIMIT 1
        "#,
        student_id.into_uuid(),
        exam_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)?;

    Ok(rec.map(|r| r.id))
}

/// A started attempt: the id and the fields the runner needs back immediately.
#[derive(Debug, Clone)]
pub struct StartedAttempt {
    pub id: ExamAttemptId,
    pub attempt_no: i16,
    pub max_score: f64,
    pub started_at: DateTime<Utc>,
    pub status: ExamAttemptStatus,
}

/// Start an attempt: insert the `exam_attempts` row and its sampled paper in
/// **one transaction**.
///
/// `attempt_no` is `max + 1` computed inside the same statement, so two
/// simultaneous starts cannot both claim the same number — the
/// `UNIQUE (exam_id, student_id, attempt_no)` index is what resolves the race,
/// and the loser surfaces as [`Error::UniqueViolation`] for the handler to
/// retry or reject rather than as two attempts sharing a number.
///
/// `max_score` is snapshotted by the caller from the exam row: re-weighting an
/// exam later must not restate a score a student has already been shown.
pub async fn start_attempt(
    pool: &PgPool,
    student_id: StudentId,
    exam_id: ExamId,
    max_score: f64,
    question_ids: &[uuid::Uuid],
) -> Result<StartedAttempt> {
    let mut tx = pool.begin().await.map_err(Error::from_sqlx)?;

    let attempt = sqlx::query_as!(
        StartedAttempt,
        r#"
        INSERT INTO exam_attempts (exam_id, student_id, attempt_no, max_score)
        SELECT $2, $1,
               COALESCE(max(ea.attempt_no), 0) + 1,
               $3
        FROM exam_attempts ea
        WHERE ea.exam_id = $2 AND ea.student_id = $1
        RETURNING id         as "id!: ExamAttemptId",
                  attempt_no as "attempt_no!",
                  max_score  as "max_score!",
                  started_at as "started_at!",
                  status     as "status!: ExamAttemptStatus"
        "#,
        student_id.into_uuid(),
        exam_id.into_uuid(),
        max_score
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(Error::from_sqlx)?;

    insert_paper(&mut *tx, attempt.id, question_ids).await?;

    tx.commit().await.map_err(Error::from_sqlx)?;

    Ok(attempt)
}
