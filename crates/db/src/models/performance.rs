//! Per-block performance reads, and the student's own remark against a block.
//!
//! # There is no performance store
//!
//! Every figure here is **computed at read time** from rows other paths
//! already write: `exam_attempts` (what was scored), `learning_sessions` (what
//! was studied) and `exam_attempt_answers` joined to `question_pool` (which
//! topics were missed). Nothing is cached, denormalised or mirrored.
//!
//! That is the same argument `student_reports` makes and it matters more here,
//! because this data is read by two different audiences: the student on their
//! assessment page and the admin on that student's record. A stored
//! "performance level" would be a second copy that could disagree with the
//! attempts it was derived from, and the two screens would then disagree with
//! each other. Deriving it twice from one source cannot drift.
//!
//! # A block is the unit of assessment
//!
//! Rows are per **block**, because that is where assessment actually attaches:
//! `exams.block_id` is NOT NULL and `learning_sessions.block_id` is too. An
//! uploaded unit (a `documents` row) carries no score of its own, so a
//! per-unit performance figure would have to be invented rather than measured.
//!
//! # `Option` means "not measured", never zero
//!
//! Averages and percentages are `Option<f64>` throughout. A block the student
//! has never been examined on returns `None`, which the level/star helpers in
//! `dg_core::performance` turn into "not assessed" — as distinct from a
//! measured zero, which is a real and much worse result.

use chrono::{DateTime, Utc};
use dg_core::{BlockId, CourseId, StudentId};
use sqlx::PgPool;

use crate::error::{Error, Result};

/// One block's line on the assessment page.
///
/// Denormalised across course and block so a whole page renders from one
/// query — a per-row name lookup would be an N+1 in the browser, which
/// `api-conventions.md` rules out for every drill-down list.
#[derive(Debug, Clone)]
pub struct BlockPerformance {
    pub block_id: BlockId,
    pub block_no: i16,
    pub block_title: String,
    pub course_id: CourseId,
    pub course_code: String,
    pub course_name: String,
    pub semester_number: i16,
    /// Exams published on this block, whether or not the student sat them.
    /// Without it a block with no exam and a block the student skipped look
    /// identical on the page.
    pub exams_available: i64,
    pub attempts_total: i64,
    pub attempts_graded: i64,
    /// `None` until something is graded.
    pub average_percentage: Option<f64>,
    pub best_percentage: Option<f64>,
    /// Tutoring sessions the student has opened on this block.
    pub sessions_total: i64,
    pub sessions_completed: i64,
    /// NN-3 active voice across those sessions. A real measured `0` when the
    /// student has opened the block but never spoken.
    pub active_voice_ms: i64,
    pub last_studied_at: Option<DateTime<Utc>>,
    /// The student's own words, or `None` if they have not written any.
    pub remark: Option<String>,
    pub remark_updated_at: Option<DateTime<Utc>>,
}

/// Every block of the student's **actively enrolled** courses, with what they
/// have scored and studied on each.
///
/// Starts from `student_courses` with the caller's id bound, exactly as the
/// student catalogue routes do: a block outside the student's active
/// enrolments selects no row, so there is no result set for a later predicate
/// to widen. A `dropped` or `completed` enrolment is a historical record, not a
/// licence to keep being assessed on it.
///
/// Blocks with no attempt and no session are **included**, not filtered out.
/// The page's job is to show the student where they stand across the course,
/// and a block they have not started is exactly the thing they most need to
/// see.
pub async fn blocks_for_student(
    pool: &PgPool,
    student_id: StudentId,
    course_id: Option<CourseId>,
) -> Result<Vec<BlockPerformance>> {
    sqlx::query_as!(
        BlockPerformance,
        r#"
        SELECT
            b.id    as "block_id!: BlockId",
            b.block_no as "block_no!",
            b.title as "block_title!",
            c.id    as "course_id!: CourseId",
            c.code  as "course_code!",
            c.name  as "course_name!",
            sm.semester_number as "semester_number!",
            (SELECT count(*) FROM exams e
              WHERE e.block_id = b.id AND e.status = 'published')  as "exams_available!",
            (SELECT count(*) FROM exam_attempts ea
               JOIN exams e ON e.id = ea.exam_id
              WHERE e.block_id = b.id AND ea.student_id = sc.student_id) as "attempts_total!",
            (SELECT count(*) FROM exam_attempts ea
               JOIN exams e ON e.id = ea.exam_id
              WHERE e.block_id = b.id AND ea.student_id = sc.student_id
                AND ea.status = 'graded')                          as "attempts_graded!",
            -- Averaged over attempts that were actually marked. An
            -- `in_progress` attempt has no score and must not drag an average
            -- toward zero.
            (SELECT avg(100.0 * ea.score / ea.max_score) FROM exam_attempts ea
               JOIN exams e ON e.id = ea.exam_id
              WHERE e.block_id = b.id AND ea.student_id = sc.student_id
                AND ea.score IS NOT NULL)                          as "average_percentage",
            (SELECT max(100.0 * ea.score / ea.max_score) FROM exam_attempts ea
               JOIN exams e ON e.id = ea.exam_id
              WHERE e.block_id = b.id AND ea.student_id = sc.student_id
                AND ea.score IS NOT NULL)                          as "best_percentage",
            (SELECT count(*) FROM learning_sessions ls
              WHERE ls.block_id = b.id AND ls.student_id = sc.student_id) as "sessions_total!",
            (SELECT count(*) FROM learning_sessions ls
              WHERE ls.block_id = b.id AND ls.student_id = sc.student_id
                AND ls.status = 'completed')                       as "sessions_completed!",
            COALESCE((SELECT sum(ls.active_voice_ms) FROM learning_sessions ls
              WHERE ls.block_id = b.id AND ls.student_id = sc.student_id), 0)::bigint
                                                                   as "active_voice_ms!",
            (SELECT max(ls.started_at) FROM learning_sessions ls
              WHERE ls.block_id = b.id AND ls.student_id = sc.student_id) as "last_studied_at",
            -- `?` forced, not inferred. `student_block_remarks.remark` is
            -- TEXT NOT NULL in its own table, so sqlx types it as non-null and
            -- then fails to decode the NULL a LEFT JOIN produces for a block
            -- the student has not written about — which is most of them.
            r.remark     as "remark?",
            r.updated_at as "remark_updated_at?"
        FROM student_courses sc
        JOIN courses   c  ON c.id  = sc.course_id
        JOIN semesters sm ON sm.id = c.semester_id
        JOIN blocks    b  ON b.course_id = c.id AND b.status = 'active'
        LEFT JOIN student_block_remarks r
               ON r.student_id = sc.student_id AND r.block_id = b.id
        WHERE sc.student_id = $1
          AND sc.status = 'active'
          AND ($2::uuid IS NULL OR c.id = $2)
        ORDER BY sm.semester_number, c.code, b.block_no
        "#,
        student_id.into_uuid(),
        course_id.map(|c| c.into_uuid())
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Every block of one student's courses, for an admin, **scoped**.
///
/// Separate from [`blocks_for_student`] rather than sharing it with an extra
/// parameter: the scope predicate is what decides whether an admin may see
/// this student at all, and a single function with an `Option` scope is one
/// `None` away from serving a sub-admin the unscoped result. `Some(&[])` — a
/// sub-admin with no scopes — correctly yields nothing.
pub async fn blocks_for_student_scoped(
    pool: &PgPool,
    student_id: StudentId,
    scope_program_ids: Option<&[uuid::Uuid]>,
) -> Result<Vec<BlockPerformance>> {
    sqlx::query_as!(
        BlockPerformance,
        r#"
        SELECT
            b.id    as "block_id!: BlockId",
            b.block_no as "block_no!",
            b.title as "block_title!",
            c.id    as "course_id!: CourseId",
            c.code  as "course_code!",
            c.name  as "course_name!",
            sm.semester_number as "semester_number!",
            (SELECT count(*) FROM exams e
              WHERE e.block_id = b.id AND e.status = 'published')  as "exams_available!",
            (SELECT count(*) FROM exam_attempts ea
               JOIN exams e ON e.id = ea.exam_id
              WHERE e.block_id = b.id AND ea.student_id = sc.student_id) as "attempts_total!",
            (SELECT count(*) FROM exam_attempts ea
               JOIN exams e ON e.id = ea.exam_id
              WHERE e.block_id = b.id AND ea.student_id = sc.student_id
                AND ea.status = 'graded')                          as "attempts_graded!",
            (SELECT avg(100.0 * ea.score / ea.max_score) FROM exam_attempts ea
               JOIN exams e ON e.id = ea.exam_id
              WHERE e.block_id = b.id AND ea.student_id = sc.student_id
                AND ea.score IS NOT NULL)                          as "average_percentage",
            (SELECT max(100.0 * ea.score / ea.max_score) FROM exam_attempts ea
               JOIN exams e ON e.id = ea.exam_id
              WHERE e.block_id = b.id AND ea.student_id = sc.student_id
                AND ea.score IS NOT NULL)                          as "best_percentage",
            (SELECT count(*) FROM learning_sessions ls
              WHERE ls.block_id = b.id AND ls.student_id = sc.student_id) as "sessions_total!",
            (SELECT count(*) FROM learning_sessions ls
              WHERE ls.block_id = b.id AND ls.student_id = sc.student_id
                AND ls.status = 'completed')                       as "sessions_completed!",
            COALESCE((SELECT sum(ls.active_voice_ms) FROM learning_sessions ls
              WHERE ls.block_id = b.id AND ls.student_id = sc.student_id), 0)::bigint
                                                                   as "active_voice_ms!",
            (SELECT max(ls.started_at) FROM learning_sessions ls
              WHERE ls.block_id = b.id AND ls.student_id = sc.student_id) as "last_studied_at",
            -- `?` forced, not inferred. `student_block_remarks.remark` is
            -- TEXT NOT NULL in its own table, so sqlx types it as non-null and
            -- then fails to decode the NULL a LEFT JOIN produces for a block
            -- the student has not written about — which is most of them.
            r.remark     as "remark?",
            r.updated_at as "remark_updated_at?"
        FROM student_courses sc
        JOIN students  st ON st.id = sc.student_id
        JOIN courses   c  ON c.id  = sc.course_id
        JOIN semesters sm ON sm.id = c.semester_id
        JOIN blocks    b  ON b.course_id = c.id AND b.status = 'active'
        LEFT JOIN student_block_remarks r
               ON r.student_id = sc.student_id AND r.block_id = b.id
        WHERE sc.student_id = $1
          AND sc.status = 'active'
          AND ($2::uuid[] IS NULL OR st.program_id = ANY($2))
        ORDER BY sm.semester_number, c.code, b.block_no
        "#,
        student_id.into_uuid(),
        scope_program_ids
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// A topic the student has actually got wrong, and how often.
#[derive(Debug, Clone)]
pub struct WeakTopic {
    pub topic: String,
    pub missed_count: i64,
}

/// Weak topics for one student within one block, worst first.
///
/// Counts only answers marked **wrong** (`is_correct = false`). A NULL
/// `is_correct` belongs to an attempt that was never graded, and inventing
/// weakness from an abandoned paper would put a topic in front of the student
/// that nobody ever marked.
pub async fn weak_topics_for_block(
    pool: &PgPool,
    student_id: StudentId,
    block_id: BlockId,
    limit: i64,
) -> Result<Vec<WeakTopic>> {
    sqlx::query_as!(
        WeakTopic,
        r#"
        SELECT q.topic as "topic!", count(*) as "missed_count!"
        FROM exam_attempt_answers eaa
        JOIN exam_attempts ea ON ea.id = eaa.attempt_id
        JOIN exams         e  ON e.id  = ea.exam_id
        JOIN question_pool q  ON q.id  = eaa.question_id
        WHERE ea.student_id = $1
          AND e.block_id = $2
          AND eaa.is_correct = false
        GROUP BY q.topic
        ORDER BY count(*) DESC, q.topic
        LIMIT $3
        "#,
        student_id.into_uuid(),
        block_id.into_uuid(),
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Writes (or replaces) a student's remark on a block.
///
/// Upsert on `(student_id, block_id)`: editing a remark replaces it. There is
/// deliberately no history — the page shows one current reflection per block,
/// and a version log nothing renders is a store nobody maintains.
///
/// The block is **not** validated here. The handler resolves it through the
/// student's own active enrolments first, which is the same check that decides
/// whether the block may be shown at all; re-checking it in SQL would imply
/// this function is safe to call with an unchecked id, which it is not.
pub async fn upsert_remark(
    pool: &PgPool,
    student_id: StudentId,
    block_id: BlockId,
    remark: &str,
) -> Result<DateTime<Utc>> {
    let row = sqlx::query!(
        r#"
        INSERT INTO student_block_remarks (student_id, block_id, remark)
        VALUES ($1, $2, $3)
        ON CONFLICT (student_id, block_id) DO UPDATE
          SET remark = EXCLUDED.remark, updated_at = now()
        RETURNING updated_at as "updated_at!"
        "#,
        student_id.into_uuid(),
        block_id.into_uuid(),
        remark
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(row.updated_at)
}

/// Removes a student's remark on a block. Idempotent: clearing a remark that
/// was never written is success, not `404` — the end state the caller asked
/// for is the end state they get.
pub async fn delete_remark(
    pool: &PgPool,
    student_id: StudentId,
    block_id: BlockId,
) -> Result<()> {
    sqlx::query!(
        "DELETE FROM student_block_remarks WHERE student_id = $1 AND block_id = $2",
        student_id.into_uuid(),
        block_id.into_uuid()
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}

/// Confirms a block is inside the student's own active enrolments.
///
/// The gate on the remark write. Returns `false` for a block that does not
/// exist and for one in someone else's programme alike — the handler answers
/// `404` to both, so an id belonging to another student cannot be told apart
/// from one that was never real.
pub async fn block_is_enrolled(
    pool: &PgPool,
    student_id: StudentId,
    block_id: BlockId,
) -> Result<bool> {
    let row = sqlx::query!(
        r#"
        SELECT 1 as "hit!"
        FROM student_courses sc
        JOIN blocks b ON b.course_id = sc.course_id
        WHERE sc.student_id = $1 AND sc.status = 'active' AND b.id = $2
        "#,
        student_id.into_uuid(),
        block_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(row.is_some())
}

/// One student's line in the admin's "best performers" board.
#[derive(Debug, Clone)]
pub struct TopPerformer {
    pub student_id: StudentId,
    pub student_name: String,
    pub roll_number: String,
    pub program_name: String,
    pub semester_number: i16,
    pub attempts_graded: i64,
    pub average_percentage: f64,
    pub best_percentage: f64,
}

/// The best performers, by average across graded attempts.
///
/// Two deliberate constraints:
///
/// * Only **graded** attempts count. An average over submitted-but-unmarked
///   papers would rank students on work nobody has marked.
/// * A student needs at least `min_attempts` graded papers to appear at all.
///   Ranking a student who has sat one paper above one who has sat twenty is
///   not a leaderboard, it is a sampling artefact — the same reason
///   `dg_core::performance::trophy_for` refuses a trophy on thin evidence.
///
/// Scoped like every other admin read: the caller's role decides which query
/// runs, and `Some(&[])` yields nothing rather than falling back to unscoped.
pub async fn top_performers(
    pool: &PgPool,
    scope_program_ids: Option<&[uuid::Uuid]>,
    min_attempts: i64,
    limit: i64,
) -> Result<Vec<TopPerformer>> {
    sqlx::query_as!(
        TopPerformer,
        r#"
        SELECT
            st.id          as "student_id!: StudentId",
            st.full_name   as "student_name!",
            st.roll_number as "roll_number!",
            p.name         as "program_name!",
            sm.semester_number as "semester_number!",
            count(*)                                   as "attempts_graded!",
            avg(100.0 * ea.score / ea.max_score)       as "average_percentage!",
            max(100.0 * ea.score / ea.max_score)       as "best_percentage!"
        FROM exam_attempts ea
        JOIN students  st ON st.id = ea.student_id
        JOIN programs  p  ON p.id  = st.program_id
        JOIN semesters sm ON sm.id = st.semester_id
        WHERE ea.status = 'graded'
          AND ea.score IS NOT NULL
          AND ($1::uuid[] IS NULL OR st.program_id = ANY($1))
        GROUP BY st.id, st.full_name, st.roll_number, p.name, sm.semester_number
        HAVING count(*) >= $2
        -- Roll number breaks a tie, so the board does not reshuffle between
        -- refreshes when two students share an average.
        ORDER BY avg(100.0 * ea.score / ea.max_score) DESC, st.roll_number
        LIMIT $3
        "#,
        scope_program_ids,
        min_attempts,
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}
