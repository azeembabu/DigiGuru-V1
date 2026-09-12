//! Layout-aware PDF text extraction (`IMPLEMENTATION_PLAN.md` §5.1: "LAYOUT
//! PARSE — pdfium text + layout boxes; OCR fallback for scanned Malayalam").
//!
//! This pass uses `lopdf` (pure-Rust, no native binary dependency) rather
//! than `pdfium-render` to keep the build simple in this environment. It
//! extracts text per page; it does not attempt real layout-box detection —
//! callers downstream (`clean`, `chunk`) work over plain per-page text.

use crate::error::{RagError, Result};

/// A single text block on a page. `lopdf`'s content-stream text operators do
/// not give us real bounding boxes without a full layout engine, so `bbox`
/// is left `None` for now — this field exists so a future pdfium-backed
/// parser can populate it without changing the `ParsedDocument` shape.
#[derive(Debug, Clone)]
pub struct TextBlock {
    pub text: String,
    pub bbox: Option<(f32, f32, f32, f32)>,
}

#[derive(Debug, Clone)]
pub struct ParsedPage {
    /// 1-indexed page number, matching `documents.page_count` and the
    /// `page` field in the Qdrant payload schema (`IMPLEMENTATION_PLAN.md`
    /// §3.2).
    pub page_number: i32,
    pub blocks: Vec<TextBlock>,
    // TODO(phase2-ocr): scanned/low-text-density pages should be flagged
    // here (e.g. via a text-density heuristic: blocks.len() == 0 or very
    // low character count for the page's expected content) and routed
    // through OCR. For this pass they simply pass through as-is with
    // `ocr_confidence: None` at the `documents` row level — no OCR attempt
    // is made.
    pub ocr_confidence: Option<f32>,
}

impl ParsedPage {
    /// The page's text, blocks joined with a newline — the common case
    /// `clean`/`chunk` want.
    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub pages: Vec<ParsedPage>,
}

/// Extracts text (with best-effort layout info) from a PDF's raw bytes.
pub trait PdfParser {
    fn parse(&self, bytes: &[u8]) -> Result<ParsedDocument>;
}

/// `lopdf`-backed implementation. Pure-Rust, no native `pdfium` binary to
/// vendor or link — simplest thing that extracts real text per page.
pub struct LopdfParser;

impl PdfParser for LopdfParser {
    fn parse(&self, bytes: &[u8]) -> Result<ParsedDocument> {
        let document =
            lopdf::Document::load_mem(bytes).map_err(|e| RagError::Parse(e.to_string()))?;

        let mut pages = Vec::new();
        // `get_pages()` returns a `BTreeMap<page_number, object_id>`, already
        // ordered by page number.
        for (page_number, object_id) in document.get_pages() {
            let text = document
                .extract_text(&[page_number])
                .map_err(|e| RagError::Parse(e.to_string()))?;
            let _ = object_id; // page identity only, not needed beyond extraction

            let blocks = if text.trim().is_empty() {
                Vec::new()
            } else {
                vec![TextBlock {
                    text,
                    bbox: None,
                }]
            };

            pages.push(ParsedPage {
                page_number: page_number as i32,
                // No text density -> no OCR attempted this pass -> None.
                // A real OCR-confidence value only ever comes from an OCR
                // engine we don't run yet.
                ocr_confidence: None,
                blocks,
            });
        }

        Ok(ParsedDocument { pages })
    }
}
