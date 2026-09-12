//! Orchestrates the full ingestion pipeline (`IMPLEMENTATION_PLAN.md` §5.1):
//! parse -> clean -> chunk -> enrich -> embed -> upsert to Qdrant, then
//! flips `documents.status` to `embedded` (or `failed`, with the reason
//! recorded on the owning `ingestion_jobs` row).

use dg_core::{CourseId, DocumentId, ProgramId};
use dg_db::models::documents;
use qdrant_client::Qdrant;
use sqlx::PgPool;

use crate::chunk::{self, Paragraph};
use crate::clean::{self, PageText};
use crate::embed::Embedder;
use crate::enrich::{self, EnrichContext};
use crate::error::{RagError, Result};
use crate::parse::{ParsedDocument, PdfParser};
use crate::qdrant as dg_qdrant;

/// Summary of one ingestion run, returned to the caller (the sub-admin
/// upload flow / worker) as the "ingest report" mentioned in §5.1.
#[derive(Debug, Clone)]
pub struct IngestReport {
    pub document_id: DocumentId,
    pub chunk_count: usize,
    pub page_count: usize,
    pub warnings: Vec<String>,
}

/// Runs the full ingestion pipeline for one already-uploaded, already
/// deduped PDF. `program_id`/`semester_no`/`course_id`/`block_no` come from
/// the `documents`/`blocks`/`courses` join the caller already did (Phase 1
/// tables) — they are not derivable from the PDF itself.
#[allow(clippy::too_many_arguments)]
pub async fn run_ingestion(
    pool: &PgPool,
    qdrant: &Qdrant,
    embedder: &dyn Embedder,
    document_id: DocumentId,
    pdf_bytes: &[u8],
    program_id: ProgramId,
    semester_no: i16,
    course_id: CourseId,
    block_no: i16,
) -> Result<IngestReport> {
    match run_ingestion_inner(
        pool,
        qdrant,
        embedder,
        document_id,
        pdf_bytes,
        program_id,
        semester_no,
        course_id,
        block_no,
    )
    .await
    {
        Ok(report) => {
            documents::mark_embedded(pool, document_id)
                .await
                .map_err(RagError::from)?;
            documents::complete_latest_job(pool, document_id)
                .await
                .map_err(RagError::from)?;
            Ok(report)
        }
        Err(err) => {
            documents::mark_failed(pool, document_id)
                .await
                .map_err(RagError::from)?;
            documents::fail_latest_job(pool, document_id, &err.to_string())
                .await
                .map_err(RagError::from)?;
            Err(err)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_ingestion_inner(
    _pool: &PgPool,
    qdrant: &Qdrant,
    embedder: &dyn Embedder,
    document_id: DocumentId,
    pdf_bytes: &[u8],
    program_id: ProgramId,
    semester_no: i16,
    course_id: CourseId,
    block_no: i16,
) -> Result<IngestReport> {
    let mut warnings = Vec::new();

    // PARSE — CPU-bound; keep it off the async runtime's worker threads
    // (`.claude/rules/code-style.md`: "CPU-bound work goes through
    // spawn_blocking").
    let bytes = pdf_bytes.to_vec();
    let parsed: ParsedDocument = tokio::task::spawn_blocking(move || {
        let parser = crate::parse::LopdfParser;
        parser.parse(&bytes)
    })
    .await
    .map_err(|e| RagError::Parse(format!("parse task panicked: {e}")))??;

    let page_count = parsed.pages.len();

    for page in &parsed.pages {
        if page.ocr_confidence.is_none() && page.blocks.is_empty() {
            // TODO(phase2-ocr): a blank/low-text page should trigger OCR.
            // For this pass it's just surfaced as a warning so the
            // sub-admin's ingest report can flag it for manual review.
            warnings.push(format!(
                "page {} produced no extractable text (possible scan; OCR not run)",
                page.page_number
            ));
        }
    }

    // CLEAN — strip headers/footers/page numbers/watermarks.
    let page_texts: Vec<PageText> = parsed
        .pages
        .iter()
        .map(|p| PageText {
            page_number: p.page_number,
            text: p.text(),
        })
        .collect();
    let cleaned = clean::clean_pages(page_texts);

    // CHUNK — split each cleaned page into paragraphs (blank-line
    // separated), then paragraph-level chunk across the whole document.
    let paragraphs = pages_to_paragraphs(cleaned);
    let chunks = chunk::chunk_paragraphs(paragraphs);

    // ENRICH — attach program/semester/course/block + chapter/topic/lang.
    let ctx = EnrichContext {
        program_id,
        semester_no,
        course_id,
        block_no,
        document_id,
    };
    let enriched = enrich::enrich_chunks(chunks, ctx);
    let chunk_count = enriched.len();

    // EMBED — dense vector per chunk. Sequential for simplicity; the
    // corpus-scale batching/concurrency tuning is a follow-up, not required
    // for pipeline correctness.
    let mut embeddings = Vec::with_capacity(enriched.len());
    for chunk in &enriched {
        let vector = embedder.embed(&chunk.text).await?;
        embeddings.push(vector);
    }

    // UPSERT to Qdrant with full metadata payload.
    dg_qdrant::ensure_collection(qdrant).await?;
    dg_qdrant::upsert_chunks(qdrant, &enriched, &embeddings).await?;

    Ok(IngestReport {
        document_id,
        chunk_count,
        page_count,
        warnings,
    })
}

/// Splits each cleaned page's text into paragraphs on blank lines, indexing
/// paragraphs in document reading order. A page with no blank-line breaks
/// becomes a single paragraph.
fn pages_to_paragraphs(pages: Vec<PageText>) -> Vec<Paragraph> {
    let mut paragraphs = Vec::new();
    let mut index = 0usize;

    for page in pages {
        let mut current = String::new();
        for line in page.text.lines() {
            if line.trim().is_empty() {
                if !current.trim().is_empty() {
                    paragraphs.push(Paragraph {
                        page: page.page_number,
                        index,
                        text: std::mem::take(&mut current).trim().to_string(),
                    });
                    index += 1;
                }
                current.clear();
                continue;
            }
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }
        if !current.trim().is_empty() {
            paragraphs.push(Paragraph {
                page: page.page_number,
                index,
                text: current.trim().to_string(),
            });
            index += 1;
        }
    }

    paragraphs
}
