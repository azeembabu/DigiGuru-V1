//! Dashboard metrics and chart series for `GET /api/v1/admin/analytics`.
//!
//! This is the richer sibling of [`crate::models::stats`]; `/admin/stats` is
//! unchanged and still served, because the existing dashboard tiles read it.
//!
//! # Why one query function and not two
//!
//! `stats.rs` carries the unscoped and the scoped variant as two hand-written
//! SQL bodies. That is tolerable for ten counters. It is not tolerable for the
//! ~50 here: two copies of a 50-metric `SELECT` drift the moment someone edits
//! one of them, and the drift is silent — a sub-admin would simply see a
//! different definition of the same tile than a super-admin does, with nothing
//! failing to announce it.
//!
//! So every query body is written **once**, in a private function taking
//! `(pool, program_ids, unscoped)`, and each scoping predicate is
//! `($2 OR <col> = ANY($1))`: when `$2` is true the predicate is a constant
//! `true` and the filter vanishes, otherwise the row must belong to one of the
//! caller's programs. The two thin public wrappers — [`counts_all`] and
//! [`counts_scoped`] — still exist so the *caller's role* picks the function,
//! exactly as it does for `/admin/stats`; the role is never reduced to a flag
//! threaded through a single public entry point, because a flag is far easier
//! to default wrong than a missing match arm is.
//!
//! Scoping walks `blocks -> courses -> program_id`,
//! `documents -> blocks -> courses`, `learning_sessions -> courses`,
//! `board_events -> learning_sessions -> courses`,
//! `student_courses -> courses`, and `safety_incidents -> students`. `lscs` is
//! platform-wide in both variants for the reason given in `stats.rs`: an LSC
//! has no program, and every admin may already list them all.
//!
//! An empty scope slice is a real answer — zeroes and empty series — not a
//! reason to fall back to the unscoped query.

use chrono::NaiveDate;
use dg_core::ProgramId;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Error, Result};

/// How many days each `*_daily` series spans, inclusive of today.
pub const SERIES_DAYS: i64 = 30;

/// Every scalar metric on the analytics dashboard, in one round trip.
#[derive(Debug, Clone)]
pub struct AnalyticsCounts {
    // catalogue
    pub programs: i64,
    pub semesters: i64,
    pub courses: i64,
    pub blocks: i64,
    pub blocks_active: i64,
    pub blocks_inactive: i64,
    pub lscs: i64,
    pub blocks_without_documents: i64,
    pub courses_without_blocks: i64,
    pub semesters_without_courses: i64,
    // students
    pub students_total: i64,
    pub students_active: i64,
    pub students_inactive: i64,
    pub students_suspended: i64,
    pub students_first_login_pending: i64,
    pub students_new_last_30d: i64,
    pub students_without_enrollment: i64,
    pub enrollments_active: i64,
    pub enrollments_completed: i64,
    pub enrollments_dropped: i64,
    // documents
    pub documents_total: i64,
    pub documents_pending: i64,
    pub documents_parsing: i64,
    pub documents_pending_review: i64,
    pub documents_embedded: i64,
    pub documents_failed: i64,
    pub documents_total_pages: i64,
    pub documents_avg_ocr_confidence: Option<f64>,
    pub jobs_pending: i64,
    pub jobs_processing: i64,
    pub jobs_completed: i64,
    pub jobs_failed: i64,
    pub jobs_retried: i64,
    // sessions
    pub sessions_total: i64,
    pub sessions_in_progress: i64,
    pub sessions_completed: i64,
    pub sessions_abandoned: i64,
    pub sessions_last_7d: i64,
    pub active_voice_ms_total: i64,
    pub active_voice_ms_avg: Option<f64>,
    pub ended_quota: i64,
    pub ended_idle: i64,
    pub ended_user: i64,
    pub ended_jailbreak: i64,
    pub ended_error: i64,
    // whiteboard
    pub ops_total: i64,
    pub ops_acked: i64,
    pub ops_unacked: i64,
    pub ack_p50_ms: Option<f64>,
    pub ack_p95_ms: Option<f64>,
    // safety
    pub safety_total: i64,
    pub safety_tier0: i64,
    pub safety_tier1: i64,
    pub safety_tier2: i64,
    pub safety_last_7d: i64,
    pub safety_jailbreak: i64,
    pub safety_toxicity: i64,
    pub safety_out_of_scope: i64,
}

/// One day of the sessions series. `voice_ms` is the day's total active voice.
#[derive(Debug, Clone)]
pub struct SessionDay {
    pub day: NaiveDate,
    pub count: i64,
    pub voice_ms: i64,
}

/// One day of a plain counting series.
#[derive(Debug, Clone)]
pub struct DayCount {
    pub day: NaiveDate,
    pub count: i64,
}

/// One bar of a fixed-label series.
#[derive(Debug, Clone)]
pub struct LabelCount {
    pub label: String,
    pub count: i64,
}

/// Every chart series, each already gap-filled or label-filled so the client
/// renders it without post-processing.
#[derive(Debug, Clone)]
pub struct AnalyticsSeries {
    pub sessions_daily: Vec<SessionDay>,
    pub students_daily: Vec<DayCount>,
    pub documents_daily: Vec<DayCount>,
    pub incidents_daily: Vec<DayCount>,
    pub documents_by_status: Vec<LabelCount>,
    pub session_end_reasons: Vec<LabelCount>,
    pub ack_latency_buckets: Vec<LabelCount>,
    pub top_blocks: Vec<LabelCount>,
}

/// The whole payload: scalars plus series.
#[derive(Debug, Clone)]
pub struct AdminAnalytics {
    pub counts: AnalyticsCounts,
    pub series: AnalyticsSeries,
}

/// Platform-wide analytics. Super-admin only — [`counts_scoped`] is the
/// sub-admin path.
pub async fn counts_all(pool: &PgPool) -> Result<AdminAnalytics> {
    load(pool, &[], true).await
}

/// The same analytics, restricted to `program_ids`. An empty slice yields
/// zeroes and empty series (except `lscs`), which is the correct answer for a
/// sub-admin with no scopes — never a reason to run [`counts_all`].
pub async fn counts_scoped(pool: &PgPool, program_ids: &[ProgramId]) -> Result<AdminAnalytics> {
    let ids: Vec<Uuid> = program_ids.iter().map(|p| p.into_uuid()).collect();
    load(pool, &ids, false).await
}

async fn load(pool: &PgPool, ids: &[Uuid], unscoped: bool) -> Result<AdminAnalytics> {
    let counts = scalars(pool, ids, unscoped).await?;
    let series = AnalyticsSeries {
        sessions_daily: sessions_daily(pool, ids, unscoped).await?,
        students_daily: students_daily(pool, ids, unscoped).await?,
        documents_daily: documents_daily(pool, ids, unscoped).await?,
        incidents_daily: incidents_daily(pool, ids, unscoped).await?,
        documents_by_status: documents_by_status(pool, ids, unscoped).await?,
        session_end_reasons: session_end_reasons(pool, ids, unscoped).await?,
        ack_latency_buckets: ack_latency_buckets(pool, ids, unscoped).await?,
        top_blocks: top_blocks(pool, ids, unscoped).await?,
    };
    Ok(AdminAnalytics { counts, series })
}

/// Every scalar metric, as one `SELECT` of correlated sub-queries — the same
/// single-round-trip shape `stats::counts_all` uses, for the same reason:
/// a tile per query would be an N+1 fan-out on a dashboard load.
async fn scalars(pool: &PgPool, ids: &[Uuid], unscoped: bool) -> Result<AnalyticsCounts> {
    sqlx::query_as!(
        AnalyticsCounts,
        r#"
        SELECT
            (SELECT count(*) FROM programs p
                WHERE ($2 OR p.id = ANY($1)))                       as "programs!",
            (SELECT count(*) FROM semesters s
                WHERE ($2 OR s.program_id = ANY($1)))               as "semesters!",
            (SELECT count(*) FROM courses c
                WHERE ($2 OR c.program_id = ANY($1)))               as "courses!",
            (SELECT count(*) FROM blocks b
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1)))               as "blocks!",
            (SELECT count(*) FROM blocks b
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND b.status = 'active')                          as "blocks_active!",
            (SELECT count(*) FROM blocks b
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND b.status = 'inactive')                        as "blocks_inactive!",
            (SELECT count(*) FROM lscs)                             as "lscs!",
            (SELECT count(*) FROM blocks b
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND NOT EXISTS (SELECT 1 FROM documents d WHERE d.block_id = b.id))
                                                                    as "blocks_without_documents!",
            (SELECT count(*) FROM courses c
                WHERE ($2 OR c.program_id = ANY($1))
                  AND NOT EXISTS (SELECT 1 FROM blocks b WHERE b.course_id = c.id))
                                                                    as "courses_without_blocks!",
            (SELECT count(*) FROM semesters s
                WHERE ($2 OR s.program_id = ANY($1))
                  AND NOT EXISTS (SELECT 1 FROM courses c WHERE c.semester_id = s.id))
                                                                    as "semesters_without_courses!",

            (SELECT count(*) FROM students st
                WHERE ($2 OR st.program_id = ANY($1)))              as "students_total!",
            (SELECT count(*) FROM students st
                JOIN users u ON u.id = st.user_id
                WHERE ($2 OR st.program_id = ANY($1))
                  AND u.status = 'active')                          as "students_active!",
            (SELECT count(*) FROM students st
                JOIN users u ON u.id = st.user_id
                WHERE ($2 OR st.program_id = ANY($1))
                  AND u.status = 'inactive')                        as "students_inactive!",
            (SELECT count(*) FROM students st
                JOIN users u ON u.id = st.user_id
                WHERE ($2 OR st.program_id = ANY($1))
                  AND u.status = 'suspended')                       as "students_suspended!",
            (SELECT count(*) FROM students st
                WHERE ($2 OR st.program_id = ANY($1))
                  AND st.is_first_login)                            as "students_first_login_pending!",
            (SELECT count(*) FROM students st
                WHERE ($2 OR st.program_id = ANY($1))
                  AND st.created_at >= now() - interval '30 days')  as "students_new_last_30d!",
            (SELECT count(*) FROM students st
                WHERE ($2 OR st.program_id = ANY($1))
                  AND NOT EXISTS (
                      SELECT 1 FROM student_courses sc WHERE sc.student_id = st.id))
                                                                    as "students_without_enrollment!",
            (SELECT count(*) FROM student_courses sc
                JOIN courses c ON c.id = sc.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND sc.status = 'active')                         as "enrollments_active!",
            (SELECT count(*) FROM student_courses sc
                JOIN courses c ON c.id = sc.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND sc.status = 'completed')                      as "enrollments_completed!",
            (SELECT count(*) FROM student_courses sc
                JOIN courses c ON c.id = sc.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND sc.status = 'dropped')                        as "enrollments_dropped!",

            (SELECT count(*) FROM documents d
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1)))               as "documents_total!",
            (SELECT count(*) FROM documents d
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND d.status = 'pending')                         as "documents_pending!",
            (SELECT count(*) FROM documents d
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND d.status = 'parsing')                         as "documents_parsing!",
            (SELECT count(*) FROM documents d
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND d.status = 'pending_review')                  as "documents_pending_review!",
            (SELECT count(*) FROM documents d
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND d.status = 'embedded')                        as "documents_embedded!",
            (SELECT count(*) FROM documents d
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND d.status = 'failed')                          as "documents_failed!",
            (SELECT COALESCE(sum(d.page_count), 0)::bigint FROM documents d
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1)))               as "documents_total_pages!",
            -- NULL, not 0, when nothing has an OCR score: a born-digital-only
            -- corpus has no measured confidence, it does not have zero.
            (SELECT avg(d.ocr_confidence)::float8 FROM documents d
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1)))               as "documents_avg_ocr_confidence?",
            (SELECT count(*) FROM ingestion_jobs j
                JOIN documents d ON d.id = j.document_id
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND j.status = 'pending')                         as "jobs_pending!",
            (SELECT count(*) FROM ingestion_jobs j
                JOIN documents d ON d.id = j.document_id
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND j.status = 'processing')                      as "jobs_processing!",
            (SELECT count(*) FROM ingestion_jobs j
                JOIN documents d ON d.id = j.document_id
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND j.status = 'completed')                       as "jobs_completed!",
            (SELECT count(*) FROM ingestion_jobs j
                JOIN documents d ON d.id = j.document_id
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND j.status = 'failed')                          as "jobs_failed!",
            (SELECT count(*) FROM ingestion_jobs j
                JOIN documents d ON d.id = j.document_id
                JOIN blocks b  ON b.id = d.block_id
                JOIN courses c ON c.id = b.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND j.attempts > 1)                               as "jobs_retried!",

            (SELECT count(*) FROM learning_sessions ls
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1)))               as "sessions_total!",
            (SELECT count(*) FROM learning_sessions ls
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND ls.status = 'in_progress')                    as "sessions_in_progress!",
            (SELECT count(*) FROM learning_sessions ls
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND ls.status = 'completed')                      as "sessions_completed!",
            (SELECT count(*) FROM learning_sessions ls
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND ls.status = 'abandoned')                      as "sessions_abandoned!",
            (SELECT count(*) FROM learning_sessions ls
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND ls.started_at >= now() - interval '7 days')   as "sessions_last_7d!",
            (SELECT COALESCE(sum(ls.active_voice_ms), 0)::bigint FROM learning_sessions ls
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1)))               as "active_voice_ms_total!",
            (SELECT avg(ls.active_voice_ms)::float8 FROM learning_sessions ls
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1)))               as "active_voice_ms_avg?",
            (SELECT count(*) FROM learning_sessions ls
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND ls.end_reason = 'quota')                      as "ended_quota!",
            (SELECT count(*) FROM learning_sessions ls
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND ls.end_reason = 'idle')                       as "ended_idle!",
            (SELECT count(*) FROM learning_sessions ls
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND ls.end_reason = 'user')                       as "ended_user!",
            (SELECT count(*) FROM learning_sessions ls
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND ls.end_reason = 'jailbreak')                  as "ended_jailbreak!",
            (SELECT count(*) FROM learning_sessions ls
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND ls.end_reason = 'error')                      as "ended_error!",

            (SELECT count(*) FROM board_events be
                JOIN learning_sessions ls ON ls.id = be.session_id
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1)))               as "ops_total!",
            (SELECT count(*) FROM board_events be
                JOIN learning_sessions ls ON ls.id = be.session_id
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND be.acked_ms IS NOT NULL)                      as "ops_acked!",
            (SELECT count(*) FROM board_events be
                JOIN learning_sessions ls ON ls.id = be.session_id
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND be.acked_ms IS NULL)                          as "ops_unacked!",
            (SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY be.acked_ms)
                FROM board_events be
                JOIN learning_sessions ls ON ls.id = be.session_id
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND be.acked_ms IS NOT NULL)                      as "ack_p50_ms?",
            (SELECT percentile_cont(0.95) WITHIN GROUP (ORDER BY be.acked_ms)
                FROM board_events be
                JOIN learning_sessions ls ON ls.id = be.session_id
                JOIN courses c ON c.id = ls.course_id
                WHERE ($2 OR c.program_id = ANY($1))
                  AND be.acked_ms IS NOT NULL)                      as "ack_p95_ms?",

            (SELECT count(*) FROM safety_incidents si
                JOIN students st ON st.id = si.student_id
                WHERE ($2 OR st.program_id = ANY($1)))              as "safety_total!",
            (SELECT count(*) FROM safety_incidents si
                JOIN students st ON st.id = si.student_id
                WHERE ($2 OR st.program_id = ANY($1))
                  AND si.tier = 0)                                  as "safety_tier0!",
            (SELECT count(*) FROM safety_incidents si
                JOIN students st ON st.id = si.student_id
                WHERE ($2 OR st.program_id = ANY($1))
                  AND si.tier = 1)                                  as "safety_tier1!",
            (SELECT count(*) FROM safety_incidents si
                JOIN students st ON st.id = si.student_id
                WHERE ($2 OR st.program_id = ANY($1))
                  AND si.tier = 2)                                  as "safety_tier2!",
            (SELECT count(*) FROM safety_incidents si
                JOIN students st ON st.id = si.student_id
                WHERE ($2 OR st.program_id = ANY($1))
                  AND si.created_at >= now() - interval '7 days')   as "safety_last_7d!",
            (SELECT count(*) FROM safety_incidents si
                JOIN students st ON st.id = si.student_id
                WHERE ($2 OR st.program_id = ANY($1))
                  AND si.kind = 'jailbreak')                        as "safety_jailbreak!",
            (SELECT count(*) FROM safety_incidents si
                JOIN students st ON st.id = si.student_id
                WHERE ($2 OR st.program_id = ANY($1))
                  AND si.kind = 'toxicity')                         as "safety_toxicity!",
            (SELECT count(*) FROM safety_incidents si
                JOIN students st ON st.id = si.student_id
                WHERE ($2 OR st.program_id = ANY($1))
                  AND si.kind = 'out_of_scope')                     as "safety_out_of_scope!"
        "#,
        ids,
        unscoped
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)
}

// Each `*_daily` series is gap-filled by `generate_series` LEFT JOINed to the
// grouped data, so it is always exactly 30 rows ending today in UTC, oldest
// first, with explicit zeroes. A chart asked to infer its own missing days
// draws a line that is simply wrong between two sparse points.

async fn sessions_daily(pool: &PgPool, ids: &[Uuid], unscoped: bool) -> Result<Vec<SessionDay>> {
    sqlx::query_as!(
        SessionDay,
        r#"
        SELECT
            g.day::date                  as "day!",
            COALESCE(x.count, 0)         as "count!",
            COALESCE(x.voice_ms, 0)      as "voice_ms!"
        FROM generate_series(
                (now() AT TIME ZONE 'utc')::date - 29,
                (now() AT TIME ZONE 'utc')::date,
                interval '1 day') AS g(day)
        LEFT JOIN (
            SELECT (ls.started_at AT TIME ZONE 'utc')::date        AS day,
                   count(*)                                        AS count,
                   COALESCE(sum(ls.active_voice_ms), 0)::bigint    AS voice_ms
            FROM learning_sessions ls
            JOIN courses c ON c.id = ls.course_id
            WHERE ($2 OR c.program_id = ANY($1))
            GROUP BY 1
        ) x ON x.day = g.day::date
        ORDER BY g.day
        "#,
        ids,
        unscoped
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

async fn students_daily(pool: &PgPool, ids: &[Uuid], unscoped: bool) -> Result<Vec<DayCount>> {
    sqlx::query_as!(
        DayCount,
        r#"
        SELECT g.day::date as "day!", COALESCE(x.count, 0) as "count!"
        FROM generate_series(
                (now() AT TIME ZONE 'utc')::date - 29,
                (now() AT TIME ZONE 'utc')::date,
                interval '1 day') AS g(day)
        LEFT JOIN (
            SELECT (st.created_at AT TIME ZONE 'utc')::date AS day, count(*) AS count
            FROM students st
            WHERE ($2 OR st.program_id = ANY($1))
            GROUP BY 1
        ) x ON x.day = g.day::date
        ORDER BY g.day
        "#,
        ids,
        unscoped
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

async fn documents_daily(pool: &PgPool, ids: &[Uuid], unscoped: bool) -> Result<Vec<DayCount>> {
    sqlx::query_as!(
        DayCount,
        r#"
        SELECT g.day::date as "day!", COALESCE(x.count, 0) as "count!"
        FROM generate_series(
                (now() AT TIME ZONE 'utc')::date - 29,
                (now() AT TIME ZONE 'utc')::date,
                interval '1 day') AS g(day)
        LEFT JOIN (
            SELECT (d.created_at AT TIME ZONE 'utc')::date AS day, count(*) AS count
            FROM documents d
            JOIN blocks b  ON b.id = d.block_id
            JOIN courses c ON c.id = b.course_id
            WHERE ($2 OR c.program_id = ANY($1))
            GROUP BY 1
        ) x ON x.day = g.day::date
        ORDER BY g.day
        "#,
        ids,
        unscoped
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

async fn incidents_daily(pool: &PgPool, ids: &[Uuid], unscoped: bool) -> Result<Vec<DayCount>> {
    sqlx::query_as!(
        DayCount,
        r#"
        SELECT g.day::date as "day!", COALESCE(x.count, 0) as "count!"
        FROM generate_series(
                (now() AT TIME ZONE 'utc')::date - 29,
                (now() AT TIME ZONE 'utc')::date,
                interval '1 day') AS g(day)
        LEFT JOIN (
            SELECT (si.created_at AT TIME ZONE 'utc')::date AS day, count(*) AS count
            FROM safety_incidents si
            JOIN students st ON st.id = si.student_id
            WHERE ($2 OR st.program_id = ANY($1))
            GROUP BY 1
        ) x ON x.day = g.day::date
        ORDER BY g.day
        "#,
        ids,
        unscoped
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

// The three fixed-label series drive their labels from a `VALUES` list rather
// than from the data, so the full set always comes back in the same order with
// zeroes included — a legend whose entries appear and reorder as data arrives
// also changes every series colour underneath the reader.

async fn documents_by_status(
    pool: &PgPool,
    ids: &[Uuid],
    unscoped: bool,
) -> Result<Vec<LabelCount>> {
    sqlx::query_as!(
        LabelCount,
        r#"
        SELECT l.label as "label!", COALESCE(x.count, 0) as "count!"
        FROM (VALUES
                ('pending'::text, 1), ('parsing', 2), ('pending_review', 3),
                ('embedded', 4), ('failed', 5)
             ) AS l(label, ord)
        LEFT JOIN (
            SELECT d.status AS status, count(*) AS count
            FROM documents d
            JOIN blocks b  ON b.id = d.block_id
            JOIN courses c ON c.id = b.course_id
            WHERE ($2 OR c.program_id = ANY($1))
            GROUP BY 1
        ) x ON x.status = l.label
        ORDER BY l.ord
        "#,
        ids,
        unscoped
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

async fn session_end_reasons(
    pool: &PgPool,
    ids: &[Uuid],
    unscoped: bool,
) -> Result<Vec<LabelCount>> {
    sqlx::query_as!(
        LabelCount,
        r#"
        SELECT l.label as "label!", COALESCE(x.count, 0) as "count!"
        FROM (VALUES
                ('quota'::text, 1), ('idle', 2), ('user', 3),
                ('jailbreak', 4), ('error', 5)
             ) AS l(label, ord)
        LEFT JOIN (
            SELECT ls.end_reason AS reason, count(*) AS count
            FROM learning_sessions ls
            JOIN courses c ON c.id = ls.course_id
            WHERE ($2 OR c.program_id = ANY($1))
              AND ls.end_reason IS NOT NULL
            GROUP BY 1
        ) x ON x.reason = l.label
        ORDER BY l.ord
        "#,
        ids,
        unscoped
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Buckets are cut at the NN-1 `HOLD_MAX` ceiling: anything in `>400ms` was
/// released without an ACK, i.e. a whiteboard-first violation, so the bucket
/// must exist in the series even when it is empty for the UI to say so.
async fn ack_latency_buckets(
    pool: &PgPool,
    ids: &[Uuid],
    unscoped: bool,
) -> Result<Vec<LabelCount>> {
    sqlx::query_as!(
        LabelCount,
        r#"
        SELECT l.label as "label!", COALESCE(x.count, 0) as "count!"
        FROM (VALUES
                ('0-100ms'::text, 1), ('100-250ms', 2), ('250-400ms', 3), ('>400ms', 4)
             ) AS l(label, ord)
        LEFT JOIN (
            SELECT CASE
                     WHEN be.acked_ms < 100 THEN 1
                     WHEN be.acked_ms < 250 THEN 2
                     WHEN be.acked_ms < 400 THEN 3
                     ELSE 4
                   END                       AS ord,
                   count(*)                  AS count
            FROM board_events be
            JOIN learning_sessions ls ON ls.id = be.session_id
            JOIN courses c ON c.id = ls.course_id
            WHERE ($2 OR c.program_id = ANY($1))
              AND be.acked_ms IS NOT NULL
            GROUP BY 1
        ) x ON x.ord = l.ord
        ORDER BY l.ord
        "#,
        ids,
        unscoped
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// The ten busiest blocks by session count. Unlike the fixed-label series this
/// one is data-driven, so it is legitimately empty before any session exists.
async fn top_blocks(pool: &PgPool, ids: &[Uuid], unscoped: bool) -> Result<Vec<LabelCount>> {
    sqlx::query_as!(
        LabelCount,
        r#"
        SELECT
            ('Block ' || b.block_no || ' - ' || b.title) as "label!",
            count(*)                                      as "count!"
        FROM learning_sessions ls
        JOIN blocks b  ON b.id = ls.block_id
        JOIN courses c ON c.id = ls.course_id
        WHERE ($2 OR c.program_id = ANY($1))
        GROUP BY b.id, b.block_no, b.title
        ORDER BY count(*) DESC, b.block_no
        LIMIT 10
        "#,
        ids,
        unscoped
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}
