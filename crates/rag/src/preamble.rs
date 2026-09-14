//! The per-block static preamble and its prompt-cache registration.
//!
//! `.claude/rules/rag-pipeline.md`, cost discipline:
//!
//! > The static preamble (tutor instructions + block outline) is
//! > prompt-cached per block; the handle lives in `pcache:{block_id}`.
//!
//! The preamble is *static per block* — tutor instructions plus the block's
//! chapter/topic outline, which only changes when the block's documents are
//! re-ingested. That is exactly why ingestion owns writing it: the outline
//! is derived from the chunks this run just produced, so the cache entry and
//! the indexed corpus are written from the same data in the same pass and
//! cannot drift apart.
//!
//! ## What is real here and what is not
//!
//! The Redis side is real: [`register_preamble`] writes the preamble and its
//! content hash to `pcache:{block_id}`. The *Gemini* side is not — turning a
//! preamble into a provider-side cached-content handle requires a live
//! `GEMINI_API_KEY` (see [`crate::embed::GeminiEmbedder`] for the same
//! constraint). So the `handle` stored here is a deterministic content hash,
//! not a provider handle, and [`CachedPreamble::provider_handle`] is `None`
//! until that call is wired. A reader must therefore treat a `None` handle as
//! "send the preamble text inline", which is the correct (merely un-optimised)
//! behaviour rather than a broken one.

use std::collections::BTreeSet;

use dg_core::BlockId;

use crate::error::{RagError, Result};

/// Redis key holding a block's cached preamble (`rag-pipeline.md`).
pub fn pcache_key(block_id: BlockId) -> String {
    format!("pcache:{block_id}")
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CachedPreamble {
    /// Content hash of `text`. Changes whenever the block outline changes, so
    /// a reader can tell a stale cache entry from a current one.
    pub handle: String,
    /// The provider-side prompt-cache handle, once one can be created.
    /// `None` means "no provider cache — send `text` inline".
    pub provider_handle: Option<String>,
    pub text: String,
}

/// Builds the static preamble for a block from its chapter list.
///
/// Takes the WHOLE block's chapters, not one ingest run's. The outline is a
/// property of the block — it is what tells the tutor what this block
/// contains — so deriving it from a single document made every ingest
/// overwrite the last, and the tutor was handed the final document as though
/// it were the entire syllabus.
pub fn build_preamble<S: AsRef<str>>(block_no: i16, chapters: &[S]) -> String {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut outline: Vec<&str> = Vec::new();
    for chapter in chapters {
        let chapter = chapter.as_ref();
        if !chapter.trim().is_empty() && seen.insert(chapter) {
            outline.push(chapter);
        }
    }

    let mut text = String::new();
    text.push_str(&format!("Block {block_no} outline:\n"));
    for (position, chapter) in outline.iter().enumerate() {
        text.push_str(&format!("{}. {}\n", position + 1, chapter));
    }
    text
}

/// FNV-1a over the preamble text — a stable content hash, not a
/// cryptographic one, used only to detect a changed outline.
fn content_hash(text: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

/// Writes the block's preamble to `pcache:{block_id}`.
///
/// No TTL: the entry is valid until the block is re-ingested, and this is the
/// only writer. An expiring entry would silently push every turn back to
/// sending the preamble inline — the exact cost regression the rule exists to
/// prevent — with nothing to make the regression visible.
pub async fn register_preamble(
    redis: &mut redis::aio::ConnectionManager,
    block_id: BlockId,
    preamble_text: String,
) -> Result<CachedPreamble> {
    let cached = CachedPreamble {
        handle: content_hash(&preamble_text),
        provider_handle: None,
        text: preamble_text,
    };

    let encoded = serde_json::to_string(&cached)
        .map_err(|e| RagError::Cache(format!("preamble serialisation failed: {e}")))?;

    redis::cmd("SET")
        .arg(pcache_key(block_id))
        .arg(encoded)
        .query_async::<()>(redis)
        .await
        .map_err(|e| RagError::Cache(format!("redis pcache write failed: {e}")))?;

    Ok(cached)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn outline_lists_each_chapter_once_in_reading_order() {
        let chapters = ["Chapter One", "Chapter One", "Chapter Two", "Chapter One"];

        let preamble = build_preamble(3, &chapters);

        assert!(preamble.contains("1. Chapter One"));
        assert!(preamble.contains("2. Chapter Two"));
        assert_eq!(preamble.matches("Chapter One").count(), 1);
    }

    /// The outline must list EVERY unit in the block.
    ///
    /// Regression test for a live failure: the outline was built from one
    /// ingest run, so each document overwrote the block key and the tutor was
    /// told a six-unit block contained only the last-ingested unit. It then
    /// kept abandoning the unit the student had opened to teach that one.
    #[test]
    fn outline_covers_every_unit_in_the_block() {
        let preamble = build_preamble(
            1,
            &["Unit 1 Environmental Segments", "Unit 4 Water Resources"],
        );
        assert!(preamble.contains("1. Unit 1 Environmental Segments"));
        assert!(preamble.contains("2. Unit 4 Water Resources"));
    }

    #[test]
    fn key_matches_the_documented_shape() {
        let id = BlockId::from_uuid(Uuid::nil());
        assert_eq!(pcache_key(id), format!("pcache:{id}"));
    }

    #[test]
    fn handle_changes_when_the_outline_changes() {
        let a = content_hash(&build_preamble(3, &["Chapter One"]));
        let b = content_hash(&build_preamble(3, &["Chapter Two"]));
        assert_ne!(a, b);
    }
}
