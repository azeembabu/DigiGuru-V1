//! `ingestion_jobs` — the Phase 2 ingestion outbox (`migrations/0003_ingestion_jobs.sql`).
//!
//! The worker's contract with this table:
//!
//! 1. [`claim_next`] atomically moves one `pending` job to `processing` and
//!    returns it, together with everything the pipeline needs about the
//!    document (its storage key, and the `program_id`/`semester`/`course_id`/
//!    `block_no` that every chunk must be tagged with).
//! 2. The worker runs the pipeline, then calls [`complete`] or [`fail`].
//!
//! `FOR UPDATE SKIP LOCKED` is what makes step 1 safe to run from more than
//! one worker: concurrent claimers skip rows another transaction already
//! holds rather than blocking on them, so two workers never pick up the same
//! document and no worker serialises behind another's row lock.

use chrono::{DateTime, Utc};
use dg_core::{BlockId, CourseId, DocumentId, ProgramId};
use sqlx::PgPool;

use crate::error::{Error, Result};

/// A claimed job plus the document context the ingestion pipeline needs.
///
/// The academic ids come from the `documents -> blocks -> courses ->
/// semesters` join rather than from the PDF: they are what every chunk is
/// tagged with, and what the mandatory retrieval filter matches on.
#[derive(Debug, Clone)]
pub struct ClaimedJob {
    pub job_id: uuid::Uuid,
    pub document_id: DocumentId,
    pub attempts: i32,

    pub title: String,
    pub storage_key: String,
    pub sha256: String,

    pub block_id: BlockId,
    pub block_no: i16,
    pub course_id: CourseId,
    pub program_id: ProgramId,
    pub semester_no: i16,
}

/// Atomically claims the oldest `pending` job, marking it `processing`.
///
/// Returns `None` when the queue is empty. The `UPDATE ... FROM (SELECT ...
/// FOR UPDATE SKIP LOCKED)` form does the select and the state transition in
/// a single statement, so there is no window between "chose a job" and
/// "marked it taken" in which a second worker could choose the same one.
///
/// `attempts` is incremented **here, at claim time**, not on failure. If it
/// were incremented only when a job fails cleanly, a worker that crashed or
/// was killed mid-document would leave the row at `processing` with its
/// original count, and a requeue would retry it forever — the poison-document
/// loop `max_attempts` exists to stop. Counting attempts when the work is
/// handed out makes the count survive a crash.
pub async fn claim_next(pool: &PgPool) -> Result<Option<ClaimedJob>> {
    sqlx::query_as!(
        ClaimedJob,
        r#"
        WITH next AS (
            SELECT id FROM ingestion_jobs
            WHERE status = 'pending'
            ORDER BY created_at
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        ),
        claimed AS (
            UPDATE ingestion_jobs j
            SET status = 'processing', attempts = j.attempts + 1, updated_at = now()
            FROM next
            WHERE j.id = next.id
            RETURNING j.id, j.document_id, j.attempts
        )
        SELECT
            claimed.id as "job_id!",
            claimed.document_id as "document_id!: DocumentId",
            claimed.attempts as "attempts!",
            d.title as "title!",
            d.storage_key as "storage_key!",
            d.sha256 as "sha256!",
            d.block_id as "block_id!: BlockId",
            b.block_no as "block_no!",
            c.id as "course_id!: CourseId",
            c.program_id as "program_id!: ProgramId",
            s.semester_number as "semester_no!"
        FROM claimed
        JOIN documents d ON d.id = claimed.document_id
        JOIN blocks    b ON b.id = d.block_id
        JOIN courses   c ON c.id = b.course_id
        JOIN semesters s ON s.id = c.semester_id
"#
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Marks a claimed job `completed` and clears any error from a prior attempt.
pub async fn complete(pool: &PgPool, job_id: uuid::Uuid) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE ingestion_jobs
        SET status = 'completed', last_error = NULL, updated_at = now()
        WHERE id = $1
        "#,
        job_id
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}

/// Records a failed attempt.
///
/// `retryable` decides whether the job goes back to `pending` for another
/// worker to pick up, or terminates at `failed`. A poison document — one that
/// fails deterministically, e.g. a corrupt PDF — must land at `failed` rather
/// than cycling back to `pending` forever; the worker decides that by
/// comparing `attempts` against its retry ceiling.
pub async fn fail(
    pool: &PgPool,
    job_id: uuid::Uuid,
    error: &str,
    retryable: bool,
) -> Result<()> {
    let status = if retryable { "pending" } else { "failed" };
    sqlx::query!(
        r#"
        UPDATE ingestion_jobs
        SET status = $2::ingestion_job_status, last_error = $3, updated_at = now()
        WHERE id = $1
        "#,
        job_id,
        status as _,
        error
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}

/// Requeues jobs stuck in `processing` past `stale_after_seconds`.
///
/// A worker killed mid-document leaves its row `processing` with nothing to
/// ever move it again — the job would be silently lost, and the document
/// would sit at `parsing` forever while the tutor abstains on it (NN-4).
/// `attempts` was already incremented at claim time, so a document that
/// crashes the worker repeatedly still runs out of attempts rather than
/// looping.
pub async fn requeue_stale(pool: &PgPool, stale_after_seconds: i64) -> Result<u64> {
    let result = sqlx::query!(
        r#"
        UPDATE ingestion_jobs
        SET status = 'pending',
            last_error = 'worker did not finish; requeued as stale',
            updated_at = now()
        WHERE status = 'processing'
          AND updated_at < now() - make_interval(secs => $1::double precision)
        "#,
        stale_after_seconds as f64
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(result.rows_affected())
}

/// One row of the queue's current state, for the worker's startup log.
#[derive(Debug, Clone)]
pub struct QueueDepth {
    pub status: String,
    pub count: i64,
}

pub async fn queue_depth(pool: &PgPool) -> Result<Vec<QueueDepth>> {
    sqlx::query_as!(
        QueueDepth,
        r#"
        SELECT status::text as "status!", count(*) as "count!"
        FROM ingestion_jobs
        GROUP BY status
        ORDER BY status
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// `documents` rows whose `sha256` already appears on another document.
///
/// The column is `UNIQUE`, so the upload path cannot create one — this exists
/// so the worker can assert the invariant it depends on before indexing
/// (`rag-pipeline.md`: "Deduplicate documents by `sha256`; re-uploading the
/// same PDF must not double the corpus") rather than trusting it silently.
pub async fn duplicate_sha256(pool: &PgPool, sha256: &str, exclude: DocumentId) -> Result<bool> {
    let row = sqlx::query!(
        r#"SELECT EXISTS(
             SELECT 1 FROM documents WHERE sha256 = $1 AND id <> $2
           ) as "exists!""#,
        sha256,
        exclude.into_uuid()
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(row.exists)
}

/// Timestamp helper kept for callers that log claim latency.
pub fn now() -> DateTime<Utc> {
    Utc::now()
}
