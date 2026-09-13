//! Paragraph-level chunking (`IMPLEMENTATION_PLAN.md` §5.1, E-23):
//! never split a sentence or a table; target 250–450 tokens with a
//! one-sentence overlap; paragraphs under 80 tokens merge forward; tables
//! and verse/poetry blocks stay atomic, tagged `kind: table|verse`.
//!
//! Token count is a simple whitespace word-count proxy — not a real
//! tokenizer — per the ingestion scope for this pass.

const TARGET_MIN_TOKENS: usize = 250;
const TARGET_MAX_TOKENS: usize = 450;
// A paragraph under 80 tokens merges forward per `IMPLEMENTATION_PLAN.md`
// §5.1. In practice this falls out of the `TARGET_MIN_TOKENS` flush gate
// below without a separate threshold: anything under 250 tokens (which
// includes everything under 80) merges forward into the next paragraph
// rather than being flushed as its own chunk.

/// A single input paragraph, already page/para-indexed by the caller
/// (`ingest.rs`, which owns turning `clean::PageText` into paragraphs by
/// splitting on blank lines). Kept independent of `parse`/`clean` so this
/// module is directly unit-testable against synthetic input.
#[derive(Debug, Clone)]
pub struct Paragraph {
    pub page: i32,
    /// Index of this paragraph within the document, in reading order —
    /// becomes the chunk's `para_index` (first paragraph a chunk draws
    /// from), matching the Qdrant payload schema (`IMPLEMENTATION_PLAN.md`
    /// §3.2).
    pub index: usize,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkKind {
    Prose,
    Table,
    Verse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub text: String,
    pub token_count: usize,
    pub kind: ChunkKind,
    pub page: i32,
    pub para_index: usize,
}

/// Chunk a document's paragraphs (in reading order) into `Chunk`s.
pub fn chunk_paragraphs(paragraphs: Vec<Paragraph>) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut buffer: Vec<BufferedSentence> = Vec::new();
    // Held between flushes rather than pushed into `buffer` immediately, so
    // that if it turns out there is no further content to attach it to
    // (e.g. the paragraph that triggered the flush was the last one), it is
    // simply dropped instead of becoming a spurious one-sentence chunk.
    let mut pending_overlap: Option<BufferedSentence> = None;

    for paragraph in paragraphs {
        if let Some(kind) = classify_paragraph(&paragraph.text) {
            // A table/verse paragraph interrupts prose flow (and any
            // pending overlap): flush whatever prose is pending (even under
            // the 250-token target — a table boundary is a harder boundary
            // than a token-count target), then emit the table/verse as its
            // own atomic chunk.
            pending_overlap = None;
            flush_buffer(&mut buffer, &mut chunks);
            chunks.push(Chunk {
                token_count: count_tokens(&paragraph.text),
                text: paragraph.text,
                kind,
                page: paragraph.page,
                para_index: paragraph.index,
            });
            continue;
        }

        for sentence in split_sentences(&paragraph.text) {
            if buffer.is_empty() {
                if let Some(overlap_sentence) = pending_overlap.take() {
                    buffer.push(overlap_sentence);
                }
            }

            let sentence_tokens = count_tokens(&sentence);
            let buffer_tokens: usize = buffer.iter().map(|s| s.tokens).sum();

            // Never split a sentence: if adding this sentence would push us
            // over the max *and* we're already past the target minimum,
            // close the current chunk first and start a new one carrying a
            // one-sentence overlap forward.
            if buffer_tokens + sentence_tokens > TARGET_MAX_TOKENS
                && buffer_tokens >= TARGET_MIN_TOKENS
            {
                pending_overlap = flush_buffer(&mut buffer, &mut chunks);
                if let Some(overlap_sentence) = pending_overlap.take() {
                    buffer.push(overlap_sentence);
                }
            }

            buffer.push(BufferedSentence {
                text: sentence,
                tokens: sentence_tokens,
                page: paragraph.page,
                para_index: paragraph.index,
            });
        }

        // Paragraph boundary: flush only once the target minimum is met.
        // A paragraph under 80 tokens (and anything still short of the
        // 250-token target generally) merges forward into the next
        // paragraph by simply not flushing here.
        let buffer_tokens: usize = buffer.iter().map(|s| s.tokens).sum();
        if buffer_tokens >= TARGET_MIN_TOKENS {
            pending_overlap = flush_buffer(&mut buffer, &mut chunks);
        }
    }

    // Trailing content shorter than the target: emit as a final chunk
    // rather than dropping it. There is nothing left to merge it forward
    // into. Any still-pending overlap with no further content to attach to
    // is intentionally dropped, not emitted as its own chunk.
    flush_buffer(&mut buffer, &mut chunks);

    chunks
}

struct BufferedSentence {
    text: String,
    tokens: usize,
    page: i32,
    para_index: usize,
}

/// Flushes `buffer` into `chunks` as one `Prose` chunk (if non-empty),
/// clearing `buffer`, and returns the last sentence in it (for the
/// one-sentence overlap into the next chunk).
fn flush_buffer(
    buffer: &mut Vec<BufferedSentence>,
    chunks: &mut Vec<Chunk>,
) -> Option<BufferedSentence> {
    if buffer.is_empty() {
        return None;
    }
    let overlap_text = buffer.last().map(|s| s.text.clone());
    let overlap_tokens = buffer.last().map(|s| s.tokens).unwrap_or(0);
    let page = buffer[0].page;
    let para_index = buffer[0].para_index;
    let text = buffer
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let token_count = buffer.iter().map(|s| s.tokens).sum();

    chunks.push(Chunk {
        text,
        token_count,
        kind: ChunkKind::Prose,
        page,
        para_index,
    });
    buffer.clear();

    overlap_text.map(|text| BufferedSentence {
        text,
        tokens: overlap_tokens,
        page,
        para_index,
    })
}

fn count_tokens(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Naive sentence splitter: a `.`/`!`/`?` followed by whitespace or
/// end-of-string ends a sentence. Doesn't handle abbreviations — acceptable
/// for this pass; the invariant that actually matters (never split a
/// sentence mid-way when building a chunk) only needs *a* consistent
/// definition of "sentence", not a perfect one.
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];
        current.push(c);
        if c == '.' || c == '!' || c == '?' {
            let at_boundary = i + 1 >= chars.len() || chars[i + 1].is_whitespace();
            if at_boundary && !current.trim().is_empty() {
                sentences.push(current.trim().to_string());
                current.clear();
            }
        }
    }
    if !current.trim().is_empty() {
        sentences.push(current.trim().to_string());
    }
    sentences
}

/// Heuristic table/verse detection over a paragraph's raw lines.
///
/// - **Table**: at least half the (non-blank) lines look column-aligned —
///   contain a run of 2+ spaces or a tab separating fields.
/// - **Verse**: short lines (low average word count) that mostly don't end
///   in sentence-ending punctuation — i.e. line breaks are structural, not
///   just prose wrapping.
fn classify_paragraph(text: &str) -> Option<ChunkKind> {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() < 2 {
        return None;
    }

    let column_aligned = lines
        .iter()
        .filter(|l| l.contains('\t') || l.contains("  "))
        .count();
    if column_aligned * 2 >= lines.len() {
        return Some(ChunkKind::Table);
    }

    let total_words: usize = lines.iter().map(|l| l.split_whitespace().count()).sum();
    let avg_words = total_words as f64 / lines.len() as f64;
    let sentence_ending = lines
        .iter()
        .filter(|l| {
            let t = l.trim();
            t.ends_with('.') || t.ends_with('!') || t.ends_with('?')
        })
        .count();
    if avg_words <= 8.0 && sentence_ending * 2 < lines.len() {
        return Some(ChunkKind::Verse);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_paragraph_merges_forward_into_next() {
        let paragraphs = vec![
            Paragraph {
                page: 1,
                index: 0,
                text: "A short intro.".to_string(), // ~3 tokens, well under 80
            },
            Paragraph {
                page: 1,
                index: 1,
                text: (0..30)
                    .map(|n| format!("This is sentence number {n} with several words in it."))
                    .collect::<Vec<_>>()
                    .join(" "),
            },
        ];

        let chunks = chunk_paragraphs(paragraphs);

        assert_eq!(chunks.len(), 1, "expected the short paragraph to merge forward, not stand alone");
        assert!(chunks[0].text.starts_with("A short intro."));
        assert_eq!(chunks[0].kind, ChunkKind::Prose);
    }

    #[test]
    fn table_like_paragraph_stays_atomic_and_tagged() {
        let table_text = "Name          Age        Marks\nAmal          20         88\nBinu          21         92";
        let paragraphs = vec![
            Paragraph {
                page: 5,
                index: 2,
                text: table_text.to_string(),
            },
        ];

        let chunks = chunk_paragraphs(paragraphs);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChunkKind::Table);
        assert_eq!(chunks[0].text, table_text);
    }

    #[test]
    fn verse_block_stays_atomic_and_tagged() {
        // Short lines that mostly don't end in sentence punctuation: line
        // breaks are structural, so the block must not be reflowed or split.
        let verse_text =
            "ഒരു വരി കവിത\nരണ്ടാമത്തെ വരി\nമൂന്നാമത്തെ വരി\nനാലാമത്തെ വരി";
        let chunks = chunk_paragraphs(vec![Paragraph {
            page: 7,
            index: 4,
            text: verse_text.to_string(),
        }]);

        assert_eq!(chunks.len(), 1, "a verse block must not be split");
        assert_eq!(chunks[0].kind, ChunkKind::Verse);
        assert_eq!(chunks[0].text, verse_text);
        assert_eq!(chunks[0].page, 7);
        assert_eq!(chunks[0].para_index, 4);
    }

    #[test]
    fn a_table_flushes_pending_prose_rather_than_absorbing_it() {
        // A table boundary is a harder boundary than the token target: the
        // prose before it must not be swept into the table chunk, or the
        // table stops being atomic.
        let chunks = chunk_paragraphs(vec![
            Paragraph {
                page: 1,
                index: 0,
                text: "Some introductory prose before the table.".to_string(),
            },
            Paragraph {
                page: 1,
                index: 1,
                text: "Name          Age\nAmal          20\nBinu          21".to_string(),
            },
        ]);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].kind, ChunkKind::Prose);
        assert_eq!(chunks[1].kind, ChunkKind::Table);
        assert!(!chunks[1].text.contains("introductory prose"));
    }

    #[test]
    fn prose_chunks_respect_the_token_ceiling() {
        let text = (0..120)
            .map(|n| format!("Sentence {n} with a handful of words in it."))
            .collect::<Vec<_>>()
            .join(" ");
        let chunks = chunk_paragraphs(vec![Paragraph {
            page: 1,
            index: 0,
            text,
        }]);

        assert!(chunks.len() > 1);
        // Every chunk but the trailing remainder must sit inside the target
        // band; the last may be short because there is nothing left to merge
        // it forward into.
        for chunk in &chunks[..chunks.len() - 1] {
            assert!(
                chunk.token_count <= TARGET_MAX_TOKENS,
                "chunk of {} tokens exceeds the {TARGET_MAX_TOKENS}-token ceiling",
                chunk.token_count
            );
            assert!(
                chunk.token_count >= TARGET_MIN_TOKENS,
                "chunk of {} tokens is below the {TARGET_MIN_TOKENS}-token target",
                chunk.token_count
            );
        }
    }

    #[test]
    fn consecutive_prose_chunks_overlap_by_one_sentence() {
        let text = (0..120)
            .map(|n| format!("Sentence {n} with a handful of words in it."))
            .collect::<Vec<_>>()
            .join(" ");
        let chunks = chunk_paragraphs(vec![Paragraph {
            page: 1,
            index: 0,
            text,
        }]);

        assert!(chunks.len() > 1);
        for pair in chunks.windows(2) {
            let previous_last = split_sentences(&pair[0].text)
                .pop()
                .expect("chunk has at least one sentence");
            assert!(
                pair[1].text.starts_with(&previous_last),
                "expected a one-sentence overlap; {:?} does not start with {previous_last:?}",
                &pair[1].text[..40.min(pair[1].text.len())]
            );
        }
    }

    #[test]
    fn no_chunk_boundary_falls_mid_sentence() {
        // Enough sentences to force at least one overflow boundary well
        // past the 450-token target.
        let big_paragraph_text = (0..80)
            .map(|n| format!("This is sentence number {n} padded with extra words to add tokens."))
            .collect::<Vec<_>>()
            .join(" ");

        let paragraphs = vec![Paragraph {
            page: 1,
            index: 0,
            text: big_paragraph_text,
        }];

        let chunks = chunk_paragraphs(paragraphs);

        assert!(chunks.len() > 1, "expected the long paragraph to split into multiple chunks");
        for chunk in &chunks {
            let trimmed = chunk.text.trim();
            assert!(
                trimmed.ends_with('.') || trimmed.ends_with('!') || trimmed.ends_with('?'),
                "chunk did not end on a sentence boundary: {trimmed:?}"
            );
            // Every chunk must also *start* cleanly (a capital letter or
            // digit, not fragment punctuation), a cheap proxy for "starts
            // at a sentence boundary".
            assert!(
                trimmed.chars().next().map(|c| c.is_alphanumeric()).unwrap_or(false),
                "chunk did not start on a sentence boundary: {trimmed:?}"
            );
        }
    }
}
