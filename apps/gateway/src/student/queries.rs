//! Queries the student dashboard needs that no `crates/db` module owns yet.
//!
//! Everything else in `student/` goes through `dg_db::models` — that is the
//! pattern and it stays the pattern. The three reads here are the exception,
//! and deliberately a narrow one: `learning_sessions` and `note_reminders`
//! have struct definitions in `dg_db::models::content` but no query module,
//! because nothing wrote to either table until the classroom landed. Rather
//! than stretch a shared crate for three dashboard-shaped aggregates that no
//! other caller wants, they live here, still as `sqlx::query_as!`/`query!`
//! macros so they are checked against the real schema at compile time exactly
//! like every query in `crates/db` is.
//!
//! If a second caller ever needs one of these, that is the signal to promote
//! it into `crates/db/src/models/` — not to copy it.
//!
//! Every query binds `student_id` as a mandatory predicate. There is no
//! function in this module that can be asked for another student's rows.

use chrono::{DateTime, NaiveDate, Utc};
use dg_core::{BlockId, CourseId, PublicError, SessionId, StudentId};
use dg_db::PgPool;
use uuid::Uuid;

/// The student's most recent classroom session, with the two counts the
/// progress bar is computed from.
#[derive(Debug)]
pub struct ResumePoint {
    pub session_id: SessionId,
    pub course_id: CourseId,
    pub course_title: String,
    pub block_id: BlockId,
    pub block_no: i16,
    pub block_title: String,
    pub chapter_name: Option<String>,
    pub page_number: Option<i32>,
    pub resume_summary: Option<String>,
    pub last_active_at: DateTime<Utc>,
    /// Blocks in the course this session belongs to.
    pub blocks_total: i64,
    /// Distinct blocks of that course this student has a `completed` session
    /// for. The numerator of `progress_percentage`.
    pub blocks_completed: i64,
}

/// Where to drop the student back in: the newest `learning_sessions` row,
/// whatever its status.
///
/// `None` for a student who has never had a session — a first-login dashboard
/// must render, so the caller turns this into a `null` `continue_learning`
/// rather than an error.
///
/// The two progress counts are correlated subqueries on the same row rather
/// than a second round trip: they are only ever wanted together with the
/// session they describe.
pub async fn latest_resume_point(
    pool: &PgPool,
    student_id: StudentId,
) -> Result<Option<ResumePoint>, PublicError> {
    sqlx::query_as!(
        ResumePoint,
        r#"
        SELECT
            ls.id        as "session_id!: SessionId",
            ls.course_id as "course_id!: CourseId",
            c.name       as "course_title!",
            ls.block_id  as "block_id!: BlockId",
            b.block_no   as "block_no!",
            b.title      as "block_title!",
            ls.last_topic      as "chapter_name",
            ls.last_page       as "page_number",
            ls.resume_summary  as "resume_summary",
            COALESCE(ls.ended_at, ls.started_at) as "last_active_at!",
            (SELECT count(*) FROM blocks bb WHERE bb.course_id = ls.course_id)
                as "blocks_total!",
            (SELECT count(DISTINCT ls2.block_id)
               FROM learning_sessions ls2
              WHERE ls2.student_id = ls.student_id
                AND ls2.course_id  = ls.course_id
                AND ls2.status     = 'completed')
                as "blocks_completed!"
        FROM learning_sessions ls
        JOIN blocks  b ON b.id = ls.block_id
        JOIN courses c ON c.id = ls.course_id
        WHERE ls.student_id = $1
        ORDER BY ls.started_at DESC, ls.id DESC
        LIMIT 1
        "#,
        student_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "latest resume point query failed");
        PublicError::Internal
    })
}

/// The two note-library numbers on the dashboard.
#[derive(Debug, Clone, Copy)]
pub struct NoteCounts {
    /// Every note this student has tagged for revision — the "saved notes"
    /// tile. `note_reminders` is the only ledger of exported notes
    /// (`.claude/rules/pedagogy.md`: export is a client-side render, so the
    /// reminder row is the *only* server-side trace a note was kept).
    pub preserved: i64,
    /// Of those, the ones due on or before the student's local today.
    pub due: i64,
}

/// Note counts for the dashboard. `today` is the student's **local** date, for
/// the NN-3 reason: "due today" is a calendar question and the student's
/// calendar is the one that answers it.
pub async fn note_counts(
    pool: &PgPool,
    student_id: StudentId,
    today: NaiveDate,
) -> Result<NoteCounts, PublicError> {
    let rec = sqlx::query!(
        r#"
        SELECT count(*)                                     as "preserved!",
               count(*) FILTER (WHERE nr.remind_at <= $2)   as "due!"
        FROM note_reminders nr
        WHERE nr.student_id = $1
        "#,
        student_id.into_uuid(),
        today
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "note reminder counts query failed");
        PublicError::Internal
    })?;

    Ok(NoteCounts {
        preserved: rec.preserved,
        due: rec.due,
    })
}

/// One due revision: the reminder, the board-op range it points at, and the
/// session's place in the catalogue so a card renders without a second
/// request.
#[derive(Debug)]
pub struct DueRevision {
    pub id: Uuid,
    pub session_id: SessionId,
    pub event_from_id: i64,
    pub event_to_id: i64,
    /// How many `board_events` actually fall inside the range — what the card
    /// shows as "9 notes". Counted rather than derived from the id span: ids
    /// are a global sequence, so `to - from` is not a row count.
    pub board_ops_count: i64,
    pub course_id: CourseId,
    pub course_code: String,
    pub block_id: BlockId,
    pub block_no: i16,
    pub block_title: String,
    /// The session's last topic, the nearest thing to a note title.
    pub topic: Option<String>,
    pub remind_at: NaiveDate,
    pub surfaced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// One page of this student's due revisions, most recently due first.
///
/// `due_on_or_before` is the student's local today: a reminder set for
/// tomorrow in the student's own zone must not surface because the server has
/// already rolled over.
pub async fn due_revisions(
    pool: &PgPool,
    student_id: StudentId,
    due_on_or_before: NaiveDate,
    limit: i64,
    offset: i64,
) -> Result<Vec<DueRevision>, PublicError> {
    sqlx::query_as!(
        DueRevision,
        r#"
        SELECT
            nr.id            as "id!",
            nr.session_id    as "session_id!: SessionId",
            nr.event_from_id as "event_from_id!",
            nr.event_to_id   as "event_to_id!",
            (SELECT count(*)
               FROM board_events be
              WHERE be.session_id = nr.session_id
                AND be.id BETWEEN nr.event_from_id AND nr.event_to_id)
                             as "board_ops_count!",
            ls.course_id     as "course_id!: CourseId",
            c.code           as "course_code!",
            ls.block_id      as "block_id!: BlockId",
            b.block_no       as "block_no!",
            b.title          as "block_title!",
            ls.last_topic    as "topic",
            nr.remind_at     as "remind_at!",
            nr.surfaced_at   as "surfaced_at",
            nr.created_at    as "created_at!"
        FROM note_reminders nr
        JOIN learning_sessions ls ON ls.id = nr.session_id
        JOIN blocks  b ON b.id = ls.block_id
        JOIN courses c ON c.id = ls.course_id
        WHERE nr.student_id = $1
          AND nr.remind_at <= $2
        ORDER BY nr.remind_at DESC, nr.created_at DESC, nr.id DESC
        LIMIT $3 OFFSET $4
        "#,
        student_id.into_uuid(),
        due_on_or_before,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "due revisions query failed");
        PublicError::Internal
    })
}

/// `X-Total-Count` for [`due_revisions`] — the same predicates, no `LIMIT`.
pub async fn count_due_revisions(
    pool: &PgPool,
    student_id: StudentId,
    due_on_or_before: NaiveDate,
) -> Result<i64, PublicError> {
    sqlx::query_scalar!(
        r#"
        SELECT count(*) as "count!"
        FROM note_reminders nr
        JOIN learning_sessions ls ON ls.id = nr.session_id
        WHERE nr.student_id = $1 AND nr.remind_at <= $2
        "#,
        student_id.into_uuid(),
        due_on_or_before
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "due revision count query failed");
        PublicError::Internal
    })
}
