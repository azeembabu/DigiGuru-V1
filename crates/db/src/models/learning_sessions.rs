//! Writes to `learning_sessions` — the classroom session row.
//!
//! **Why this module had to exist.** Until now a classroom session lived only in
//! Redis (`sess:{id}`), so `learning_sessions` was never inserted into. Three
//! things read that table and were therefore permanently empty, reporting health
//! rather than absence:
//!
//! * `GET /admin/sessions` — the session drill-down,
//! * the `sessions` block and `sessions_daily` series in `/admin/analytics`,
//! * `board_events.session_id`, a foreign key to this table, which means the
//!   NN-1 audit trail could not be written at all while the row was missing.
//!
//! Redis stays the live state (`realtime-audio.md`: a reconnect resumes from
//! `sess:{session_id}`, and the gateway is stateless with respect to any one
//! socket). This table is the durable record of the same session: Redis answers
//! "what is happening now", Postgres answers "what happened".
//!
//! The id is supplied by the caller rather than generated here, because the
//! gateway has already minted it for the Redis key and both stores must agree on
//! one identity.

use sqlx::PgPool;
use uuid::Uuid;

use dg_core::{BlockId, SessionId, StudentId};

use crate::error::Result;

/// Opens (or re-opens) a session row.
///
/// Idempotent on `id`: a reconnect that resumes the same `sess:{id}` calls this
/// again with the same id, and must not create a second row or fail. `ON
/// CONFLICT DO NOTHING` is therefore correct rather than lazy — the row already
/// describes the session being resumed, and overwriting `started_at` would
/// misreport its duration.
///
/// `course_id` is resolved from the block rather than taken as an argument, so a
/// caller cannot file a session under a course the block does not belong to.
pub async fn open(
    pool: &PgPool,
    id: SessionId,
    student_id: StudentId,
    block_id: BlockId,
) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO learning_sessions (id, student_id, course_id, block_id, status)
        SELECT $1, $2, b.course_id, b.id, 'in_progress'
        FROM blocks b
        WHERE b.id = $3
        ON CONFLICT (id) DO NOTHING
        "#,
        id.into_uuid(),
        student_id.into_uuid(),
        block_id.into_uuid(),
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Closes a session, recording why.
///
/// `end_reason` is one of `quota|idle|user|jailbreak|error`, matching the
/// `session_end` close reasons in `api-conventions.md` and the
/// `session_end_reasons` analytics series. Only an `in_progress` row is touched:
/// a session already closed (by the quota ledger, or a guardrail termination)
/// keeps its original reason, because the first cause is the true one and a
/// later socket teardown must not relabel a jailbreak as a polite exit.
pub async fn close(
    pool: &PgPool,
    id: SessionId,
    end_reason: &str,
    active_voice_ms: i64,
) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE learning_sessions
        SET ended_at = now(),
            status = 'completed',
            end_reason = $2,
            active_voice_ms = GREATEST(active_voice_ms, $3)
        WHERE id = $1 AND status = 'in_progress'
        "#,
        id.into_uuid(),
        end_reason,
        active_voice_ms,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Records where the lesson had reached, for the dashboard's resume card.
///
/// Called on a turn boundary, so it is a hot-ish path: one statement, no reads,
/// and a no-op when the session has already ended.
pub async fn record_progress(
    pool: &PgPool,
    id: SessionId,
    last_topic: Option<&str>,
    last_page: Option<i32>,
) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE learning_sessions
        SET last_topic = COALESCE($2, last_topic),
            last_page = COALESCE($3, last_page)
        WHERE id = $1 AND status = 'in_progress'
        "#,
        id.into_uuid(),
        last_topic,
        last_page,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// True when this id already has a row, so the gateway can tell a genuine resume
/// from a first connect without assuming.
pub async fn exists(pool: &PgPool, id: Uuid) -> Result<bool> {
    let found = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM learning_sessions WHERE id = $1)"#,
        id
    )
    .fetch_one(pool)
    .await?;

    Ok(found.unwrap_or(false))
}

/// The most recent `(topic, page)` this student reached in this block, from a
/// session other than the one just opened.
///
/// Used to phrase the classroom's resume kickoff ("continue at page N") rather
/// than opening blind on every reconnect. `NULL` fields on the row (nothing
/// taught yet) come back as `None`, not `Some("")`/`Some(0)` — an empty
/// resume prompt is worse than none, since it would tell the tutor to
/// "continue from nothing".
pub async fn latest_progress_for_block(
    pool: &PgPool,
    student_id: StudentId,
    block_id: BlockId,
    excluding: SessionId,
) -> Result<Option<(String, i32)>> {
    let row = sqlx::query!(
        r#"
        SELECT last_topic, last_page
        FROM learning_sessions
        WHERE student_id = $1 AND block_id = $2 AND id != $3
          AND last_topic IS NOT NULL AND last_page IS NOT NULL
        ORDER BY started_at DESC
        LIMIT 1
        "#,
        student_id.into_uuid(),
        block_id.into_uuid(),
        excluding.into_uuid(),
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(|r| match (r.last_topic, r.last_page) {
        (Some(topic), Some(page)) => Some((topic, page)),
        _ => None,
    }))
}
