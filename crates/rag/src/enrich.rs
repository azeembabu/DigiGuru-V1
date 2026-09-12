//! Attach retrieval metadata to each chunk (`IMPLEMENTATION_PLAN.md` §5.1
//! "ENRICH" step, feeding the Qdrant payload schema in §3.2).
//!
//! `program_id`, `semester_no`, `course_id`, and `block_no` come from the
//! `documents`/`blocks`/`courses` join the caller already did before
//! ingestion started (Phase 1 tables) — they are not derivable from the PDF
//! itself, so they're passed in rather than inferred here.

use dg_core::{CourseId, DocumentId, ProgramId};

use crate::chunk::Chunk;

/// A chunk with its full Qdrant payload attached, ready for `embed` + upsert.
#[derive(Debug, Clone)]
pub struct EnrichedChunk {
    pub text: String,
    pub token_count: usize,
    pub kind: crate::chunk::ChunkKind,

    pub program_id: ProgramId,
    pub semester_no: i16,
    pub course_id: CourseId,
    pub block_no: i16,
    pub document_id: DocumentId,

    /// Best-effort: the most recent heading-like line seen before this
    /// chunk. `None` if no heading has been seen yet in the document.
    pub chapter: Option<String>,
    /// Real topic extraction is out of scope for this pass.
    pub topic: Option<String>,
    pub page: i32,
    pub para_index: usize,
    /// `ml` | `en`. Stubbed to `"ml"` for every chunk in this pass.
    // TODO(phase2-langdetect): run real per-chunk language detection
    // (the corpus is expected to be mostly Malayalam with some English
    // technical terms) instead of a fixed default.
    pub lang: String,
}

/// Caller-supplied context that doesn't come from the PDF (Phase 1 join).
#[derive(Debug, Clone, Copy)]
pub struct EnrichContext {
    pub program_id: ProgramId,
    pub semester_no: i16,
    pub course_id: CourseId,
    pub block_no: i16,
    pub document_id: DocumentId,
}

/// A line is treated as a heading if it's short, has no terminal sentence
/// punctuation, and is not itself a table/verse chunk. This is a best-effort
/// heuristic, not a layout-derived heading detector (that needs the real
/// pdfium layout boxes `parse.rs` doesn't extract yet).
fn looks_like_heading(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let word_count = trimmed.split_whitespace().count();
    let ends_with_sentence_punct =
        trimmed.ends_with('.') || trimmed.ends_with('!') || trimmed.ends_with('?');
    word_count <= 8 && !ends_with_sentence_punct
}

/// Enriches chunks in reading order, tracking the most recent heading-like
/// chunk as `chapter` for every chunk after it.
pub fn enrich_chunks(chunks: Vec<Chunk>, ctx: EnrichContext) -> Vec<EnrichedChunk> {
    let mut current_chapter: Option<String> = None;
    let mut enriched = Vec::with_capacity(chunks.len());

    for chunk in chunks {
        if matches!(chunk.kind, crate::chunk::ChunkKind::Prose) && looks_like_heading(&chunk.text)
        {
            current_chapter = Some(chunk.text.trim().to_string());
        }

        enriched.push(EnrichedChunk {
            text: chunk.text,
            token_count: chunk.token_count,
            kind: chunk.kind,
            program_id: ctx.program_id,
            semester_no: ctx.semester_no,
            course_id: ctx.course_id,
            block_no: ctx.block_no,
            document_id: ctx.document_id,
            chapter: current_chapter.clone(),
            topic: None,
            page: chunk.page,
            para_index: chunk.para_index,
            lang: "ml".to_string(),
        });
    }

    enriched
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkKind;
    use uuid::Uuid;

    fn ctx() -> EnrichContext {
        EnrichContext {
            program_id: ProgramId::from_uuid(Uuid::nil()),
            semester_no: 1,
            course_id: CourseId::from_uuid(Uuid::nil()),
            block_no: 1,
            document_id: DocumentId::from_uuid(Uuid::nil()),
        }
    }

    #[test]
    fn tracks_most_recent_heading_as_chapter() {
        let chunks = vec![
            Chunk {
                text: "Chapter One".to_string(),
                token_count: 2,
                kind: ChunkKind::Prose,
                page: 1,
                para_index: 0,
            },
            Chunk {
                text: "Some real paragraph content that is not a heading at all.".to_string(),
                token_count: 10,
                kind: ChunkKind::Prose,
                page: 1,
                para_index: 1,
            },
        ];

        let enriched = enrich_chunks(chunks, ctx());

        assert_eq!(enriched[0].chapter.as_deref(), Some("Chapter One"));
        assert_eq!(enriched[1].chapter.as_deref(), Some("Chapter One"));
        assert_eq!(enriched[1].lang, "ml");
        assert_eq!(enriched[1].topic, None);
    }
}
