//! Drill-down lists behind the dashboard's session and whiteboard tiles —
//! `GET /api/v1/admin/sessions` and `GET /api/v1/admin/board-events`.
//!
//! The construction (one private SQL body per list, `($2 OR col = ANY($1))`
//! scoping, thin `_all` / `_scoped` wrappers so the caller's *role* picks the
//! query) is documented in full on [`crate::models::drilldown`]; this module is
//! its other half, split out only for file length.
//!
//! Scoping walks `learning_sessions -> courses -> program_id` and
//! `board_events -> learning_sessions -> courses -> program_id`.

use chrono::{DateTime, Utc};
use dg_core::{BlockId, CourseId, ProgramId, SessionId, SessionStatus, StudentId};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::drilldown::raw;
use crate::error::{Error, Result};

/// The NN-1 `HOLD_MAX` in milliseconds (`.claude/rules/whiteboard-sync.md`).
/// An op acked later than this — or never acked — was released without its
/// board landing first, i.e. a `wb_violation`.
pub const HOLD_MAX_MS: i32 = 400;

/// One row of `GET /admin/sessions`.
#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: SessionId,
    pub student_id: StudentId,
    pub student_name: String,
    pub roll_number: String,
    pub course_id: CourseId,
    pub course_code: String,
    pub block_id: BlockId,
    pub block_no: i16,
    pub block_title: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    /// NN-3, server-authoritative; never above 1 200 000.
    pub active_voice_ms: i64,
    pub status: SessionStatus,
    /// `quota|idle|user|jailbreak|error`; `None` while the session runs.
    pub end_reason: Option<String>,
    pub last_topic: Option<String>,
    /// Zero rather than `None` when no page was recorded — the wire contract
    /// types this as a number, and "no page yet" and "page 0" are the same
    /// thing to a list that only renders a location.
    pub last_page: i32,
    /// Per-session whiteboard roll-ups, so the list can flag a bad session
    /// without a second request per row.
    pub board_ops: i64,
    pub board_violations: i64,
}

/// The `GET /admin/sessions` filters. `from`/`to` bound `started_at`
/// inclusively and are already parsed from RFC3339 by the handler.
#[derive(Debug, Clone, Copy, Default)]
pub struct SessionFilters {
    pub status: Option<SessionStatus>,
    /// `quota|idle|user|jailbreak|error`, validated by the handler.
    pub end_reason: Option<&'static str>,
    pub student_id: Option<Uuid>,
    pub block_id: Option<Uuid>,
    pub course_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

/// The `session_status` label this maps to in Postgres.
fn status_str(s: SessionStatus) -> &'static str {
    match s {
        SessionStatus::InProgress => "in_progress",
        SessionStatus::Completed => "completed",
        SessionStatus::Abandoned => "abandoned",
    }
}

/// Platform-wide sessions. Super-admin only.
pub async fn sessions_all(
    pool: &PgPool,
    f: SessionFilters,
    limit: i64,
    offset: i64,
) -> Result<Vec<SessionRow>> {
    sessions(pool, &[], true, f, limit, offset).await
}

/// The same list, restricted to `program_ids`.
pub async fn sessions_scoped(
    pool: &PgPool,
    program_ids: &[ProgramId],
    f: SessionFilters,
    limit: i64,
    offset: i64,
) -> Result<Vec<SessionRow>> {
    sessions(pool, &raw(program_ids), false, f, limit, offset).await
}

/// `X-Total-Count` for [`sessions_all`].
pub async fn count_sessions_all(pool: &PgPool, f: SessionFilters) -> Result<i64> {
    count_sessions(pool, &[], true, f).await
}

/// `X-Total-Count` for [`sessions_scoped`].
pub async fn count_sessions_scoped(
    pool: &PgPool,
    program_ids: &[ProgramId],
    f: SessionFilters,
) -> Result<i64> {
    count_sessions(pool, &raw(program_ids), false, f).await
}

async fn sessions(
    pool: &PgPool,
    ids: &[Uuid],
    unscoped: bool,
    f: SessionFilters,
    limit: i64,
    offset: i64,
) -> Result<Vec<SessionRow>> {
    // The roll-ups are a LATERAL aggregate over `board_events`, not a second
    // round trip per row: a page of 50 sessions would otherwise be 51 queries.
    sqlx::query_as!(
        SessionRow,
        r#"
        SELECT
            ls.id                          as "id!: SessionId",
            ls.student_id                  as "student_id!: StudentId",
            st.full_name                   as "student_name!",
            st.roll_number                 as "roll_number!",
            ls.course_id                   as "course_id!: CourseId",
            c.code                         as "course_code!",
            ls.block_id                    as "block_id!: BlockId",
            b.block_no                     as "block_no!",
            b.title                        as "block_title!",
            ls.started_at                  as "started_at!",
            ls.ended_at                    as "ended_at",
            ls.active_voice_ms             as "active_voice_ms!",
            ls.status                      as "status!: SessionStatus",
            ls.end_reason                  as "end_reason",
            ls.last_topic                  as "last_topic",
            COALESCE(ls.last_page, 0)      as "last_page!",
            COALESCE(be.ops, 0)            as "board_ops!",
            COALESCE(be.violations, 0)     as "board_violations!"
        FROM learning_sessions ls
        JOIN students st ON st.id = ls.student_id
        JOIN courses  c  ON c.id  = ls.course_id
        JOIN blocks   b  ON b.id  = ls.block_id
        LEFT JOIN LATERAL (
            SELECT count(*)                                              AS ops,
                   count(*) FILTER (
                       WHERE e.acked_ms IS NULL OR e.acked_ms > $10
                   )                                                     AS violations
            FROM board_events e
            WHERE e.session_id = ls.id
        ) be ON true
        WHERE ($2 OR c.program_id = ANY($1))
          AND ($3::text        IS NULL OR ls.status = $3::text::session_status)
          AND ($4::text        IS NULL OR ls.end_reason = $4)
          AND ($5::uuid        IS NULL OR ls.student_id = $5)
          AND ($6::uuid        IS NULL OR ls.block_id   = $6)
          AND ($7::uuid        IS NULL OR ls.course_id  = $7)
          AND ($8::timestamptz IS NULL OR ls.started_at >= $8)
          AND ($9::timestamptz IS NULL OR ls.started_at <= $9)
        ORDER BY ls.started_at DESC, ls.id
        LIMIT $11 OFFSET $12
        "#,
        ids,
        unscoped,
        f.status.map(status_str),
        f.end_reason,
        f.student_id,
        f.block_id,
        f.course_id,
        f.from,
        f.to,
        HOLD_MAX_MS,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

async fn count_sessions(
    pool: &PgPool,
    ids: &[Uuid],
    unscoped: bool,
    f: SessionFilters,
) -> Result<i64> {
    sqlx::query_scalar!(
        r#"
        SELECT count(*) as "count!"
        FROM learning_sessions ls
        JOIN courses c ON c.id = ls.course_id
        WHERE ($2 OR c.program_id = ANY($1))
          AND ($3::text        IS NULL OR ls.status = $3::text::session_status)
          AND ($4::text        IS NULL OR ls.end_reason = $4)
          AND ($5::uuid        IS NULL OR ls.student_id = $5)
          AND ($6::uuid        IS NULL OR ls.block_id   = $6)
          AND ($7::uuid        IS NULL OR ls.course_id  = $7)
          AND ($8::timestamptz IS NULL OR ls.started_at >= $8)
          AND ($9::timestamptz IS NULL OR ls.started_at <= $9)
        "#,
        ids,
        unscoped,
        f.status.map(status_str),
        f.end_reason,
        f.student_id,
        f.block_id,
        f.course_id,
        f.from,
        f.to
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// One row of `GET /admin/board-events`.
#[derive(Debug, Clone)]
pub struct BoardEventRow {
    pub id: i64,
    pub session_id: SessionId,
    pub turn_seq: i32,
    /// `heading|bullets|math|draw|image|highlight`, lifted out of `op` so a
    /// list renders without parsing the payload. Falls back to `unknown` for a
    /// row whose `op` predates op validation — a list of audit rows must not
    /// 500 because one historic payload is shaped oddly.
    pub op_kind: String,
    /// The full validated payload, for a detail view.
    pub op: Value,
    pub emitted_at: DateTime<Utc>,
    /// Client ACK latency; `None` means never acked.
    pub acked_ms: Option<i32>,
    /// `acked_ms IS NULL OR acked_ms > HOLD_MAX_MS`.
    pub is_violation: bool,
}

/// The `GET /admin/board-events` filters.
#[derive(Debug, Clone, Copy, Default)]
pub struct BoardEventFilters {
    pub session_id: Option<Uuid>,
    pub violations_only: bool,
    /// One of the analytics ack-latency bucket labels, validated by the
    /// handler. Buckets cover acked ops only, matching `analytics`.
    pub bucket: Option<&'static str>,
}

/// Platform-wide board events. Super-admin only.
pub async fn board_events_all(
    pool: &PgPool,
    f: BoardEventFilters,
    limit: i64,
    offset: i64,
) -> Result<Vec<BoardEventRow>> {
    board_events(pool, &[], true, f, limit, offset).await
}

/// The same list, restricted to `program_ids`.
pub async fn board_events_scoped(
    pool: &PgPool,
    program_ids: &[ProgramId],
    f: BoardEventFilters,
    limit: i64,
    offset: i64,
) -> Result<Vec<BoardEventRow>> {
    board_events(pool, &raw(program_ids), false, f, limit, offset).await
}

/// `X-Total-Count` for [`board_events_all`].
pub async fn count_board_events_all(pool: &PgPool, f: BoardEventFilters) -> Result<i64> {
    count_board_events(pool, &[], true, f).await
}

/// `X-Total-Count` for [`board_events_scoped`].
pub async fn count_board_events_scoped(
    pool: &PgPool,
    program_ids: &[ProgramId],
    f: BoardEventFilters,
) -> Result<i64> {
    count_board_events(pool, &raw(program_ids), false, f).await
}

async fn board_events(
    pool: &PgPool,
    ids: &[Uuid],
    unscoped: bool,
    f: BoardEventFilters,
    limit: i64,
    offset: i64,
) -> Result<Vec<BoardEventRow>> {
    // Ordering is one expression, not two queries: with `session_id` given the
    // leading key is `turn_seq` and the page reads as a transcript; without it
    // the key is a constant NULL for every row, so the sort falls straight
    // through to the reverse-chronological feed.
    sqlx::query_as!(
        BoardEventRow,
        r#"
        SELECT
            be.id                                     as "id!",
            be.session_id                             as "session_id!: SessionId",
            be.turn_seq                               as "turn_seq!",
            COALESCE(be.op ->> 'type', 'unknown')     as "op_kind!",
            be.op                                     as "op!",
            be.emitted_at                             as "emitted_at!",
            be.acked_ms                               as "acked_ms",
            (be.acked_ms IS NULL OR be.acked_ms > $4) as "is_violation!"
        FROM board_events be
        JOIN learning_sessions ls ON ls.id = be.session_id
        JOIN courses c ON c.id = ls.course_id
        WHERE ($2 OR c.program_id = ANY($1))
          AND ($3::uuid IS NULL OR be.session_id = $3)
          AND (NOT $5 OR be.acked_ms IS NULL OR be.acked_ms > $4)
          AND ($6::text IS NULL
               OR (be.acked_ms IS NOT NULL
                   AND CASE
                         WHEN be.acked_ms < 100 THEN '0-100ms'
                         WHEN be.acked_ms < 250 THEN '100-250ms'
                         WHEN be.acked_ms < 400 THEN '250-400ms'
                         ELSE '>400ms'
                       END = $6))
        ORDER BY
            CASE WHEN $3::uuid IS NOT NULL THEN be.turn_seq END ASC,
            be.emitted_at DESC,
            be.id DESC
        LIMIT $7 OFFSET $8
        "#,
        ids,
        unscoped,
        f.session_id,
        HOLD_MAX_MS,
        f.violations_only,
        f.bucket,
        limit,
        offset
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

async fn count_board_events(
    pool: &PgPool,
    ids: &[Uuid],
    unscoped: bool,
    f: BoardEventFilters,
) -> Result<i64> {
    sqlx::query_scalar!(
        r#"
        SELECT count(*) as "count!"
        FROM board_events be
        JOIN learning_sessions ls ON ls.id = be.session_id
        JOIN courses c ON c.id = ls.course_id
        WHERE ($2 OR c.program_id = ANY($1))
          AND ($3::uuid IS NULL OR be.session_id = $3)
          AND (NOT $5 OR be.acked_ms IS NULL OR be.acked_ms > $4)
          AND ($6::text IS NULL
               OR (be.acked_ms IS NOT NULL
                   AND CASE
                         WHEN be.acked_ms < 100 THEN '0-100ms'
                         WHEN be.acked_ms < 250 THEN '100-250ms'
                         WHEN be.acked_ms < 400 THEN '250-400ms'
                         ELSE '>400ms'
                       END = $6))
        "#,
        ids,
        unscoped,
        f.session_id,
        HOLD_MAX_MS,
        f.violations_only,
        f.bucket
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}
