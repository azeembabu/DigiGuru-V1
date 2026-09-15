//! `documents` — query module for the ingested-PDF table (Phase 2).
//!
//! The `Document` struct itself lives in `content.rs` (added before Phase 2
//! landed, per that module's doc comment); this module owns the queries
//! against it, following the pattern in `students.rs`.

use chrono::{DateTime, Utc};
use dg_core::{BlockId, DocumentId, UserId};
use sqlx::PgPool;

use crate::error::{Error, Result};
use crate::models::content::Document;

/// The document's title, if it belongs to that block.
///
/// Validates a client-supplied unit id before it is allowed to narrow
/// retrieval, and returns the title in the same round trip because the tutor
/// needs it: a prompt that says "you are teaching Unit 3 Forest Resources" lets
/// the model place itself, and one that says nothing leaves it guessing which
/// of six units the retrieved paragraphs came from.
///
/// `None` means "not in this block" — which the caller treats as "teach the
/// whole block" rather than as an error.
pub async fn title_within_block(
    pool: &PgPool,
    document_id: DocumentId,
    block_id: BlockId,
) -> Result<Option<String>> {
    sqlx::query_scalar!(
        r#"SELECT title FROM documents WHERE id = $1 AND block_id = $2"#,
        document_id.into_uuid(),
        block_id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn find_by_id(pool: &PgPool, id: DocumentId) -> Result<Option<Document>> {
    sqlx::query_as!(
        Document,
        r#"
        SELECT id, block_id, uploaded_by, title, storage_key, sha256, page_count,
               ocr_confidence, status, created_at
        FROM documents WHERE id = $1
        "#,
        id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Set `documents.status` (`pending|parsing|pending_review|embedded|failed`).
/// Kept as a plain `&str` rather than a Rust enum, matching the `Document`
/// struct's own field (see `content.rs`).
pub async fn set_status(pool: &PgPool, id: DocumentId, status: &str) -> Result<()> {
    sqlx::query!(
        r#"UPDATE documents SET status = $2 WHERE id = $1"#,
        id.into_uuid(),
        status
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}

/// Records the terminal status of an ingestion **together with what the parse
/// measured**.
///
/// `set_status` alone was the bug: the worker computed `page_count` and
/// `ocr_confidence`, logged both, and then wrote neither, so every successfully
/// ingested document sat at `page_count = 0`. Students saw "0 pages · still
/// being prepared" beside a unit the tutor could already teach from, and an
/// admin had no way to tell a parsed 60-page PDF from an empty one.
///
/// `ocr_confidence` is `Option` because a born-digital PDF has no OCR score at
/// all — that is a different fact from "scored zero", so it stays NULL rather
/// than being flattened to a number.
pub async fn finish_ingestion(
    pool: &PgPool,
    id: DocumentId,
    status: &str,
    page_count: i32,
    ocr_confidence: Option<f32>,
) -> Result<()> {
    sqlx::query!(
        r#"UPDATE documents
              SET status = $2, page_count = $3, ocr_confidence = $4
            WHERE id = $1"#,
        id.into_uuid(),
        status,
        page_count,
        ocr_confidence
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}

/// Mark ingestion complete: `documents.status = 'embedded'`.
pub async fn mark_embedded(pool: &PgPool, id: DocumentId) -> Result<()> {
    set_status(pool, id, "embedded").await
}

/// Mark ingestion failed and record the reason on the owning
/// `ingestion_jobs` row (`documents` itself has no `last_error` column —
/// that detail lives on the outbox row, one per attempt).
pub async fn mark_failed(pool: &PgPool, id: DocumentId) -> Result<()> {
    set_status(pool, id, "failed").await
}

/// Update the most recent `ingestion_jobs` row for this document to
/// `completed`.
pub async fn complete_latest_job(pool: &PgPool, document_id: DocumentId) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE ingestion_jobs
        SET status = 'completed', updated_at = now()
        WHERE id = (
            SELECT id FROM ingestion_jobs
            WHERE document_id = $1
            ORDER BY created_at DESC
            LIMIT 1
        )
        "#,
        document_id.into_uuid()
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}

/// Update the most recent `ingestion_jobs` row for this document to
/// `failed`, incrementing `attempts` and recording `last_error`.
pub async fn fail_latest_job(pool: &PgPool, document_id: DocumentId, error: &str) -> Result<()> {
    let now = Utc::now();
    sqlx::query!(
        r#"
        UPDATE ingestion_jobs
        SET status = 'failed', attempts = attempts + 1, last_error = $2, updated_at = $3
        WHERE id = (
            SELECT id FROM ingestion_jobs
            WHERE document_id = $1
            ORDER BY created_at DESC
            LIMIT 1
        )
        "#,
        document_id.into_uuid(),
        error,
        now
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?;
    Ok(())
}

/// One row of the admin ingestion-status view: the `documents` columns an
/// operator needs, plus the status and error of that document's most recent
/// `ingestion_jobs` attempt.
///
/// `storage_key` is deliberately **absent**: it is a server-side filesystem
/// path (`admin/documents.rs`'s `UPLOAD_DIR`), and `.claude/rules/security.md`
/// keeps internal paths off the wire. Every other column of the row is here.
/// `documents` itself has no error column — the failure reason lives on the
/// outbox row, one per attempt — hence the lateral join.
#[derive(Debug, Clone)]
pub struct DocumentSummary {
    pub id: DocumentId,
    pub block_id: BlockId,
    pub uploaded_by: UserId,
    pub title: String,
    pub sha256: String,
    pub page_count: i32,
    pub ocr_confidence: Option<f32>,
    /// `pending|parsing|pending_review|embedded|failed`.
    pub status: String,
    pub created_at: DateTime<Utc>,
    /// `pending|processing|completed|failed` of the latest ingestion attempt,
    /// `None` if no job row was ever enqueued.
    pub job_status: Option<String>,
    /// The latest attempt's failure reason, `None` unless it failed.
    pub last_error: Option<String>,
    /// Optional introductory video shown before the tutoring session.
    /// `None` — the normal shape — means the classroom opens straight onto the
    /// interactive discussion.
    pub video_url: Option<String>,
}

pub async fn list_summaries_by_block(
    pool: &PgPool,
    block_id: BlockId,
) -> Result<Vec<DocumentSummary>> {
    sqlx::query_as!(
        DocumentSummary,
        r#"
        SELECT d.id as "id: DocumentId", d.block_id as "block_id: BlockId",
               d.uploaded_by as "uploaded_by: UserId", d.title, d.sha256,
               d.page_count, d.ocr_confidence, d.status, d.created_at,
               j.status::text as "job_status?", j.last_error as "last_error?",
               d.video_url
        FROM documents d
        LEFT JOIN LATERAL (
            SELECT status, last_error
            FROM ingestion_jobs
            WHERE document_id = d.id
            ORDER BY created_at DESC
            LIMIT 1
        ) j ON TRUE
        WHERE d.block_id = $1
        ORDER BY d.created_at DESC
        "#,
        block_id.into_uuid()
    )
    .fetch_all(pool)
    .await
    .map_err(Error::from_sqlx)
}

pub async fn find_summary_by_id(
    pool: &PgPool,
    id: DocumentId,
) -> Result<Option<DocumentSummary>> {
    sqlx::query_as!(
        DocumentSummary,
        r#"
        SELECT d.id as "id: DocumentId", d.block_id as "block_id: BlockId",
               d.uploaded_by as "uploaded_by: UserId", d.title, d.sha256,
               d.page_count, d.ocr_confidence, d.status, d.created_at,
               j.status::text as "job_status?", j.last_error as "last_error?",
               d.video_url
        FROM documents d
        LEFT JOIN LATERAL (
            SELECT status, last_error
            FROM ingestion_jobs
            WHERE document_id = d.id
            ORDER BY created_at DESC
            LIMIT 1
        ) j ON TRUE
        WHERE d.id = $1
        "#,
        id.into_uuid()
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)
}

/// Set or clear the unit's introductory video.
///
/// `None` clears it, which is the same state a unit that never had one is in —
/// the classroom then routes straight to the interactive discussion. The URL is
/// normalised by the caller before it reaches here, so what is stored is always
/// a canonical watch URL or nothing.
pub async fn set_video_url(pool: &PgPool, id: DocumentId, video_url: Option<&str>) -> Result<bool> {
    let affected = sqlx::query!(
        r#"UPDATE documents SET video_url = $2 WHERE id = $1"#,
        id.into_uuid(),
        video_url
    )
    .execute(pool)
    .await
    .map_err(Error::from_sqlx)?
    .rows_affected();
    Ok(affected > 0)
}
