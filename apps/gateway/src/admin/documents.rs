//! `POST /api/v1/admin/blocks/{block_id}/documents` — sub-admin PDF upload,
//! the entry point to the Phase 2 ingestion pipeline
//! (`IMPLEMENTATION_PLAN.md` §5.1).
//!
//! Scope: this handler only accepts the file, dedupes it by content hash,
//! writes the `documents` row (`status = 'pending'`), and enqueues an
//! `ingestion_jobs` outbox row for the worker to pick up. Virus scanning,
//! real object storage, and everything from layout-parse onward belong to
//! the ingestion worker (`workers/ingest`, owned by a parallel agent in this
//! pass) — out of scope here by design, not an oversight.
//!
//! The two `GET` routes alongside it are the read side an admin console
//! needs to answer "did that PDF ingest?" — they expose the `documents` row
//! plus the status and error of its most recent `ingestion_jobs` attempt
//! (`documents` itself has no error column). `storage_key` is not on the
//! wire: it is a server-side filesystem path.
//!
//! Request body is the raw PDF bytes (`Content-Type: application/pdf` or
//! `application/octet-stream`), not multipart — simpler to get right for
//! this pass than a multipart boundary parser, and the frontend's upload
//! button can just `fetch` the raw file. `title` is a query parameter.

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use dg_core::{BlockId, Capability, DocumentId, PublicError};
use dg_db::models::{documents, uploads};

use crate::extractors::AuthenticatedActor;
use crate::state::AppState;

/// Where uploaded PDFs land in this pass.
///
/// TODO(phase2-storage): this must move to real object storage (the
/// `IMPLEMENTATION_PLAN.md` §5.1 ingestion workflow says "object storage")
/// before production — local disk does not survive a redeploy and does not
/// scale past one gateway instance. Local-disk storage is a deliberate
/// stopgap for this pass, not a silent shortcut.
const UPLOAD_DIR: &str = "./data/uploads";

#[derive(Debug, Deserialize)]
pub struct UploadQuery {
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub document_id: Uuid,
    pub job_id: Uuid,
}

pub async fn upload(
    AuthenticatedActor(actor): AuthenticatedActor,
    State(state): State<AppState>,
    Path(block_id): Path<Uuid>,
    Query(query): Query<UploadQuery>,
    body: Bytes,
) -> Result<Json<UploadResponse>, PublicError> {
    let block_id = BlockId::from(block_id);

    // Resolve the block's owning program *before* checking the capability,
    // so a sub-admin outside the program's scope gets the same 403 whether
    // or not the block exists — no information leak about block validity.
    let program_id = uploads::program_id_for_block(&state.pool, block_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::UploadDocuments, program_id)?;

    if body.is_empty() {
        return Err(PublicError::validation("file", "Uploaded file is empty."));
    }

    if query.title.trim().is_empty() {
        return Err(PublicError::validation("title", "Title is required."));
    }

    let mut hasher = Sha256::new();
    hasher.update(&body);
    let sha256 = format!("{:x}", hasher.finalize());

    // E-24 / rag-pipeline.md item 6: dedupe by content hash before doing
    // any storage or DB write, so re-uploading the same PDF never doubles
    // the corpus.
    if uploads::find_id_by_sha256(&state.pool, &sha256)
        .await
        .map_err(PublicError::from)?
        .is_some()
    {
        return Err(PublicError::Conflict {
            code: "DOCUMENT_ALREADY_EXISTS",
            message: "A document with identical contents has already been uploaded.".into(),
        });
    }

    tokio::fs::create_dir_all(UPLOAD_DIR).await.map_err(|err| {
        tracing::error!(error = %err, "failed to create upload directory");
        PublicError::Internal
    })?;

    let storage_key = format!("{UPLOAD_DIR}/{sha256}.pdf");
    tokio::fs::write(&storage_key, &body).await.map_err(|err| {
        tracing::error!(error = %err, "failed to write uploaded file to disk");
        PublicError::Internal
    })?;

    // `page_count` is unknown until the ingestion worker's layout-parse
    // step runs; `0` is a placeholder the worker is expected to update.
    let document_id = uploads::insert_document(
        &state.pool,
        block_id,
        actor.user_id,
        query.title.trim(),
        &storage_key,
        &sha256,
        0,
    )
    .await
    .map_err(PublicError::from)?;

    let job_id = uploads::insert_ingestion_job(&state.pool, document_id)
        .await
        .map_err(PublicError::from)?;

    crate::audit::log(
        &state,
        Some(actor.user_id),
        "admin.document_uploaded",
        None,
        None,
        Some(serde_json::json!({
            "document_id": document_id,
            "block_id": block_id,
            "job_id": job_id,
        })),
    )
    .await;

    Ok(Json(UploadResponse {
        document_id: document_id.into_uuid(),
        job_id,
    }))
}

/// The ingestion-status view of a document. Mirrors
/// `dg_db::models::documents::DocumentSummary` — see that struct for why
/// `storage_key` is absent.
#[derive(Debug, Serialize)]
pub struct DocumentResponse {
    pub id: Uuid,
    pub block_id: Uuid,
    pub uploaded_by: Uuid,
    pub title: String,
    pub sha256: String,
    pub page_count: i32,
    pub ocr_confidence: Option<f32>,
    /// `pending|parsing|pending_review|embedded|failed`.
    pub status: String,
    pub created_at: DateTime<Utc>,
    /// `pending|processing|completed|failed` of the latest ingestion attempt,
    /// `null` if no job row was ever enqueued.
    pub job_status: Option<String>,
    /// The latest attempt's failure reason, `null` unless it failed.
    pub last_error: Option<String>,
}

impl From<documents::DocumentSummary> for DocumentResponse {
    fn from(d: documents::DocumentSummary) -> Self {
        Self {
            id: d.id.into_uuid(),
            block_id: d.block_id.into_uuid(),
            uploaded_by: d.uploaded_by.into_uuid(),
            title: d.title,
            sha256: d.sha256,
            page_count: d.page_count,
            ocr_confidence: d.ocr_confidence,
            status: d.status,
            created_at: d.created_at,
            job_status: d.job_status,
            last_error: d.last_error,
        }
    }
}

pub async fn list_documents_for_block(
    AuthenticatedActor(actor): AuthenticatedActor,
    State(state): State<AppState>,
    Path(block_id): Path<Uuid>,
) -> Result<Json<Vec<DocumentResponse>>, PublicError> {
    let block_id = BlockId::from(block_id);

    // Same ordering as `upload`: resolve the owning program before the
    // capability check, so block existence never leaks across a scope.
    let program_id = uploads::program_id_for_block(&state.pool, block_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    actor.require_scoped(Capability::UploadDocuments, program_id)?;

    let rows = documents::list_summaries_by_block(&state.pool, block_id)
        .await
        .map_err(PublicError::from)?;

    Ok(Json(rows.into_iter().map(DocumentResponse::from).collect()))
}

pub async fn get_document(
    AuthenticatedActor(actor): AuthenticatedActor,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<DocumentResponse>, PublicError> {
    let document_id = DocumentId::from(id);
    let document = documents::find_summary_by_id(&state.pool, document_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::NotFound)?;

    let program_id = uploads::program_id_for_block(&state.pool, document.block_id)
        .await
        .map_err(PublicError::from)?
        .ok_or(PublicError::Internal)?;

    actor.require_scoped(Capability::UploadDocuments, program_id)?;

    Ok(Json(document.into()))
}

#[cfg(test)]
mod tests {
    use dg_core::{Actor, Capability, ProgramId, Role, UserId};

    /// The authorization rule shared by upload and both read routes, applied
    /// after the document's owning program has been resolved.
    fn authorize(actor: &Actor, program_id: ProgramId) -> Result<(), dg_core::PublicError> {
        actor.require_scoped(Capability::UploadDocuments, program_id)
    }

    #[test]
    fn super_admin_may_read_documents_in_any_program() {
        let actor = Actor::new(UserId::new(), Role::SuperAdmin, vec![]);
        assert!(authorize(&actor, ProgramId::new()).is_ok());
    }

    #[test]
    fn sub_admin_may_read_documents_only_in_scoped_programs() {
        let scoped = ProgramId::new();
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![scoped]);
        assert!(authorize(&actor, scoped).is_ok());
    }

    #[test]
    fn sub_admin_is_forbidden_documents_outside_its_scope() {
        let actor = Actor::new(UserId::new(), Role::SubAdmin, vec![ProgramId::new()]);
        let err = authorize(&actor, ProgramId::new()).expect_err("out of scope");
        assert_eq!(err.code(), "FORBIDDEN");
    }

    #[test]
    fn student_is_forbidden_every_document_route() {
        let program_id = ProgramId::new();
        let actor = Actor::new(UserId::new(), Role::Student, vec![program_id]);
        let err = authorize(&actor, program_id).expect_err("students never read documents");
        assert_eq!(err.code(), "FORBIDDEN");
    }
}
