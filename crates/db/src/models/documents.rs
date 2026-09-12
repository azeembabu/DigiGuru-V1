//! `documents` — query module for the ingested-PDF table (Phase 2).
//!
//! The `Document` struct itself lives in `content.rs` (added before Phase 2
//! landed, per that module's doc comment); this module owns the queries
//! against it, following the pattern in `students.rs`.

use chrono::Utc;
use dg_core::DocumentId;
use sqlx::PgPool;

use crate::error::{Error, Result};
use crate::models::content::Document;

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
