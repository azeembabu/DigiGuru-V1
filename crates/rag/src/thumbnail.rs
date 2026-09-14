//! First-page rasterisation — the cover image a student recognises a unit by.
//!
//! # Why this is optional rather than required
//!
//! Rendering a PDF page is not text extraction: it needs a full layout and
//! glyph-rasterising engine, which in practice means pdfium, a native C++
//! library. `parse.rs` already explains why this build does not vendor one —
//! the same reason applies here, so the binding is resolved **at runtime** and
//! its absence is a downgrade, not a failure.
//!
//! [`Renderer::available`] is therefore the honest question a caller asks
//! first, and [`Renderer::first_page_webp`] returns `Ok(None)` rather than an
//! error when there is no engine to render with. A deployment without pdfium
//! ingests exactly as it did before and simply serves no covers — mirroring
//! how a page needing OCR is *reported* rather than faked.
//!
//! # Where the library is looked for
//!
//! 1. `PDFIUM_LIB_PATH` — an explicit path to the library file or its
//!    containing directory. Set this in a container image that ships one.
//! 2. The directory holding the running binary, so an operator can drop
//!    `pdfium.dll`/`libpdfium.so` next to `ingest-worker` with no config.
//! 3. The system loader path.
//!
//! The binding is resolved **once per thread** and cached. It is a thread-local
//! rather than a process-wide static because `Pdfium` is neither `Send` nor
//! `Sync`, so it cannot live in a `static` at all — and rendering belongs on a
//! `spawn_blocking` thread anyway (`.claude/rules/code-style.md`: CPU-bound
//! work never runs on an async worker). Tokio reuses its blocking threads, so
//! the library is loaded a handful of times per process, not once per upload.
//!
//! # Why WebP, and why this size
//!
//! A cover is decoration on a navigation screen — it is rendered at 96x128 CSS
//! pixels, and the student is choosing between six of them, not reading one.
//! WebP at that size is a few kilobytes where a PNG is tens, and the whole list
//! of covers should cost less than the JSON around it. The render is done at
//! 2x for high-density screens and no larger: a full-resolution page image
//! would be a copy of the textbook, which is not what a thumbnail is for.

use std::path::PathBuf;

use crate::error::{RagError, Result};

/// Rendered width in device pixels — 2x the 96 CSS px the card paints, so the
/// cover stays sharp on a phone without shipping a page-sized image.
pub const THUMBNAIL_WIDTH: u16 = 192;

/// Cap on the rendered height, for the same reason. A page taller than A4
/// (a scanned foolscap sheet) is fitted inside this rather than stretched:
/// aspect ratio is preserved by pdfium when both bounds are given.
pub const THUMBNAIL_MAX_HEIGHT: u16 = 256;

/// What the renderer decided, so a caller can log the difference between "no
/// engine" and "this PDF could not be drawn" without matching on strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailable {
    /// No pdfium binding could be resolved in this process.
    NoEngine,
    /// The document has no pages to render.
    EmptyDocument,
}

/// A process-wide handle to the rasterising engine.
///
/// Cheap to construct repeatedly: the underlying binding is resolved once and
/// shared, so this is a lookup rather than a library load.
pub struct Renderer;

impl Renderer {
    /// Is there an engine to render with on *this thread*?
    ///
    /// Per-thread because the binding is (see the module docs), but the answer
    /// is in practice process-wide: it depends only on whether the library is
    /// on disk. Worth calling once at startup so the operator learns from the
    /// log that covers will be missing, rather than from an empty card weeks
    /// later.
    pub fn available() -> bool {
        with_binding(|bound| bound)
    }

    /// Render page 1 of `pdf_bytes` to WebP.
    ///
    /// `Ok(None)` means *no cover can be produced* and the caller should carry
    /// on — no engine, or a document with no pages. `Err` is reserved for a
    /// PDF that an engine was available for and still could not open, which is
    /// a real property of the upload and worth surfacing.
    pub fn first_page_webp(pdf_bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        match Self::render(pdf_bytes) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(RenderOutcome::Skipped(reason)) => {
                tracing::debug!(?reason, "no cover rendered");
                Ok(None)
            }
            Err(RenderOutcome::Failed(detail)) => Err(RagError::Parse(detail)),
        }
    }

    #[cfg(feature = "pdfium")]
    fn render(pdf_bytes: &[u8]) -> std::result::Result<Vec<u8>, RenderOutcome> {
        use pdfium_render::prelude::*;

        PDFIUM.with(|slot| {
            let pdfium = slot
                .get_or_init(resolve_binding)
                .as_ref()
                .ok_or(RenderOutcome::Skipped(Unavailable::NoEngine))?;

            let document = pdfium
                .load_pdf_from_byte_slice(pdf_bytes, None)
                .map_err(|err| RenderOutcome::Failed(format!("opening the pdf to render: {err}")))?;

            let page = document
                .pages()
                .first()
                .map_err(|_| RenderOutcome::Skipped(Unavailable::EmptyDocument))?;

            let config = PdfRenderConfig::new()
                .set_target_width(THUMBNAIL_WIDTH as i32)
                .set_maximum_height(THUMBNAIL_MAX_HEIGHT as i32);

            let image = page
                .render_with_config(&config)
                .map_err(|err| RenderOutcome::Failed(format!("rendering page 1: {err}")))?
                .as_image()
                .map_err(|err| RenderOutcome::Failed(format!("converting the render: {err}")))?
                .into_rgb8();

            // RGB8 rather than RGBA: a page is opaque, and dropping the alpha
            // channel is a quarter off the encoded size for no visible
            // difference at 192px wide.
            let mut out = std::io::Cursor::new(Vec::new());
            image
                .write_to(&mut out, image::ImageFormat::WebP)
                .map_err(|err| RenderOutcome::Failed(format!("encoding the cover: {err}")))?;

            Ok(out.into_inner())
        })
    }

    /// Built without the `pdfium` feature: there is no engine, by construction.
    ///
    /// The feature exists so a build that cannot supply the native library
    /// does not have to carry the dependency at all, while the call sites and
    /// the database column stay identical either way.
    #[cfg(not(feature = "pdfium"))]
    fn render(_pdf_bytes: &[u8]) -> std::result::Result<Vec<u8>, RenderOutcome> {
        Err(RenderOutcome::Skipped(Unavailable::NoEngine))
    }
}

/// Internal three-way result: rendered, deliberately skipped, or genuinely
/// failed. Kept private so the public surface stays `Result<Option<_>>`.
enum RenderOutcome {
    Skipped(Unavailable),
    /// Only reachable with an engine present: without one, every document is
    /// skipped before anything can fail.
    #[cfg_attr(not(feature = "pdfium"), allow(dead_code))]
    Failed(String),
}

#[cfg(feature = "pdfium")]
thread_local! {
    /// The resolved binding for this thread. `OnceCell<Option<_>>` rather than
    /// `OnceCell<_>` so that a *failure* to find the library is remembered too
    /// — a missing library must not be re-probed for every document.
    static PDFIUM: std::cell::OnceCell<Option<pdfium_render::prelude::Pdfium>> =
        const { std::cell::OnceCell::new() };
}

#[cfg(feature = "pdfium")]
fn resolve_binding() -> Option<pdfium_render::prelude::Pdfium> {
    use pdfium_render::prelude::*;

    for candidate in search_paths() {
        let path = Pdfium::pdfium_platform_library_name_at_path(&candidate);
        if let Ok(bindings) = Pdfium::bind_to_library(&path) {
            tracing::info!(path = %path.display(), "pdfium bound; unit covers will be rendered");
            return Some(Pdfium::new(bindings));
        }
    }

    match Pdfium::bind_to_system_library() {
        Ok(bindings) => {
            tracing::info!("pdfium bound from the system library path");
            Some(Pdfium::new(bindings))
        }
        Err(err) => {
            tracing::warn!(
                error = %err,
                "no pdfium library found; units will have no cover image. \
                 Set PDFIUM_LIB_PATH or place the library beside the binary."
            );
            None
        }
    }
}

/// Answers `f(is_bound)` without handing the non-`Send` binding out of the
/// thread that owns it.
#[cfg(feature = "pdfium")]
fn with_binding<T>(f: impl FnOnce(bool) -> T) -> T {
    PDFIUM.with(|slot| f(slot.get_or_init(resolve_binding).is_some()))
}

/// Built without the `pdfium` feature: there is never a binding.
#[cfg(not(feature = "pdfium"))]
fn with_binding<T>(f: impl FnOnce(bool) -> T) -> T {
    f(false)
}

/// Directories to probe, in order, before falling back to the system loader.
#[cfg_attr(not(feature = "pdfium"), allow(dead_code))]
fn search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(configured) = std::env::var("PDFIUM_LIB_PATH") {
        let configured = PathBuf::from(configured);
        // Accept either the library file itself or the directory holding it,
        // because both are what an operator naturally sets.
        if let Some(parent) = configured.parent().filter(|_| configured.is_file()) {
            paths.push(parent.to_path_buf());
        } else {
            paths.push(configured);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.to_path_buf());
        }
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The contract the ingest worker depends on: no engine is a skip, never
    /// an error, so a build without pdfium still completes every ingestion.
    #[test]
    fn missing_engine_is_not_an_error() {
        if Renderer::available() {
            return; // This build can render; the skip path is not exercised.
        }
        let result = Renderer::first_page_webp(b"%PDF-1.7\n").expect("a skip is not an error");
        assert!(result.is_none());
    }

    /// Bytes that are not a PDF must not produce a cover. With an engine they
    /// fail to open; without one they are skipped. Neither may return an image.
    #[test]
    fn non_pdf_bytes_never_yield_a_cover() {
        let outcome = Renderer::first_page_webp(b"this is not a pdf at all");
        assert!(matches!(outcome, Ok(None) | Err(_)));
    }
}
