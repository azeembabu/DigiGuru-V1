//! PDF ingestion worker (`IMPLEMENTATION_PLAN.md` §5.1).
//!
//! Drains the `ingestion_jobs` outbox: claim a pending job, run the document
//! through parse -> clean -> chunk -> enrich -> embed -> Qdrant upsert ->
//! prompt cache, then record the terminal state.
//!
//! ## Concurrency
//!
//! Jobs are claimed with `FOR UPDATE SKIP LOCKED` (see
//! `dg_db::models::ingestion_jobs::claim_next`), so any number of workers can
//! run against the same database without two of them picking up the same
//! document and without one serialising behind another's row lock.
//!
//! ## Retries
//!
//! `attempts` is incremented at claim time and compared against
//! `INGEST_MAX_ATTEMPTS`. Failures are classified: a *permanent* failure (a
//! corrupt PDF, a chunk missing mandatory metadata) goes straight to `failed`
//! without burning the retry budget, because retrying it would produce the
//! same result forever. A *transient* failure (Qdrant or Redis unreachable)
//! returns the job to `pending` until the budget runs out. Either way a
//! poison document terminates rather than looping.
//!
//! ## Usage
//!
//! ```text
//! ingest-worker              # poll forever
//! ingest-worker --once       # drain the queue, then exit (CI / one-shot)
//! ingest-worker --dry-run    # claim nothing; just report queue depth
//! ```

use std::time::Duration;

use anyhow::{Context, Result};
use dg_db::models::{documents, ingestion_jobs};
use rag::embed::{Embedder, StubEmbedder};
use rag::ingest::{self, IngestContext};
use rag::parse::{LopdfParser, PdfParser};
use rag::RagError;

/// How long to wait before polling again when the queue is empty.
const IDLE_POLL_INTERVAL: Duration = Duration::from_secs(5);

/// A job left `processing` for longer than this is assumed to belong to a
/// worker that died, and is requeued.
const STALE_JOB_SECONDS: i64 = 900;

/// Default retry ceiling for *transient* failures.
const DEFAULT_MAX_ATTEMPTS: i32 = 3;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let once = args.iter().any(|a| a == "--once");
    let dry_run = args.iter().any(|a| a == "--dry-run");

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL is not set")?;
    let qdrant_url =
        std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".to_string());
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let max_attempts: i32 = std::env::var("INGEST_MAX_ATTEMPTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_ATTEMPTS);

    let pool = dg_db::create_pool(&database_url)
        .await
        .context("connecting to Postgres")?;

    for row in ingestion_jobs::queue_depth(&pool)
        .await
        .context("reading queue depth")?
    {
        tracing::info!(status = %row.status, count = row.count, "ingestion queue");
    }

    if dry_run {
        tracing::info!("--dry-run: claiming nothing, exiting");
        return Ok(());
    }

    let qdrant = qdrant_client::Qdrant::from_url(&qdrant_url)
        .build()
        .context("building the Qdrant client")?;
    let redis_client = redis::Client::open(redis_url).context("opening the Redis client")?;
    let mut redis = redis::aio::ConnectionManager::new(redis_client)
        .await
        .context("connecting to Redis")?;

    // Both are injected into the pipeline, so swapping in a pdfium+OCR parser
    // or the real Gemini embedder is a change here, not in the pipeline.
    let parser = LopdfParser;
    let embedder = select_embedder();

    let requeued = ingestion_jobs::requeue_stale(&pool, STALE_JOB_SECONDS)
        .await
        .context("requeueing stale jobs")?;
    if requeued > 0 {
        tracing::warn!(requeued, "requeued jobs abandoned by a previous worker");
    }

    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        let claimed = ingestion_jobs::claim_next(&pool)
            .await
            .context("claiming a job")?;

        let Some(job) = claimed else {
            if once {
                tracing::info!("queue drained; exiting (--once)");
                return Ok(());
            }
            tokio::select! {
                _ = &mut shutdown => {
                    tracing::info!("shutdown signal received; exiting");
                    return Ok(());
                }
                _ = tokio::time::sleep(IDLE_POLL_INTERVAL) => continue,
            }
        };

        process_job(&pool, &qdrant, &mut redis, &parser, embedder.as_ref(), job, max_attempts)
            .await?;
    }
}

/// Chooses the embedder. Real Gemini embeddings need `GEMINI_API_KEY`; with
/// none set the deterministic stub runs instead, and says so loudly — a
/// corpus embedded with the stub is *not* semantically searchable, so this
/// must never be mistaken for a working production ingest.
fn select_embedder() -> Box<dyn Embedder> {
    match std::env::var("GEMINI_API_KEY") {
        Ok(key) if !key.trim().is_empty() => {
            tracing::info!("using the Gemini embedder");
            Box::new(rag::embed::GeminiEmbedder { api_key: key })
        }
        _ => {
            tracing::warn!(
                "GEMINI_API_KEY is not set — using the DETERMINISTIC STUB EMBEDDER. \
                 Chunks will be indexed with meaningless vectors: the pipeline is \
                 exercised end to end, but retrieval quality is not. Re-ingest with a \
                 real key before trusting any retrieval result."
            );
            Box::new(StubEmbedder)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn process_job(
    pool: &sqlx::PgPool,
    qdrant: &qdrant_client::Qdrant,
    redis: &mut redis::aio::ConnectionManager,
    parser: &(dyn PdfParser + 'static),
    embedder: &dyn Embedder,
    job: ingestion_jobs::ClaimedJob,
    max_attempts: i32,
) -> Result<()> {
    // Log identifiers only, never student PII (`security.md`). A document
    // title is admin-authored curriculum metadata, not student data.
    tracing::info!(
        job_id = %job.job_id,
        document_id = %job.document_id,
        attempt = job.attempts,
        "claimed ingestion job"
    );

    documents::set_status(pool, job.document_id, "parsing")
        .await
        .context("marking the document as parsing")?;

    match run_one(pool, qdrant, redis, parser, embedder, &job).await {
        Ok(outcome) => {
            documents::set_status(pool, job.document_id, outcome.status.as_db_status())
                .await
                .context("recording the terminal document status")?;
            ingestion_jobs::complete(pool, job.job_id)
                .await
                .context("completing the job")?;

            for warning in &outcome.warnings {
                tracing::warn!(document_id = %job.document_id, "{warning}");
            }
            tracing::info!(
                document_id = %job.document_id,
                status = outcome.status.as_db_status(),
                chunks = outcome.chunk_count,
                pages = outcome.page_count,
                ocr_confidence = ?outcome.ocr_confidence,
                preamble = ?outcome.preamble_handle,
                "ingestion complete"
            );
        }
        Err(err) => {
            let permanent = is_permanent(&err);
            let budget_exhausted = job.attempts >= max_attempts;
            let retryable = !permanent && !budget_exhausted;

            // The full error detail goes to the internal log and to
            // `ingestion_jobs.last_error`, which only an admin reads; it never
            // reaches a student (`security.md` error exposure).
            let detail = err.to_string();

            documents::set_status(pool, job.document_id, if retryable { "pending" } else { "failed" })
                .await
                .context("recording the failed document status")?;
            ingestion_jobs::fail(pool, job.job_id, &detail, retryable)
                .await
                .context("recording the job failure")?;

            if retryable {
                tracing::warn!(
                    document_id = %job.document_id,
                    attempt = job.attempts,
                    max_attempts,
                    "ingestion failed transiently; requeued: {detail}"
                );
            } else {
                tracing::error!(
                    document_id = %job.document_id,
                    attempt = job.attempts,
                    permanent,
                    "ingestion failed permanently: {detail}"
                );
            }
        }
    }

    Ok(())
}

async fn run_one(
    pool: &sqlx::PgPool,
    qdrant: &qdrant_client::Qdrant,
    redis: &mut redis::aio::ConnectionManager,
    parser: &(dyn PdfParser + 'static),
    embedder: &dyn Embedder,
    job: &ingestion_jobs::ClaimedJob,
) -> std::result::Result<rag::ingest::IngestOutcome, RagError> {
    // `documents.sha256` is UNIQUE, so this should be impossible; assert it
    // anyway rather than trusting it, since a duplicate would double the
    // corpus (`rag-pipeline.md` ingestion rule 6).
    if ingestion_jobs::duplicate_sha256(pool, &job.sha256, job.document_id).await? {
        return Err(RagError::Parse(format!(
            "another document already has sha256 {}; refusing to index a duplicate",
            job.sha256
        )));
    }

    let bytes = tokio::fs::read(&job.storage_key)
        .await
        .map_err(|e| RagError::Parse(format!("reading {}: {e}", job.storage_key)))?;

    // The stored file must still be the file that was hashed at upload.
    ingest::verify_sha256(&bytes, &job.sha256)?;

    let ctx = IngestContext {
        document_id: job.document_id,
        document_title: job.title.clone(),
        block_id: job.block_id,
        block_no: job.block_no,
        course_id: job.course_id,
        program_id: job.program_id,
        semester_no: job.semester_no,
    };

    ingest::run_pipeline(qdrant, redis, parser, embedder, &ctx, bytes).await
}

/// Whether a failure will recur identically on every retry.
///
/// A corrupt PDF, an empty document, or a chunk missing mandatory metadata is
/// a property of the document itself — retrying burns the budget and ends in
/// the same place, so these terminate immediately. Qdrant, Redis, and
/// database failures are infrastructure and may well succeed next time.
fn is_permanent(err: &RagError) -> bool {
    matches!(
        err,
        RagError::Parse(_) | RagError::Chunk(_) | RagError::MissingMetadata { .. }
    )
}
