//! Layout-aware PDF text extraction (`IMPLEMENTATION_PLAN.md` §5.1: "LAYOUT
//! PARSE — pdfium text + layout boxes; OCR fallback for scanned Malayalam").
//!
//! ## What this module actually does, and what it does not
//!
//! Extraction is **real**: [`LopdfParser`] runs `lopdf`, a pure-Rust PDF
//! library, over the uploaded bytes and returns the text the PDF's own
//! content streams contain. Nothing here invents or paraphrases text — a page
//! that yields nothing yields an empty page, and that emptiness is reported
//! rather than papered over.
//!
//! Two pieces of §5.1 are **not** implemented, and the code says so rather
//! than pretending otherwise:
//!
//! - **Layout boxes.** `lopdf` exposes content-stream text operators, not a
//!   laid-out page model, so [`TextBlock::bbox`] is always `None`. The field
//!   exists so a pdfium-backed parser can fill it without changing the
//!   [`ParsedDocument`] shape that `clean`/`chunk` consume.
//! - **OCR.** There is no OCR engine in this build (`tesseract`/`pdfium` are
//!   native dependencies that are not vendored here). A scanned page is
//!   therefore *detected* — see [`ParsedPage::looks_scanned`] — and reported
//!   with `ocr_confidence: Some(0.0)`, which drives the document to
//!   `pending_review` instead of `embedded`. That is the honest outcome: the
//!   page's content is genuinely unavailable, so a human is asked to look,
//!   and nothing fabricated reaches the corpus.
//!
//! [`PdfParser`] is the seam between the two: swap in a pdfium+OCR
//! implementation and the rest of the pipeline is unchanged.

use crate::error::{RagError, Result};

/// A single text block on a page. `bbox` is `None` under [`LopdfParser`]
/// (see module docs); a pdfium-backed parser would populate it.
#[derive(Debug, Clone)]
pub struct TextBlock {
    pub text: String,
    pub bbox: Option<(f32, f32, f32, f32)>,
}

#[derive(Debug, Clone)]
pub struct ParsedPage {
    /// 1-indexed page number, matching `documents.page_count` and the `page`
    /// field in the Qdrant payload schema (`IMPLEMENTATION_PLAN.md` §3.2).
    pub page_number: i32,
    pub blocks: Vec<TextBlock>,
    /// `None` = born-digital, no OCR needed (matching `documents.ocr_confidence`,
    /// whose column comment says exactly that). `Some(c)` = this page needed
    /// OCR and the engine reported confidence `c`. With no OCR engine in this
    /// build, a page needing OCR reports `Some(0.0)` — "needed OCR, got
    /// nothing" — which is what routes the document to `pending_review`.
    pub ocr_confidence: Option<f32>,
}

/// A page yielding fewer than this many characters is treated as scanned
/// rather than born-digital. A genuinely near-empty page (a part-title page,
/// say) trips this too, which is the safe direction to err: it asks a human
/// to confirm rather than silently indexing a page whose content was missed.
const SCANNED_PAGE_CHAR_THRESHOLD: usize = 32;

impl ParsedPage {
    /// The page's text, blocks joined with a newline.
    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Whether this page carries too little extractable text to be
    /// born-digital — i.e. it is probably a scan needing OCR.
    pub fn looks_scanned(&self) -> bool {
        self.text().chars().filter(|c| !c.is_whitespace()).count()
            < SCANNED_PAGE_CHAR_THRESHOLD
    }
}

#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub pages: Vec<ParsedPage>,
}

impl ParsedDocument {
    /// The document's overall OCR confidence, for `documents.ocr_confidence`.
    ///
    /// `None` when every page was born-digital. Otherwise the mean confidence
    /// across pages that needed OCR — the lower it is, the more of the
    /// document is unread, which is what the `pending_review` gate keys on.
    pub fn ocr_confidence(&self) -> Option<f32> {
        let scores: Vec<f32> = self.pages.iter().filter_map(|p| p.ocr_confidence).collect();
        if scores.is_empty() {
            return None;
        }
        Some(scores.iter().sum::<f32>() / scores.len() as f32)
    }

    /// Page numbers that produced no usable text.
    pub fn scanned_pages(&self) -> Vec<i32> {
        self.pages
            .iter()
            .filter(|p| p.looks_scanned())
            .map(|p| p.page_number)
            .collect()
    }
}

/// Extracts text (with best-effort layout info) from a PDF's raw bytes.
pub trait PdfParser: Send + Sync {
    fn parse(&self, bytes: &[u8]) -> Result<ParsedDocument>;
}

/// `lopdf`-backed implementation — real extraction, no native dependency.
/// See the module docs for its two documented limits (no layout boxes, no OCR).
pub struct LopdfParser;

impl PdfParser for LopdfParser {
    fn parse(&self, bytes: &[u8]) -> Result<ParsedDocument> {
        if !bytes.starts_with(b"%PDF-") {
            // The upload route sniffs this too; re-checking here keeps the
            // parser honest when called from anywhere else, and turns a
            // corrupt file into a clear permanent failure rather than a
            // confusing library error.
            return Err(RagError::Parse(
                "file does not begin with the %PDF- signature".to_string(),
            ));
        }

        let document =
            lopdf::Document::load_mem(bytes).map_err(|e| RagError::Parse(e.to_string()))?;

        let mut pages = Vec::new();
        // `get_pages()` returns a `BTreeMap<page_number, object_id>`, already
        // ordered by page number.
        for (page_number, _object_id) in document.get_pages() {
            // A single malformed page must not fail the whole document: treat
            // it as unreadable (and therefore review-worthy), the same as a
            // scan, rather than discarding every other page's real content.
            let text = document.extract_text(&[page_number]).unwrap_or_default();

            let blocks = if text.trim().is_empty() {
                Vec::new()
            } else {
                vec![TextBlock { text, bbox: None }]
            };

            let mut page = ParsedPage {
                page_number: page_number as i32,
                blocks,
                ocr_confidence: None,
            };

            if page.looks_scanned() {
                // Needed OCR; no OCR engine available, so nothing was read.
                // Reported honestly as zero confidence rather than omitted.
                page.ocr_confidence = Some(0.0);
            }

            pages.push(page);
        }

        if pages.is_empty() {
            return Err(RagError::Parse("PDF contains no pages".to_string()));
        }

        Ok(ParsedDocument { pages })
    }
}

/// `pdftotext`-backed implementation — the production parser.
///
/// **Why this exists.** [`LopdfParser`] cannot decode Identity-H / CID font
/// encodings, which is how essentially every embedded-subset font in a real
/// textbook is written. Instead of failing it emits the literal string
/// `?Identity-H Unimplemented?` per unmapped glyph, and that string is what
/// got chunked, embedded and stored: measured on this project's own corpus,
/// **55% of indexed chunks contained it and 32% of every stored character was
/// that placeholder**. Retrieval then "succeeded" and handed the tutor a
/// context window of placeholders, so the tutor truthfully reported it could
/// not find the topic in the textbook. The same pages extract cleanly through
/// `pdftotext`.
///
/// **The dependency.** This shells out to `pdftotext` (poppler-utils) rather
/// than binding a PDF library. That is a real external requirement — ingest
/// hosts must have it on `PATH` — accepted deliberately: poppler is the
/// reference implementation of this extraction, and the alternative that keeps
/// everything in-process (`pdfium-render`) ships a native blob of its own.
/// [`LopdfParser`] remains as the no-native-dependency fallback, and
/// [`PopplerParser::available`] reports whether the binary is actually there,
/// so a caller can choose rather than discover it mid-ingest.
///
/// Pages are extracted one at a time (`-f N -l N`) rather than split out of a
/// whole-document dump on form feeds: a page whose own extraction fails then
/// lands as an empty page routed to OCR review, exactly as a scan does,
/// instead of silently shifting every subsequent page number by one. Page
/// numbers are what `turn_state` cites to the student, so an off-by-one here
/// is a wrong citation, not a cosmetic fault.
pub struct PopplerParser;

impl PopplerParser {
    /// Whether `pdftotext` can actually be run. Checked before use so the
    /// caller can fall back deliberately instead of failing per document.
    pub fn available() -> bool {
        std::process::Command::new("pdftotext")
            .arg("-v")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok()
    }

    /// Total pages, from `pdfinfo`-free parsing: `pdftotext` needs an explicit
    /// last page, and asking for one past the end is an error rather than an
    /// empty result. `lopdf` is used only to count pages, which is the one
    /// thing it does reliably regardless of font encoding.
    fn page_count(bytes: &[u8]) -> Result<usize> {
        let document =
            lopdf::Document::load_mem(bytes).map_err(|e| RagError::Parse(e.to_string()))?;
        Ok(document.get_pages().len())
    }

    fn extract_page(path: &std::path::Path, page: usize) -> String {
        let out = std::process::Command::new("pdftotext")
            .args([
                "-f",
                &page.to_string(),
                "-l",
                &page.to_string(),
                "-enc",
                "UTF-8",
                // Reading order, NOT `-layout`. Measured on this corpus: with
                // `-layout` a two-column page is reproduced visually, so every
                // output line splices the left column to the right one and the
                // chunker sees sentences that never existed ("1.6.2 Types and
                // classification    making coke."). Plain mode follows the
                // document order instead and yields whole paragraphs, which is
                // the unit `rag-pipeline.md` chunks on. Losing the coordinates
                // costs nothing here: `bbox` is already `None` either way.
            ])
            .arg(path)
            .arg("-")
            .output();

        match out {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
            // A page that will not extract is not a parse failure for the
            // document: it is reported as empty and picked up by the
            // `looks_scanned` check below, which is what routes it to review.
            Ok(o) => {
                tracing::warn!(
                    page,
                    status = ?o.status.code(),
                    stderr = %String::from_utf8_lossy(&o.stderr).trim(),
                    "pdftotext could not extract this page; treating it as unreadable"
                );
                String::new()
            }
            Err(err) => {
                tracing::warn!(page, %err, "could not run pdftotext for this page");
                String::new()
            }
        }
    }
}

impl PdfParser for PopplerParser {
    fn parse(&self, bytes: &[u8]) -> Result<ParsedDocument> {
        if !bytes.starts_with(b"%PDF-") {
            return Err(RagError::Parse(
                "file does not begin with the %PDF- signature".to_string(),
            ));
        }

        let count = Self::page_count(bytes)?;
        if count == 0 {
            return Err(RagError::Parse("PDF contains no pages".to_string()));
        }

        // `pdftotext` reads a file, not a stream. The temporary lives exactly
        // as long as this parse and is removed on drop, including on the error
        // paths below — curriculum PDFs must not accumulate in the temp dir.
        let dir = tempfile::tempdir().map_err(|e| RagError::Parse(e.to_string()))?;
        let path = dir.path().join("input.pdf");
        std::fs::write(&path, bytes).map_err(|e| RagError::Parse(e.to_string()))?;

        let mut pages = Vec::with_capacity(count);
        for page_number in 1..=count {
            let text = Self::extract_page(&path, page_number);

            let blocks = if text.trim().is_empty() {
                Vec::new()
            } else {
                // `bbox` stays `None`: `-layout` preserves reading order but
                // reports no coordinates. Chunking uses order, not geometry.
                vec![TextBlock { text, bbox: None }]
            };

            let mut page = ParsedPage {
                page_number: page_number as i32,
                blocks,
                ocr_confidence: None,
            };
            if page.looks_scanned() {
                page.ocr_confidence = Some(0.0);
            }
            pages.push(page);
        }

        Ok(ParsedDocument { pages })
    }
}

/// Deterministic parser for tests and for exercising the pipeline without a
/// real PDF. It returns exactly the pages it was constructed with — it does
/// not read the bytes it is handed, and is never used on the ingest path.
#[derive(Debug, Clone, Default)]
pub struct StubParser {
    pub pages: Vec<(i32, String)>,
    /// Set to make every page report this OCR confidence, for exercising the
    /// `pending_review` gate.
    pub ocr_confidence: Option<f32>,
}

impl StubParser {
    pub fn with_pages(pages: Vec<(i32, &str)>) -> Self {
        Self {
            pages: pages
                .into_iter()
                .map(|(n, t)| (n, t.to_string()))
                .collect(),
            ocr_confidence: None,
        }
    }
}

impl PdfParser for StubParser {
    fn parse(&self, _bytes: &[u8]) -> Result<ParsedDocument> {
        Ok(ParsedDocument {
            pages: self
                .pages
                .iter()
                .map(|(page_number, text)| ParsedPage {
                    page_number: *page_number,
                    blocks: vec![TextBlock {
                        text: text.clone(),
                        bbox: None,
                    }],
                    ocr_confidence: self.ocr_confidence,
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_file_without_the_pdf_signature() {
        let err = LopdfParser
            .parse(b"this is not a pdf at all, not even close")
            .expect_err("non-PDF bytes must be rejected");
        assert!(matches!(err, RagError::Parse(_)));
    }

    #[test]
    fn flags_a_text_poor_page_as_scanned() {
        let parsed = StubParser::with_pages(vec![(1, "x")])
            .parse(b"")
            .expect("stub parses");
        assert!(parsed.pages[0].looks_scanned());
        assert_eq!(parsed.scanned_pages(), vec![1]);
    }

    #[test]
    fn does_not_flag_a_text_rich_page() {
        let parsed = StubParser::with_pages(vec![(
            1,
            "This page has a comfortable amount of real extractable prose on it.",
        )])
        .parse(b"")
        .expect("stub parses");
        assert!(!parsed.pages[0].looks_scanned());
        assert!(parsed.scanned_pages().is_empty());
    }

    #[test]
    fn ocr_confidence_is_none_for_a_born_digital_document() {
        let parsed = StubParser::with_pages(vec![(
            1,
            "Plenty of born-digital text extracted straight from the content stream.",
        )])
        .parse(b"")
        .expect("stub parses");
        assert_eq!(parsed.ocr_confidence(), None);
    }

    #[test]
    fn ocr_confidence_averages_pages_that_needed_ocr() {
        let mut stub = StubParser::with_pages(vec![(1, "a"), (2, "b")]);
        stub.ocr_confidence = Some(0.4);
        let parsed = stub.parse(b"").expect("stub parses");
        assert_eq!(parsed.ocr_confidence(), Some(0.4));
    }
}
