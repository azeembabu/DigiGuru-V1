//! Reads and writes for `unit_assessments` — the tutor's verdict on one
//! student's conversational performance in one unit sitting.
//!
//! # Not an exam result
//!
//! These rows are judgement about *conversation*, produced by the model at the
//! end of a session. `exam_attempts` holds marks computed in SQL from a stored
//! answer key. The two are never mixed in a query and never summed into one
//! figure: a student must be able to tell which of their numbers came from a
//! marked paper and which from a conversation.
//!
//! # The evidence travels with the verdict
//!
//! Every row stores the counters the model was given alongside the prose it
//! wrote. A student asking "why three stars?" is answered with the turns,
//! questions and minutes behind it, not with the rating repeated louder.

use chrono::{DateTime, Utc};
use dg_core::{BlockId, DocumentId, SessionId, StudentId};
use sqlx::PgPool;

use crate::error::{Error, Result};

/// A stored assessment, as both the student's page and the admin's read it.
#[derive(Debug, Clone)]
pub struct UnitAssessmentRow {
    pub id: uuid::Uuid,
    pub document_id: DocumentId,
    pub block_id: BlockId,
    pub session_id: Option<SessionId>,
    pub stars: i16,
    pub mark: f64,
    pub trophy: Option<String>,
    pub summary: String,
    /// JSONB arrays of strings, passed through as-is — the shape the model
    /// returned and the shape the API sends, with no array-literal round trip.
    pub strengths: serde_json::Value,
    pub improvements: serde_json::Value,
    pub student_turns: i32,
    pub questions_asked: i32,
    pub comprehension_passed: i32,
    pub comprehension_failed: i32,
    pub active_voice_ms: i64,
    pub created_at: DateTime<Utc>,
}

/// What the gateway hands over after a session, for one insert.
#[derive(Debug, Clone)]
pub struct NewUnitAssessment {
    pub student_id: StudentId,
    pub document_id: DocumentId,
    pub block_id: BlockId,
    pub session_id: SessionId,
    pub stars: i16,
    pub mark: f64,
    pub trophy: Option<String>,
    pub summary: String,
    pub strengths: Vec<String>,
    pub improvements: Vec<String>,
    pub student_turns: i32,
    pub questions_asked: i32,
    pub comprehension_passed: i32,
    pub comprehension_failed: i32,
    pub active_voice_ms: i64,
}

/// Records one sitting's verdict.
///
/// Upsert on `session_id`: the assessment job may be retried, and a retry must
/// replace its own verdict rather than leave two opinions on one conversation.
pub async fn insert(pool: &PgPool, new: &NewUnitAssessment) -> Result<uuid::Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO unit_assessments (
            student_id, document_id, block_id, session_id,
            stars, mark, trophy, summary, strengths, improvements,
            student_turns, questions_asked, comprehension_passed,
            comprehension_failed, active_voice_ms
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        ON CONFLICT (session_id) DO UPDATE SET
            stars = EXCLUDED.stars,
            mark = EXCLUDED.mark,
            trophy = EXCLUDED.trophy,
            summary = EXCLUDED.summary,
            strengths = EXCLUDED.strengths,
            improvements = EXCLUDED.improvements,
            student_turns = EXCLUDED.student_turns,
            questions_asked = EXCLUDED.questions_asked,
            comprehension_passed = EXCLUDED.comprehension_passed,
            comprehension_failed = EXCLUDED.comprehension_failed,
            active_voice_ms = EXCLUDED.active_voice_ms
        RETURNING id as "id!"
        "#,
        new.student_id.into_uuid(),
        new.document_id.into_uuid(),
        new.block_id.into_uuid(),
        new.session_id.into_uuid(),
        new.stars,
        new.mark,
        new.trophy.as_deref(),
        new.summary,
        serde_json::json!(new.strengths),
        serde_json::json!(new.improvements),
        new.student_turns,
        new.questions_asked,
        new.comprehension_passed,
        new.comprehension_failed,
        new.active_voice_ms,
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(row.id)
}

/// One unit's line on the assessment page.
///
/// A unit the student has opened but never been assessed on still appears —
/// with `latest` absent — because "you studied this and have no verdict yet" is
/// a different and more useful thing to show than nothing at all.
#[derive(Debug, Clone)]
pub struct UnitProgress {
    pub document_id: DocumentId,
    pub unit_title: String,
    pub block_id: BlockId,
    pub block_no: i16,
    pub block_title: String,
    pub course_id: dg_core::CourseId,
    pub course_code: String,
    pub course_name: String,
    pub semester_number: i16,
    /// Whether the unit has been ingested. A unit that is not `embedded` cannot
    /// be taught, and a student looking at an empty row deserves to know which
    /// of the two reasons applies.
    pub is_ready: bool,
    pub sessions_total: i64,
    pub active_voice_ms: i64,
    /// When the student last opened this unit — the "usage date & time".
    pub last_studied_at: Option<DateTime<Utc>>,
    pub assessments_count: i64,
    // The latest verdict, flattened. All `None` together when there is none.
    pub latest_stars: Option<i16>,
    pub latest_mark: Option<f64>,
    pub latest_trophy: Option<String>,
    pub latest_summary: Option<String>,
    pub latest_strengths: Option<serde_json::Value>,
    pub latest_improvements: Option<serde_json::Value>,
    pub latest_at: Option<DateTime<Utc>>,
    pub latest_student_turns: Option<i32>,
    pub latest_questions_asked: Option<i32>,
    pub latest_comprehension_passed: Option<i32>,
    pub latest_comprehension_failed: Option<i32>,
    /// The student's best mark across every sitting of this unit, so improving
    /// on a bad session is visible rather than overwritten by it.
    pub best_mark: Option<f64>,
}

/// Every unit of the student's actively enrolled courses, with study activity
/// and the latest assessment.
///
/// Starts from `student_courses` with the caller's id bound, `status =
/// 'active'`, exactly as the catalogue and block-performance queries do: a unit
/// outside the student's own enrolments selects no row, so there is no result
/// set for a later predicate to widen.
///
/// `scope_program_ids` is the admin path's scope and is `None` for the
/// student's own read — the student *is* the scope there.
pub async fn units_for_student(
    pool: &PgPool,
    student_id: StudentId,
    scope_program_ids: Option<&[uuid::Uuid]>,
) -> Result<Vec<UnitProgress>> {
    sqlx::query_as!(
        UnitProgress,
        r#"
        SELECT
            d.id     as "document_id!: DocumentId",
            d.title  as "unit_title!",
            b.id     as "block_id!: BlockId",
            b.block_no as "block_no!",
            b.title  as "block_title!",
            c.id     as "course_id!: dg_core::CourseId",
            c.code   as "course_code!",
            c.name   as "course_name!",
            sm.semester_number as "semester_number!",
            (d.status = 'embedded') as "is_ready!",
            (SELECT count(*) FROM learning_sessions ls
              WHERE ls.student_id = sc.student_id
                AND ls.document_id = d.id)                      as "sessions_total!",
            COALESCE((SELECT sum(ls.active_voice_ms) FROM learning_sessions ls
              WHERE ls.student_id = sc.student_id
                AND ls.document_id = d.id), 0)::bigint          as "active_voice_ms!",
            (SELECT max(ls.started_at) FROM learning_sessions ls
              WHERE ls.student_id = sc.student_id
                AND ls.document_id = d.id)                      as "last_studied_at",
            (SELECT count(*) FROM unit_assessments ua
              WHERE ua.student_id = sc.student_id
                AND ua.document_id = d.id)                      as "assessments_count!",
            latest.stars                as "latest_stars?",
            latest.mark                 as "latest_mark?",
            latest.trophy               as "latest_trophy?",
            latest.summary              as "latest_summary?",
            latest.strengths            as "latest_strengths?",
            latest.improvements         as "latest_improvements?",
            latest.created_at           as "latest_at?",
            latest.student_turns        as "latest_student_turns?",
            latest.questions_asked      as "latest_questions_asked?",
            latest.comprehension_passed as "latest_comprehension_passed?",
            latest.comprehension_failed as "latest_comprehension_failed?",
            (SELECT max(ua.mark) FROM unit_assessments ua
              WHERE ua.student_id = sc.student_id
                AND ua.document_id = d.id)                      as "best_mark"
        FROM student_courses sc
        JOIN students  st ON st.id = sc.student_id
        JOIN courses   c  ON c.id  = sc.course_id
        JOIN semesters sm ON sm.id = c.semester_id
        JOIN blocks    b  ON b.course_id = c.id AND b.status = 'active'
        JOIN documents d  ON d.block_id = b.id
        -- The most recent verdict for this unit. LATERAL rather than a window
        -- function so the row limit is applied per unit by the planner instead
        -- of ranking every assessment the student has ever received.
        LEFT JOIN LATERAL (
            SELECT ua.* FROM unit_assessments ua
            WHERE ua.student_id = sc.student_id AND ua.document_id = d.id
            ORDER BY ua.created_at DESC
            LIMIT 1
        ) latest ON true
        WHERE sc.student_id = $1
          AND sc.status = 'active'
          AND ($2::uuid[] IS NULL OR st.program_id = ANY($2))
        ORDER BY sm.semester_number, c.code, b.block_no, d.title
        "#,
        student_id.into_uuid(),
        scope_program_ids
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Every assessment for one unit, newest first — the history behind the latest
/// verdict, so a student can see a second sitting went better than the first.
pub async fn history_for_unit(
    pool: &PgPool,
    student_id: StudentId,
    document_id: DocumentId,
    limit: i64,
) -> Result<Vec<UnitAssessmentRow>> {
    sqlx::query_as!(
        UnitAssessmentRow,
        r#"
        SELECT
            id as "id!",
            document_id as "document_id!: DocumentId",
            block_id    as "block_id!: BlockId",
            session_id  as "session_id?: SessionId",
            stars as "stars!",
            mark  as "mark!",
            trophy,
            summary as "summary!",
            strengths    as "strengths!",
            improvements as "improvements!",
            student_turns        as "student_turns!",
            questions_asked      as "questions_asked!",
            comprehension_passed as "comprehension_passed!",
            comprehension_failed as "comprehension_failed!",
            active_voice_ms      as "active_voice_ms!",
            created_at as "created_at!"
        FROM unit_assessments
        WHERE student_id = $1 AND document_id = $2
        ORDER BY created_at DESC
        LIMIT $3
        "#,
        student_id.into_uuid(),
        document_id.into_uuid(),
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// The most recent assessments across every unit, for the dashboard card.
///
/// Newest first and deliberately small: the dashboard shows the last few, and
/// the full record lives on the assessment page.
pub async fn recent_for_student(
    pool: &PgPool,
    student_id: StudentId,
    limit: i64,
) -> Result<Vec<RecentAssessment>> {
    sqlx::query_as!(
        RecentAssessment,
        r#"
        SELECT
            ua.id          as "id!",
            ua.document_id as "document_id!: DocumentId",
            d.title        as "unit_title!",
            b.block_no     as "block_no!",
            c.code         as "course_code!",
            ua.stars       as "stars!",
            ua.mark        as "mark!",
            ua.trophy,
            ua.summary     as "summary!",
            ua.created_at  as "created_at!"
        FROM unit_assessments ua
        JOIN documents d ON d.id = ua.document_id
        JOIN blocks    b ON b.id = ua.block_id
        JOIN courses   c ON c.id = b.course_id
        WHERE ua.student_id = $1
        ORDER BY ua.created_at DESC
        LIMIT $2
        "#,
        student_id.into_uuid(),
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// One dashboard card.
#[derive(Debug, Clone)]
pub struct RecentAssessment {
    pub id: uuid::Uuid,
    pub document_id: DocumentId,
    pub unit_title: String,
    pub block_no: i16,
    pub course_code: String,
    pub stars: i16,
    pub mark: f64,
    pub trophy: Option<String>,
    pub summary: String,
    pub created_at: DateTime<Utc>,
}
