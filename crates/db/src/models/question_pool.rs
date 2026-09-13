//! `question_pool` — the MCQ bank the exam module samples its papers from
//! (`migrations/0006_question_pool_and_flashcards.sql`,
//! `migrations/0007_question_pool_metadata.sql`).
//!
//! A pool is built per **course**; `block_id` is the optional unit/module
//! pointer (`migrations/0010_question_pool_course_scope.sql`). Program and
//! semester are still not columns — both are reached through
//! `question_pool -> courses`, which is also the one hop
//! [`program_id_for_question`] now walks, so an admin handler can resolve scope
//! *before* its capability check.
//!
//! Sampling is **course-scoped**: a paper draws from the active pool of the
//! exam's own course for the exam's `assessment_type` — see
//! [`sample_for_course`]. The exam's block supplies the course and the exam row
//! the paper size and timer; the module link never narrows the pool, because
//! most questions will not have one.
//!
//! Two row shapes exist deliberately, and the split is the security property
//! that keeps the answer key off the wire before submission:
//! [`Question`] carries `correct_option_index` and `explanation` and is
//! **admin-only**; the student-facing paper is [`crate::models::exam_papers`],
//! whose unsubmitted row type has no such fields to serialise. A student
//! handler that never sees a `Question` cannot leak one.

use chrono::{DateTime, Utc};
use dg_core::{
    AssessmentType, BlockId, CourseId, DifficultyLevel, ProgramId, QuestionId, QuestionStatus,
    SemesterId, UserId,
};
use sqlx::PgPool;

use crate::error::{Error, Result};

/// One authored question, answer key included. Admin-facing only.
///
/// `options` is always exactly four entries (A/B/C/D) and `correct_option_index`
/// always indexes one of them — both enforced by CHECK constraints, so the
/// invariant holds for rows written by any path, not only by this module.
#[derive(Debug, Clone)]
pub struct Question {
    pub id: QuestionId,
    /// The optional unit/module link. `None` is the normal case for a
    /// course-wide question, not a missing value.
    pub block_id: Option<BlockId>,
    pub block_no: Option<i16>,
    pub block_title: Option<String>,
    pub course_id: CourseId,
    pub course_code: String,
    pub semester_id: SemesterId,
    pub program_id: ProgramId,
    pub topic: String,
    pub question_text: String,
    pub options: Vec<String>,
    pub correct_option_index: i16,
    pub explanation: String,
    pub assessment_type: AssessmentType,
    pub difficulty_level: DifficultyLevel,
    pub status: QuestionStatus,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
}

/// Insert one question.
///
/// `options` is taken as a slice of exactly four strings and serialised to
/// JSONB here, so no caller has to know the storage shape; a wrong length is
/// rejected by the table's CHECK rather than silently stored.
#[allow(clippy::too_many_arguments)]
pub async fn create(
    pool: &PgPool,
    course_id: CourseId,
    block_id: Option<BlockId>,
    topic: &str,
    question_text: &str,
    options: &[String],
    correct_option_index: i16,
    explanation: &str,
    assessment_type: AssessmentType,
    difficulty_level: DifficultyLevel,
    created_by: UserId,
) -> Result<Question> {
    let options = serde_json::Value::from(options.to_vec());

    sqlx::query_as!(
        Question,
        r#"
        WITH inserted AS (
            INSERT INTO question_pool (course_id, block_id, topic, question_text, options,
                                       correct_option_index, explanation,
                                       assessment_type, difficulty_level, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7,
                    $8::text::assessment_type, $9::text::difficulty_level, $10)
            RETURNING id, course_id, block_id, topic, question_text, options,
                      correct_option_index,
                      explanation, assessment_type, difficulty_level, status,
                      created_by, created_at
        )
        SELECT
            q.id               as "id!: QuestionId",
            q.block_id         as "block_id?: BlockId",
            b.block_no         as "block_no?",
            b.title            as "block_title?",
            q.course_id        as "course_id!: CourseId",
            c.code             as "course_code!",
            c.semester_id      as "semester_id!: SemesterId",
            c.program_id       as "program_id!: ProgramId",
            q.topic            as "topic!",
            q.question_text    as "question_text!",
            -- `WITH ORDINALITY` and an explicit `ORDER BY` rather than a bare
            -- `array_agg`: option order *is* the A/B/C/D labelling the student
            -- answers against, and `jsonb_array_elements_text` guarantees no
            -- ordering of its own.
            (SELECT array_agg(o.value ORDER BY o.ord)
               FROM jsonb_array_elements_text(q.options) WITH ORDINALITY AS o(value, ord))
                               as "options!: Vec<String>",
            q.correct_option_index as "correct_option_index!",
            q.explanation      as "explanation!",
            q.assessment_type  as "assessment_type!: AssessmentType",
            q.difficulty_level as "difficulty_level!: DifficultyLevel",
            q.status           as "status!: QuestionStatus",
            q.created_by       as "created_by!: UserId",
            q.created_at       as "created_at!"
        FROM inserted q
        JOIN courses c ON c.id = q.course_id
        -- LEFT: the unit/module link is optional, and an inner join here would
        -- silently hide every course-wide question.
        LEFT JOIN blocks b ON b.id = q.block_id
        "#,
        course_id.into_uuid(),
        block_id.map(BlockId::into_uuid),
        topic,
        question_text,
        options,
        correct_option_index,
        explanation,
        assessment_type.as_db_str(),
        difficulty_level.as_db_str(),
        created_by.into_uuid()
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Insert a whole import in **one statement**, all rows or none.
///
/// A half-imported pool is worse than a rejected one: a student could then sit
/// a paper sampled from a partially-validated bank. One `INSERT ... SELECT
/// unnest(...)` is atomic without needing an explicit transaction, and a single
/// bad row (length, answer-key range, missing block) aborts every row with it.
///
/// Every array must be the same length; `unnest` over ragged arrays pads with
/// NULL, which the NOT NULL columns then reject — so a caller mismatch fails
/// loudly rather than importing a truncated pool. `block_ids` is the one
/// genuinely nullable column: the unit/module link is optional.
#[allow(clippy::too_many_arguments)]
pub async fn create_bulk(
    pool: &PgPool,
    course_ids: &[uuid::Uuid],
    block_ids: &[Option<uuid::Uuid>],
    topics: &[String],
    question_texts: &[String],
    options: &[serde_json::Value],
    correct_option_indices: &[i16],
    explanations: &[String],
    assessment_types: &[String],
    difficulty_levels: &[String],
    created_by: UserId,
) -> Result<i64> {
    let inserted = sqlx::query_scalar!(
        r#"
        WITH inserted AS (
            INSERT INTO question_pool (course_id, block_id, topic, question_text, options,
                                       correct_option_index, explanation,
                                       assessment_type, difficulty_level, created_by)
            SELECT r.course_id, r.block_id, r.topic, r.question_text, r.options,
                   r.correct_option_index, r.explanation,
                   r.assessment_type::assessment_type,
                   r.difficulty_level::difficulty_level,
                   $10
            FROM unnest($1::uuid[], $2::uuid[], $3::text[], $4::text[], $5::jsonb[],
                        $6::smallint[], $7::text[], $8::text[], $9::text[])
                 AS r(course_id, block_id, topic, question_text, options,
                      correct_option_index, explanation,
                      assessment_type, difficulty_level)
            RETURNING 1 AS one
        )
        SELECT count(*) as "count!" FROM inserted
        "#,
        course_ids,
        block_ids as &[Option<uuid::Uuid>],
        topics,
        question_texts,
        options,
        correct_option_indices,
        explanations,
        assessment_types,
        difficulty_levels,
        created_by.into_uuid()
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)?;

    Ok(inserted)
}

pub async fn find_by_id(pool: &PgPool, id: QuestionId) -> Result<Option<Question>> {
    sqlx::query_as!(
        Question,
        r#"
        SELECT
            q.id               as "id!: QuestionId",
            q.block_id         as "block_id?: BlockId",
            b.block_no         as "block_no?",
            b.title            as "block_title?",
            q.course_id        as "course_id!: CourseId",
            c.code             as "course_code!",
            c.semester_id      as "semester_id!: SemesterId",
            c.program_id       as "program_id!: ProgramId",
            q.topic            as "topic!",
            q.question_text    as "question_text!",
            -- `WITH ORDINALITY` and an explicit `ORDER BY` rather than a bare
            -- `array_agg`: option order *is* the A/B/C/D labelling the student
            -- answers against, and `jsonb_array_elements_text` guarantees no
            -- ordering of its own.
            (SELECT array_agg(o.value ORDER BY o.ord)
               FROM jsonb_array_elements_text(q.options) WITH ORDINALITY AS o(value, ord))
                               as "options!: Vec<String>",
            q.correct_option_index as "correct_option_index!",
            q.explanation      as "explanation!",
            q.assessment_type  as "assessment_type!: AssessmentType",
            q.difficulty_level as "difficulty_level!: DifficultyLevel",
            q.status           as "status!: QuestionStatus",
            q.created_by       as "created_by!: UserId",
            q.created_at       as "created_at!"
        FROM question_pool q
        JOIN courses c ON c.id = q.course_id
        -- LEFT: the unit/module link is optional, and an inner join here would
        -- silently hide every course-wide question.
        LEFT JOIN blocks b ON b.id = q.block_id
        WHERE q.id = $1
        "#,
        id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Partial update: `None` leaves the column as-is.
///
/// `course_id` is deliberately not editable. Moving a question between courses
/// would move it past the program scope its author was authorized under — the
/// same reason `blocks.course_id` is immutable. `block_id` *is* editable: it
/// only re-files the question within a course it already belongs to, and the
/// gateway still checks the new block is in that course (the rule a CHECK
/// cannot express — see the migration).
#[allow(clippy::too_many_arguments)]
pub async fn update(
    pool: &PgPool,
    id: QuestionId,
    block_id: Option<BlockId>,
    topic: Option<&str>,
    question_text: Option<&str>,
    options: Option<&serde_json::Value>,
    correct_option_index: Option<i16>,
    explanation: Option<&str>,
    assessment_type: Option<AssessmentType>,
    difficulty_level: Option<DifficultyLevel>,
    status: Option<QuestionStatus>,
) -> Result<Option<Question>> {
    sqlx::query_as!(
        Question,
        r#"
        WITH updated AS (
            UPDATE question_pool
            SET block_id             = COALESCE($2, block_id),
                topic                = COALESCE($3, topic),
                question_text        = COALESCE($4, question_text),
                options              = COALESCE($5, options),
                correct_option_index = COALESCE($6, correct_option_index),
                explanation          = COALESCE($7, explanation),
                assessment_type      = COALESCE($8::text::assessment_type, assessment_type),
                difficulty_level     = COALESCE($9::text::difficulty_level, difficulty_level),
                status               = COALESCE($10::text::question_status, status)
            WHERE id = $1
            RETURNING id, course_id, block_id, topic, question_text, options, correct_option_index,
                      explanation, assessment_type, difficulty_level, status,
                      created_by, created_at
        )
        SELECT
            q.id               as "id!: QuestionId",
            q.block_id         as "block_id?: BlockId",
            b.block_no         as "block_no?",
            b.title            as "block_title?",
            q.course_id        as "course_id!: CourseId",
            c.code             as "course_code!",
            c.semester_id      as "semester_id!: SemesterId",
            c.program_id       as "program_id!: ProgramId",
            q.topic            as "topic!",
            q.question_text    as "question_text!",
            -- `WITH ORDINALITY` and an explicit `ORDER BY` rather than a bare
            -- `array_agg`: option order *is* the A/B/C/D labelling the student
            -- answers against, and `jsonb_array_elements_text` guarantees no
            -- ordering of its own.
            (SELECT array_agg(o.value ORDER BY o.ord)
               FROM jsonb_array_elements_text(q.options) WITH ORDINALITY AS o(value, ord))
                               as "options!: Vec<String>",
            q.correct_option_index as "correct_option_index!",
            q.explanation      as "explanation!",
            q.assessment_type  as "assessment_type!: AssessmentType",
            q.difficulty_level as "difficulty_level!: DifficultyLevel",
            q.status           as "status!: QuestionStatus",
            q.created_by       as "created_by!: UserId",
            q.created_at       as "created_at!"
        FROM updated q
        JOIN courses c ON c.id = q.course_id
        -- LEFT: the unit/module link is optional, and an inner join here would
        -- silently hide every course-wide question.
        LEFT JOIN blocks b ON b.id = q.block_id
        "#,
        id.into_uuid(),
        block_id.map(BlockId::into_uuid),
        topic,
        question_text,
        options,
        correct_option_index,
        explanation,
        assessment_type.map(AssessmentType::as_db_str),
        difficulty_level.map(DifficultyLevel::as_db_str),
        status.map(QuestionStatus::as_db_str)
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Filters for the admin list. Every field is optional and `None` means "no
/// filter", so one struct serves the unfiltered list and every drill-down.
#[derive(Debug, Clone, Copy, Default)]
pub struct QuestionFilter<'a> {
    pub program_id: Option<ProgramId>,
    pub semester_id: Option<SemesterId>,
    /// The pool's own course — the primary filter now that a pool is per course.
    pub course_id: Option<CourseId>,
    /// The optional unit/module link. Filtering on it returns only the questions
    /// filed under that module, never the course-wide ones.
    pub block_id: Option<BlockId>,
    pub assessment_type: Option<AssessmentType>,
    pub difficulty_level: Option<DifficultyLevel>,
    pub status: Option<QuestionStatus>,
    /// Case-insensitive substring over `topic` and `question_text`.
    pub q: Option<&'a str>,
    /// Sub-admin scope: when `Some`, only questions whose program is in this
    /// list are visible. Applied in SQL before pagination, so `X-Total-Count`
    /// and the page agree.
    pub scope_program_ids: Option<&'a [uuid::Uuid]>,
}

/// One page of the admin question list, newest first.
pub async fn list(
    pool: &PgPool,
    filter: &QuestionFilter<'_>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Question>> {
    sqlx::query_as!(
        Question,
        r#"
        SELECT
            q.id               as "id!: QuestionId",
            q.block_id         as "block_id?: BlockId",
            b.block_no         as "block_no?",
            b.title            as "block_title?",
            q.course_id        as "course_id!: CourseId",
            c.code             as "course_code!",
            c.semester_id      as "semester_id!: SemesterId",
            c.program_id       as "program_id!: ProgramId",
            q.topic            as "topic!",
            q.question_text    as "question_text!",
            -- `WITH ORDINALITY` and an explicit `ORDER BY` rather than a bare
            -- `array_agg`: option order *is* the A/B/C/D labelling the student
            -- answers against, and `jsonb_array_elements_text` guarantees no
            -- ordering of its own.
            (SELECT array_agg(o.value ORDER BY o.ord)
               FROM jsonb_array_elements_text(q.options) WITH ORDINALITY AS o(value, ord))
                               as "options!: Vec<String>",
            q.correct_option_index as "correct_option_index!",
            q.explanation      as "explanation!",
            q.assessment_type  as "assessment_type!: AssessmentType",
            q.difficulty_level as "difficulty_level!: DifficultyLevel",
            q.status           as "status!: QuestionStatus",
            q.created_by       as "created_by!: UserId",
            q.created_at       as "created_at!"
        FROM question_pool q
        JOIN courses c ON c.id = q.course_id
        -- LEFT: the unit/module link is optional, and an inner join here would
        -- silently hide every course-wide question.
        LEFT JOIN blocks b ON b.id = q.block_id
        WHERE ($1::uuid IS NULL OR c.program_id = $1)
          AND ($2::uuid IS NULL OR c.semester_id = $2)
          AND ($3::uuid IS NULL OR q.course_id = $3)
          AND ($4::uuid IS NULL OR q.block_id = $4)
          AND ($5::text IS NULL OR q.assessment_type = $5::assessment_type)
          AND ($6::text IS NULL OR q.difficulty_level = $6::difficulty_level)
          AND ($7::text IS NULL OR q.status = $7::question_status)
          AND ($8::text IS NULL OR q.topic ILIKE '%' || $8 || '%'
                                OR q.question_text ILIKE '%' || $8 || '%')
          AND ($9::uuid[] IS NULL OR c.program_id = ANY($9))
        ORDER BY q.created_at DESC, q.id DESC
        LIMIT $10 OFFSET $11
        "#,
        filter.program_id.map(ProgramId::into_uuid),
        filter.semester_id.map(SemesterId::into_uuid),
        filter.course_id.map(CourseId::into_uuid),
        filter.block_id.map(BlockId::into_uuid),
        filter.assessment_type.map(AssessmentType::as_db_str),
        filter.difficulty_level.map(DifficultyLevel::as_db_str),
        filter.status.map(QuestionStatus::as_db_str),
        filter.q,
        filter.scope_program_ids,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// `X-Total-Count` for [`list`] — the total *before* pagination, so it repeats
/// every filter and takes no `LIMIT`.
pub async fn count(pool: &PgPool, filter: &QuestionFilter<'_>) -> Result<i64> {
    sqlx::query_scalar!(
        r#"
        SELECT count(*) as "count!"
        FROM question_pool q
        JOIN courses c ON c.id = q.course_id
        -- LEFT: the unit/module link is optional, and an inner join here would
        -- silently hide every course-wide question.
        LEFT JOIN blocks b ON b.id = q.block_id
        WHERE ($1::uuid IS NULL OR c.program_id = $1)
          AND ($2::uuid IS NULL OR c.semester_id = $2)
          AND ($3::uuid IS NULL OR q.course_id = $3)
          AND ($4::uuid IS NULL OR q.block_id = $4)
          AND ($5::text IS NULL OR q.assessment_type = $5::assessment_type)
          AND ($6::text IS NULL OR q.difficulty_level = $6::difficulty_level)
          AND ($7::text IS NULL OR q.status = $7::question_status)
          AND ($8::text IS NULL OR q.topic ILIKE '%' || $8 || '%'
                                OR q.question_text ILIKE '%' || $8 || '%')
          AND ($9::uuid[] IS NULL OR c.program_id = ANY($9))
        "#,
        filter.program_id.map(ProgramId::into_uuid),
        filter.semester_id.map(SemesterId::into_uuid),
        filter.course_id.map(CourseId::into_uuid),
        filter.block_id.map(BlockId::into_uuid),
        filter.assessment_type.map(AssessmentType::as_db_str),
        filter.difficulty_level.map(DifficultyLevel::as_db_str),
        filter.status.map(QuestionStatus::as_db_str),
        filter.q,
        filter.scope_program_ids
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// The `program_id` that owns `question_id`, via `question_pool -> courses` —
/// one hop shorter now the pool is course-scoped. `None` if the question does
/// not exist.
///
/// Mirrors [`crate::models::exams::program_id_for_exam`] and exists for the
/// same reason: the handler resolves the owning program *before* its
/// capability check, so a sub-admin outside the scope cannot tell an existing
/// question from a missing one.
pub async fn program_id_for_question(
    pool: &PgPool,
    question_id: QuestionId,
) -> Result<Option<ProgramId>> {
    let rec = sqlx::query!(
        r#"
        SELECT c.program_id as "program_id: ProgramId"
        FROM question_pool q
        JOIN courses c ON c.id = q.course_id
        -- LEFT: the unit/module link is optional, and an inner join here would
        -- silently hide every course-wide question.
        LEFT JOIN blocks b ON b.id = q.block_id
        WHERE q.id = $1
        "#,
        question_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)?;

    Ok(rec.map(|r| r.program_id))
}

/// One sampled question: its id and the topic the weak-area roll-up needs.
///
/// Nothing else is projected on purpose. The sampler's job is to choose the
/// paper; the text and options are read back through
/// [`crate::models::exam_papers`], which is the type that cannot carry an
/// answer key.
#[derive(Debug, Clone)]
pub struct SampledQuestion {
    pub id: QuestionId,
    pub topic: String,
}
/// How many active questions the **course's** pool holds for `assessment_type`.
///
/// The handler calls this before starting an attempt: a pool smaller than the
/// exam's `question_count` is a `422`, not a short paper, because a paper the
/// student cannot be scored out of `max_score` on is not an exam.
pub async fn count_available_for_course(
    pool: &PgPool,
    course_id: CourseId,
    assessment_type: AssessmentType,
) -> Result<i64> {
    sqlx::query_scalar!(
        r#"
        SELECT count(*) as "count!"
        FROM question_pool q
        WHERE q.status = 'active'
          AND q.assessment_type = $2::text::assessment_type
          AND q.course_id = $1
        "#,
        course_id.into_uuid(),
        assessment_type.as_db_str()
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Draw `n` distinct active questions for one attempt from **one course's**
/// pool.
///
/// Course-scoped per the owner's Section 3 — "strictly from the pre-populated
/// Question Pool of the selected Course". The caller reaches that course from
/// the exam (`exams -> blocks -> course_id`); the question's own optional
/// unit/module link never narrows the draw, because most questions will not have
/// one and a module filter would silently shrink the pool.
///
/// A single statement with `ORDER BY random() LIMIT $n`: the selection *and* its
/// ordering are decided by the server in one round trip, so two attempts at the
/// same exam get a different paper and nothing a client sends influences either.
///
/// Returns fewer than `n` rows when the pool is short; callers must check the
/// length (or call [`count_available_for_course`] first) rather than assume.
pub async fn sample_for_course(
    pool: &PgPool,
    course_id: CourseId,
    assessment_type: AssessmentType,
    n: i64,
) -> Result<Vec<SampledQuestion>> {
    sqlx::query_as!(
        SampledQuestion,
        r#"
        SELECT q.id as "id!: QuestionId", q.topic as "topic!"
        FROM question_pool q
        WHERE q.status = 'active'
          AND q.assessment_type = $2::text::assessment_type
          AND q.course_id = $1
        ORDER BY random()
        LIMIT $3
        "#,
        course_id.into_uuid(),
        assessment_type.as_db_str(),
        n
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// How many active questions each course's pool holds, for the admin screen
/// that has to show which courses are still empty (A2.5).
#[derive(Debug, Clone)]
pub struct CoursePoolCount {
    pub course_id: CourseId,
    pub course_code: String,
    pub course_name: String,
    pub semester_id: SemesterId,
    pub semester_number: i16,
    pub active_questions: i64,
}

/// Active question counts for every course of one program, including the
/// courses with **no** questions at all — those are the whole point of the
/// screen, so this is a `LEFT JOIN` from `courses`, not a `GROUP BY` over the
/// pool.
pub async fn pool_counts_by_course(
    pool: &PgPool,
    program_id: ProgramId,
) -> Result<Vec<CoursePoolCount>> {
    sqlx::query_as!(
        CoursePoolCount,
        r#"
        SELECT
            c.id     as "course_id!: CourseId",
            c.code   as "course_code!",
            c.name   as "course_name!",
            c.semester_id as "semester_id!: SemesterId",
            s.semester_number as "semester_number!",
            count(q.id) as "active_questions!"
        FROM courses c
        JOIN semesters s ON s.id = c.semester_id
        LEFT JOIN question_pool q ON q.course_id = c.id AND q.status = 'active'
        WHERE c.program_id = $1
        GROUP BY c.id, c.code, c.name, c.semester_id, s.semester_number
        ORDER BY s.semester_number ASC, c.code ASC
        "#,
        program_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}
