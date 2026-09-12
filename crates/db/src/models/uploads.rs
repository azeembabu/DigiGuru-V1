//! DB queries backing the Phase 2 document-upload endpoint
//! (`apps/gateway/src/admin/documents.rs`): sha256 dedupe lookup, the
//! `documents` insert, and the `ingestion_jobs` outbox insert.
//!
//! Deliberately **not** named `documents.rs`: the parallel ingestion agent
//! owns `crates/db/src/models/documents.rs` for the full parse/chunk/embed
//! pipeline's query surface (status transitions, parse metadata, etc.). This
//! module exists so the upload endpoint has something to call without
//! touching that file. Once the ingestion-side module lands, the two should
//! likely be merged — flagged as a judgment call in the task report.
//!
//! `insert_ingestion_job` depends on the `ingestion_jobs` table defined in
//! `migrations/0003_ingestion_jobs.sql`, which is owned by the parallel
//! ingestion agent and may not exist yet in this worktree. This will not
//! compile (`sqlx::query!` is compile-time checked) until that migration is
//! present and the database/offline cache is refreshed. Documented
//! assumption — see the task report.

use dg_core::{BlockId, DocumentId, ProgramId, UserId};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Error, Result};

/// The `program_id` that owns `block_id`, resolved via `blocks -> courses`.
/// `None` if the block does not exist.
pub async fn program_id_for_block(pool: &PgPool, block_id: BlockId) -> Result<Option<ProgramId>> {
    let rec = sqlx::query!(
        r#"
        SELECT c.program_id as "program_id: ProgramId"
        FROM blocks b
        JOIN courses c ON c.id = b.course_id
        WHERE b.id = $1
        "#,
        block_id
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)?;

    Ok(rec.map(|r| r.program_id))
}

/// Look up an existing document by its content hash, for upload-time dedupe.
pub async fn find_id_by_sha256(pool: &PgPool, sha256: &str) -> Result<Option<DocumentId>> {
    let rec = sqlx::query!(
        r#"SELECT id as "id: DocumentId" FROM documents WHERE sha256 = $1"#,
        sha256
    )
    .fetch_optional(pool)
    .await
    .map_err(Error::from_sqlx)?;

    Ok(rec.map(|r| r.id))
}

/// Insert a new `documents` row in `status = 'pending'`.
///
/// `page_count` is unknown at upload time (layout parsing happens in the
/// ingestion worker), so callers pass `0` as a placeholder; the ingestion
/// pipeline is expected to update it once parsing completes.
pub async fn insert_document(
    pool: &PgPool,
    block_id: BlockId,
    uploaded_by: UserId,
    title: &str,
    storage_key: &str,
    sha256: &str,
    page_count: i32,
) -> Result<DocumentId> {
    let rec = sqlx::query!(
        r#"
        INSERT INTO documents (block_id, uploaded_by, title, storage_key, sha256, page_count, status)
        VALUES ($1, $2, $3, $4, $5, $6, 'pending')
        RETURNING id as "id: DocumentId"
        "#,
        block_id,
        uploaded_by,
        title,
        storage_key,
        sha256,
        page_count,
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)?;

    Ok(rec.id)
}

/// Insert the outbox row the ingestion worker polls. See the module-level
/// doc comment: depends on the parallel `0003_ingestion_jobs.sql` migration.
pub async fn insert_ingestion_job(pool: &PgPool, document_id: DocumentId) -> Result<Uuid> {
    let rec = sqlx::query!(
        r#"
        INSERT INTO ingestion_jobs (document_id, status, attempts)
        VALUES ($1, 'pending', 0)
        RETURNING id
        "#,
        document_id,
    )
    .fetch_one(pool)
    .await
    .map_err(Error::from_sqlx)?;

    Ok(rec.id)
}
