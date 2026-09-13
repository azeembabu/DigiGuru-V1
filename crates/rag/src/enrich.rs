//! Attach retrieval metadata to each chunk (`IMPLEMENTATION_PLAN.md` §5.1
//! "ENRICH" step, feeding the Qdrant payload schema in §3.2).
//!
//! `program_id`, `semester_no`, `course_id`, and `block_no` come from the
//! `documents`/`blocks`/`courses` join the caller already did before
//! ingestion started (Phase 1 tables) — they are not derivable from the PDF
//! itself, so they're passed in rather than inferred here.
//!
//! Every field of [`EnrichedChunk`] that the payload schema requires is a
//! non-`Option` `String`/number. That is deliberate: `.claude/rules/rag-pipeline.md`
//! makes a chunk missing any of `program_id`, `semester`, `block_no`,
//! `document_id`, `chapter`, `topic`, `page`, `para_index`, `lang` a *bug*,
//! and an `Option` here would let one reach the upsert boundary and be
//! silently `unwrap_or_default()`-ed into an empty payload string — which
//! retrieval would then happily cite as an empty chapter. Where a value
//! cannot be derived from the document, enrichment substitutes an explicit,
//! meaningful fallback (the document title) rather than nothing, and
//! [`validate`](crate::payload::validate_chunk) rejects anything still blank.

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

    /// The most recent heading-like line seen before this chunk, falling
    /// back to the document title when the document has no heading yet.
    pub chapter: String,
    /// Best available topic label. Falls back to the chapter, which itself
    /// falls back to the document title — never blank.
    pub topic: String,
    pub page: i32,
    pub para_index: usize,
    /// `ml` | `en`, detected per chunk from Malayalam codepoint density.
    pub lang: String,
}

/// Caller-supplied context that doesn't come from the PDF (Phase 1 join).
#[derive(Debug, Clone)]
pub struct EnrichContext {
    pub program_id: ProgramId,
    pub semester_no: i16,
    pub course_id: CourseId,
    pub block_no: i16,
    pub document_id: DocumentId,
    /// `documents.title`. Used as the last-resort `chapter`/`topic` label so
    /// no chunk is ever upserted with a blank citation field.
    pub document_title: String,
}

/// A line is treated as a heading if it's short and has no terminal sentence
/// punctuation. This is a best-effort heuristic, not a layout-derived heading
/// detector (that needs real pdfium layout boxes, which `parse.rs` does not
/// extract — see its module docs).
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

/// Malayalam occupies the Unicode block U+0D00–U+0D7F. A chunk is tagged
/// `ml` when Malayalam codepoints outnumber ASCII letters, else `en` — the
/// corpus is majority Malayalam but carries English technical terms, and a
/// fixed `"ml"` would mislabel a wholly-English page.
pub fn detect_lang(text: &str) -> &'static str {
    let mut malayalam = 0usize;
    let mut latin = 0usize;
    for c in text.chars() {
        if ('\u{0D00}'..='\u{0D7F}').contains(&c) {
            malayalam += 1;
        } else if c.is_ascii_alphabetic() {
            latin += 1;
        }
    }
    if malayalam > latin {
        "ml"
    } else {
        "en"
    }
}

/// Enriches chunks in reading order, tracking the most recent heading-like
/// chunk as `chapter` for every chunk after it.
pub fn enrich_chunks(chunks: Vec<Chunk>, ctx: &EnrichContext) -> Vec<EnrichedChunk> {
    let fallback = {
        let trimmed = ctx.document_title.trim();
        if trimmed.is_empty() {
            // `documents.title` is NOT NULL but not checked non-blank; keep
            // the invariant "never blank" true regardless.
            "Untitled document".to_string()
        } else {
            trimmed.to_string()
        }
    };

    let mut current_chapter: Option<String> = None;
    let mut enriched = Vec::with_capacity(chunks.len());

    for chunk in chunks {
        if matches!(chunk.kind, crate::chunk::ChunkKind::Prose) && looks_like_heading(&chunk.text) {
            current_chapter = Some(chunk.text.trim().to_string());
        }

        let chapter = current_chapter.clone().unwrap_or_else(|| fallback.clone());
        // Topic extraction proper (a real section-label model) is out of
        // scope; the chapter is the most specific real label available, and
        // is always non-blank.
        let topic = chapter.clone();
        let lang = detect_lang(&chunk.text).to_string();

        enriched.push(EnrichedChunk {
            text: chunk.text,
            token_count: chunk.token_count,
            kind: chunk.kind,
            program_id: ctx.program_id,
            semester_no: ctx.semester_no,
            course_id: ctx.course_id,
            block_no: ctx.block_no,
            document_id: ctx.document_id,
            chapter,
            topic,
            page: chunk.page,
            para_index: chunk.para_index,
            lang,
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
            document_title: "Malayalam Poetry Reader".to_string(),
        }
    }

    fn prose(text: &str, para_index: usize) -> Chunk {
        Chunk {
            token_count: text.split_whitespace().count(),
            text: text.to_string(),
            kind: ChunkKind::Prose,
            page: 1,
            para_index,
        }
    }

    #[test]
    fn tracks_most_recent_heading_as_chapter() {
        let chunks = vec![
            prose("Chapter One", 0),
            prose("Some real paragraph content that is not a heading at all.", 1),
        ];

        let enriched = enrich_chunks(chunks, &ctx());

        assert_eq!(enriched[0].chapter, "Chapter One");
        assert_eq!(enriched[1].chapter, "Chapter One");
    }

    #[test]
    fn falls_back_to_document_title_before_any_heading() {
        // A chunk that appears before any heading still must carry a
        // non-blank chapter/topic — blank would pass straight through to a
        // citation the tutor reads out.
        let enriched = enrich_chunks(
            vec![prose("Body text arriving before any heading is seen.", 0)],
            &ctx(),
        );

        assert_eq!(enriched[0].chapter, "Malayalam Poetry Reader");
        assert_eq!(enriched[0].topic, "Malayalam Poetry Reader");
    }

    #[test]
    fn never_emits_a_blank_chapter_or_topic() {
        let enriched = enrich_chunks(
            vec![prose("a", 0), prose("Some longer body sentence here.", 1)],
            &ctx(),
        );
        for chunk in &enriched {
            assert!(!chunk.chapter.trim().is_empty());
            assert!(!chunk.topic.trim().is_empty());
            assert!(!chunk.lang.trim().is_empty());
        }
    }

    #[test]
    fn detects_malayalam_and_english() {
        assert_eq!(detect_lang("ഇത് ഒരു മലയാളം വാക്യമാണ്"), "ml");
        assert_eq!(detect_lang("This is an English sentence."), "en");
    }
}
