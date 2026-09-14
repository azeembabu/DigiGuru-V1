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

/// What one pass of `process_job` decided, so the loop can react to a rate
/// limit without inspecting errors again.
enum JobOutcome {
    /// Handled — completed, or failed in a way already recorded.
    Done,
    /// The provider is rate limiting. The job was returned to the queue
    /// untouched; the loop must wait before claiming anything else.
    RateLimited,
}

/// How long to stand down after the embedding provider returns a rate limit.
///
/// Long enough that a per-minute quota has actually rolled over; retrying
/// sooner just collects another 429 and makes the log harder to read.
const RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(65);

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

    // Same REST-vs-gRPC port distinction as the gateway; see `rag::qdrant::grpc_url`.
    let qdrant = qdrant_client::Qdrant::from_url(&rag::qdrant::grpc_url(&qdrant_url))
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

    // Said once, at startup, rather than discovered weeks later from a screen
    // full of blank cards. Not fatal: covers are decoration, and a worker
    // without pdfium ingests everything exactly as it did before.
    if rag::thumbnail::Renderer::available() {
        tracing::info!("unit cover rendering is enabled");
    } else {
        tracing::info!(
            "unit covers will not be rendered (no pdfium library); \
             units will show a generated placeholder instead"
        );
    }

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

        let outcome =
            process_job(&pool, &qdrant, &mut redis, &parser, embedder.as_ref(), job, max_attempts)
                .await?;

        // A rate limit is the provider asking for time, not a reason to spin:
        // claiming the next job immediately would collect another 429 and,
        // before this existed, burn that document's retry budget too.
        if matches!(outcome, JobOutcome::RateLimited) {
            tokio::select! {
                _ = &mut shutdown => {
                    tracing::info!("shutdown signal received; exiting");
                    return Ok(());
                }
                _ = tokio::time::sleep(RATE_LIMIT_COOLDOWN) => {}
            }
        }
    }
}

/// Chooses the embedder. Real Gemini embeddings need `GEMINI_API_KEY`; with
/// none set the deterministic stub runs instead, and says so loudly — a
/// corpus embedded with the stub is *not* semantically searchable, so this
/// must never be mistaken for a working production ingest.
fn select_embedder() -> Box<dyn Embedder> {
    match std::env::var("GEMINI_API_KEY") {
        Ok(key) if !key.trim().is_empty() => {
            // `for_documents`: ingestion stores passages, so it must use the
            // RETRIEVAL_DOCUMENT task type. The query side uses RETRIEVAL_QUERY.
            // Mixing them silently degrades retrieval with no error anywhere.
            match rag::embed::GeminiEmbedder::for_documents(key) {
                Ok(embedder) => {
                    tracing::info!("using the Gemini embedder (RETRIEVAL_DOCUMENT)");
                    Box::new(embedder)
                }
                Err(err) => {
                    tracing::error!(%err, "could not build the Gemini embedder; falling back to the stub");
                    Box::new(rag::embed::StubEmbedder)
                }
            }
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
) -> Result<JobOutcome> {
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
            // The page count and OCR score come from the parse and are written
            // with the status, not discarded: they are what the admin console
            // and the student unit list render as "N pages", and leaving them
            // at 0 made a fully ingested document look unprepared.
            documents::finish_ingestion(
                pool,
                job.document_id,
                outcome.status.as_db_status(),
                i32::try_from(outcome.page_count).unwrap_or(i32::MAX),
                outcome.ocr_confidence,
            )
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
        Err(err) if is_rate_limited(&err) => {
            // Not a failure of this document. Put it back untouched and tell
            // the caller to wait before claiming anything else — retrying
            // immediately just spends the next attempt on the same 429.
            let detail = err.to_string();
            documents::set_status(pool, job.document_id, "pending")
                .await
                .context("returning the document to pending after a rate limit")?;
            ingestion_jobs::requeue_without_attempt(pool, job.job_id, &detail)
                .await
                .context("requeueing the job after a rate limit")?;

            tracing::warn!(
                document_id = %job.document_id,
                cooldown_s = RATE_LIMIT_COOLDOWN.as_secs(),
                "embedding provider is rate limiting; requeued without charging an attempt"
            );
            return Ok(JobOutcome::RateLimited);
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

    Ok(JobOutcome::Done)
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

    // Render the cover before the pipeline rather than after it. A document
    // that stalls at `pending_review` or fails to embed is still listed to the
    // student (`.claude/rules/api-conventions.md`: not-ready units are marked,
    // not hidden), and it is exactly those units a student most needs to
    // recognise. Deliberately not `?` — see `store_cover`.
    store_cover(pool, job, &bytes).await;

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

/// Render page 1 to a cover image and record where it went.
///
/// **Never fails the ingestion.** A cover is how a student recognises a unit in
/// a list; it has no bearing on whether the tutor can teach from it, so every
/// way this can go wrong — no pdfium library in this build, an unrenderable
/// PDF, a full disk — is logged and swallowed. Returning an error here would
/// mean a document that chunks, embeds and teaches perfectly is marked
/// `failed` because its picture could not be drawn.
///
/// The rasterising itself is CPU-bound and goes through `spawn_blocking`
/// (`.claude/rules/code-style.md`), which is also where the non-`Send` pdfium
/// binding is allowed to live.
async fn store_cover(pool: &sqlx::PgPool, job: &ingestion_jobs::ClaimedJob, bytes: &[u8]) {
    // Beside the PDF, under the same name: the upload directory is the
    // gateway's to choose (`admin/documents.rs`), and deriving the path keeps
    // the worker from having to know or re-configure it.
    let key = cover_key(&job.storage_key);

    let owned = bytes.to_vec();
    let rendered = match tokio::task::spawn_blocking(move || {
        rag::thumbnail::Renderer::first_page_webp(&owned)
    })
    .await
    {
        Ok(Ok(Some(image))) => image,
        Ok(Ok(None)) => return, // No engine, or nothing to draw. Already logged.
        Ok(Err(err)) => {
            tracing::warn!(
                document_id = %job.document_id,
                error = %err,
                "could not render a cover for this unit; ingestion continues without one"
            );
            return;
        }
        Err(err) => {
            tracing::warn!(document_id = %job.document_id, error = %err, "cover render panicked");
            return;
        }
    };

    if let Err(err) = tokio::fs::write(&key, &rendered).await {
        tracing::warn!(document_id = %job.document_id, error = %err, "writing the cover failed");
        return;
    }

    // Only now is the row pointed at the file, so `thumbnail_key` never names
    // something that is not on disk.
    if let Err(err) = documents::set_thumbnail_key(pool, job.document_id, &key).await {
        tracing::warn!(document_id = %job.document_id, error = %err, "recording the cover failed");
        return;
    }

    tracing::info!(document_id = %job.document_id, bytes = rendered.len(), "unit cover rendered");
}

/// The cover's path, derived from the PDF's: same directory, same stem,
/// `.webp` instead of `.pdf`.
fn cover_key(storage_key: &str) -> String {
    match storage_key.rsplit_once('.') {
        Some((stem, _ext)) => format!("{stem}.webp"),
        // No extension to swap — append rather than guess, so two documents
        // still cannot collide on a cover.
        None => format!("{storage_key}.webp"),
    }
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

/// Is this the embedding provider telling us to slow down?
///
/// Matched on the message rather than a typed variant because the rate limit
/// arrives as an HTTP status inside the provider's error body, and `RagError`
/// deliberately does not model every upstream's status codes.
///
/// It matters because a rate limit is neither of the two categories the worker
/// had. It is not permanent — the PDF is fine and will ingest happily in a
/// minute — but treating it as an ordinary transient failure burned all three
/// attempts in under a second and marked the document `failed` forever. So it
/// is a third case: requeue, do not charge an attempt, and wait.
fn is_rate_limited(err: &RagError) -> bool {
    let text = err.to_string();
    text.contains("429")
        || text.contains("Too Many Requests")
        || text.contains("RESOURCE_EXHAUSTED")
}

#[cfg(test)]
mod tests {
    use super::cover_key;

    #[test]
    fn cover_sits_beside_its_pdf() {
        assert_eq!(cover_key("./uploads/abc123.pdf"), "./uploads/abc123.webp");
    }

    /// The stem is what keeps two documents apart, so it must survive a
    /// directory name that itself contains a dot.
    #[test]
    fn a_dotted_directory_does_not_swallow_the_stem() {
        assert_eq!(
            cover_key("/srv/dg.data/uploads/abc123.pdf"),
            "/srv/dg.data/uploads/abc123.webp"
        );
    }

    #[test]
    fn an_extensionless_path_gains_one_rather_than_losing_its_name() {
        assert_eq!(cover_key("/uploads/abc123"), "/uploads/abc123.webp");
    }
}
