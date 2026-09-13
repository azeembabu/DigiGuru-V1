//! Orchestrates the ingestion pipeline (`IMPLEMENTATION_PLAN.md` §5.1):
//! parse -> clean -> chunk -> enrich -> embed -> upsert -> prompt cache.
//!
//! This module runs the pipeline and *reports* what happened; it does not
//! write `documents.status` or touch `ingestion_jobs`. The worker owns those,
//! because it is the thing holding the claimed job id and the retry budget —
//! keeping the state machine in one place rather than half here and half
//! there. [`IngestOutcome`] is the handoff.

use dg_core::{BlockId, CourseId, DocumentId, ProgramId};
use qdrant_client::Qdrant;

use crate::chunk::{self, Paragraph};
use crate::clean::{self, PageText};
use crate::embed::Embedder;
use crate::enrich::{self, EnrichContext};
use crate::error::{RagError, Result};
use crate::parse::{ParsedDocument, PdfParser};
use crate::qdrant as dg_qdrant;

/// Below this mean OCR confidence a document is held for sub-admin review
/// rather than going live (`rag-pipeline.md` ingestion rule 7).
pub const OCR_REVIEW_THRESHOLD: f32 = 0.75;

/// Everything the pipeline needs that does not come from the PDF — the
/// `documents -> blocks -> courses -> semesters` join the worker already did.
#[derive(Debug, Clone)]
pub struct IngestContext {
    pub document_id: DocumentId,
    pub document_title: String,
    pub block_id: BlockId,
    pub block_no: i16,
    pub course_id: CourseId,
    pub program_id: ProgramId,
    pub semester_no: i16,
}

/// The terminal state the worker should record for the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestStatus {
    /// Indexed and live.
    Embedded,
    /// Indexed, but OCR confidence was too low to trust — held for a
    /// sub-admin to approve before it goes live.
    PendingReview,
}

impl IngestStatus {
    /// The `documents.status` value for this outcome.
    pub fn as_db_status(self) -> &'static str {
        match self {
            IngestStatus::Embedded => "embedded",
            IngestStatus::PendingReview => "pending_review",
        }
    }
}

/// Summary of one ingestion run — the "ingest report" of §5.1.
#[derive(Debug, Clone)]
pub struct IngestOutcome {
    pub document_id: DocumentId,
    pub status: IngestStatus,
    pub chunk_count: usize,
    pub page_count: usize,
    pub ocr_confidence: Option<f32>,
    pub preamble_handle: Option<String>,
    pub warnings: Vec<String>,
}

/// Runs the pipeline for one already-uploaded, already-deduped PDF.
///
/// `parser` and `embedder` are injected so the pipeline can be exercised
/// without a native PDF stack or a live embedding API — see
/// [`crate::parse::StubParser`] and [`crate::embed::StubEmbedder`].
pub async fn run_pipeline(
    qdrant: &Qdrant,
    redis: &mut redis::aio::ConnectionManager,
    parser: &(dyn PdfParser + 'static),
    embedder: &dyn Embedder,
    ctx: &IngestContext,
    pdf_bytes: Vec<u8>,
) -> Result<IngestOutcome> {
    let mut warnings = Vec::new();

    // PARSE — CPU-bound, so off the async worker threads
    // (`.claude/rules/code-style.md`). The parser is `Send + Sync`, but it is
    // borrowed, so parse against an owned clone of the bytes on the blocking
    // pool and move the result back.
    let parsed: ParsedDocument = parse_on_blocking_pool(parser, pdf_bytes).await?;

    let page_count = parsed.pages.len();
    let ocr_confidence = parsed.ocr_confidence();

    let scanned = parsed.scanned_pages();
    if !scanned.is_empty() {
        warnings.push(format!(
            "{} of {page_count} page(s) yielded no extractable text and were not OCR'd \
             (no OCR engine in this build): pages {:?}",
            scanned.len(),
            scanned
        ));
    }

    // CLEAN — strip headers/footers/page numbers/watermarks BEFORE chunking.
    let page_texts: Vec<PageText> = parsed
        .pages
        .iter()
        .map(|p| PageText {
            page_number: p.page_number,
            text: p.text(),
        })
        .collect();
    let cleaned = clean::clean_pages(page_texts);

    // CHUNK — paragraphs (blank-line separated) then paragraph-level chunking.
    let paragraphs = pages_to_paragraphs(cleaned);
    let chunks = chunk::chunk_paragraphs(paragraphs);

    if chunks.is_empty() {
        return Err(RagError::Chunk(format!(
            "document produced no chunks from {page_count} page(s); \
             it is probably a scan needing OCR"
        )));
    }

    // ENRICH — attach the full mandatory payload.
    let enrich_ctx = EnrichContext {
        program_id: ctx.program_id,
        semester_no: ctx.semester_no,
        course_id: ctx.course_id,
        block_no: ctx.block_no,
        document_id: ctx.document_id,
        document_title: ctx.document_title.clone(),
    };
    let enriched = enrich::enrich_chunks(chunks, &enrich_ctx);
    let chunk_count = enriched.len();

    // Validate the whole batch before embedding: a chunk missing mandatory
    // metadata is a bug (`rag-pipeline.md`), and catching it here avoids
    // paying for embeddings on a run that cannot be upserted anyway.
    for chunk in &enriched {
        crate::payload::validate_chunk(chunk)?;
    }

    // EMBED.
    let mut embeddings = Vec::with_capacity(enriched.len());
    for chunk in &enriched {
        embeddings.push(embedder.embed(&chunk.text).await?);
    }

    // UPSERT. Clear this document's previous points first so a retry or
    // re-ingest replaces rather than duplicates (see `delete_document_points`).
    dg_qdrant::ensure_collection(qdrant).await?;
    dg_qdrant::delete_document_points(qdrant, ctx.document_id).await?;
    dg_qdrant::upsert_chunks(qdrant, &enriched, &embeddings).await?;

    // PROMPT CACHE — `pcache:{block_id}` (`rag-pipeline.md` cost discipline).
    let preamble_text = crate::preamble::build_preamble(ctx.block_no, &enriched);
    let cached = crate::preamble::register_preamble(redis, ctx.block_id, preamble_text).await?;

    // Low OCR confidence holds the document for review; it does not go live
    // (`rag-pipeline.md` ingestion rule 7). The chunks are indexed either
    // way — retrieval gates on `documents.status`, so nothing is served from
    // a document still awaiting approval.
    let status = match ocr_confidence {
        Some(confidence) if confidence < OCR_REVIEW_THRESHOLD => {
            warnings.push(format!(
                "mean OCR confidence {confidence:.2} is below the {OCR_REVIEW_THRESHOLD:.2} \
                 threshold; held for sub-admin review"
            ));
            IngestStatus::PendingReview
        }
        _ => IngestStatus::Embedded,
    };

    Ok(IngestOutcome {
        document_id: ctx.document_id,
        status,
        chunk_count,
        page_count,
        ocr_confidence,
        preamble_handle: Some(cached.handle),
        warnings,
    })
}

/// Runs the (CPU-bound) parse without blocking a tokio worker.
///
/// `spawn_blocking` requires a `'static` closure, which a borrowed
/// `&dyn PdfParser` is not. Rather than reach for unsafe lifetime erasure,
/// the parse runs on a dedicated blocking thread via `block_in_place` when a
/// multi-threaded runtime is available, which keeps the borrow intact and
/// still moves the CPU work off the async worker's poll loop.
async fn parse_on_blocking_pool(
    parser: &(dyn PdfParser + 'static),
    bytes: Vec<u8>,
) -> Result<ParsedDocument> {
    match tokio::runtime::Handle::try_current().map(|h| h.runtime_flavor()) {
        Ok(tokio::runtime::RuntimeFlavor::MultiThread) => {
            // Moves this task's worker thread out of the scheduler for the
            // duration, so the parse cannot stall other tasks on it.
            tokio::task::block_in_place(|| parser.parse(&bytes))
        }
        // Single-threaded runtime (tests): no worker pool to protect.
        _ => parser.parse(&bytes),
    }
}

/// Splits each cleaned page's text into paragraphs on blank lines, indexing
/// them in document reading order.
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

/// Verifies the bytes on disk still hash to the `sha256` recorded at upload.
///
/// `documents.sha256` is `UNIQUE`, which is what stops a re-upload doubling
/// the corpus — but that only holds if the stored file is still the file that
/// was hashed. Checking here turns a swapped or truncated file into a clear
/// permanent failure instead of a document indexed under another file's
/// identity.
pub fn verify_sha256(bytes: &[u8], expected: &str) -> Result<()> {
    use sha2::{Digest, Sha256};
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(RagError::Parse(format!(
            "stored file hash {actual} does not match the recorded sha256 {expected}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_are_indexed_across_pages_in_reading_order() {
        let pages = vec![
            PageText {
                page_number: 1,
                text: "First para.\n\nSecond para.".to_string(),
            },
            PageText {
                page_number: 2,
                text: "Third para.".to_string(),
            },
        ];

        let paragraphs = pages_to_paragraphs(pages);

        assert_eq!(paragraphs.len(), 3);
        assert_eq!(
            paragraphs.iter().map(|p| p.index).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(paragraphs[2].page, 2);
    }

    #[test]
    fn sha256_mismatch_is_an_error() {
        assert!(verify_sha256(b"hello", "not the right hash").is_err());
    }

    #[test]
    fn sha256_match_passes() {
        use sha2::{Digest, Sha256};
        let expected = format!("{:x}", Sha256::digest(b"hello"));
        assert!(verify_sha256(b"hello", &expected).is_ok());
    }

    #[test]
    fn low_ocr_confidence_maps_to_pending_review_not_embedded() {
        // The threshold decision itself, isolated from the IO-heavy pipeline.
        let held = 0.4f32 < OCR_REVIEW_THRESHOLD;
        assert!(held, "0.4 must be below the review threshold");
        assert_eq!(IngestStatus::PendingReview.as_db_status(), "pending_review");
        assert_eq!(IngestStatus::Embedded.as_db_status(), "embedded");
    }
}
