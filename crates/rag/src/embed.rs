//! Dense embedding (`IMPLEMENTATION_PLAN.md` §5.1 "EMBED": dense via
//! `gemini-embedding-001`, sparse via Qdrant-native BM25 — sparse is
//! Qdrant's own concern at upsert time, not this module's).
//!
//! `gemini-embedding-001` produces 3072-dim vectors (§3.2), so every
//! `Embedder` implementation here returns exactly that width.

use crate::error::Result;

/// Dense-vector width for `gemini-embedding-001` (`IMPLEMENTATION_PLAN.md` §3.2).
pub const EMBEDDING_DIM: usize = 3072;

#[async_trait::async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

/// Deterministic pseudo-embedding for pipeline plumbing/tests. Hashes the
/// input text into a seeded PRNG and fills a 3072-dim vector — not a real
/// embedding, but stable across calls for the same text, which is enough to
/// exercise chunk -> embed -> Qdrant upsert end-to-end without a live
/// Gemini API key.
pub struct StubEmbedder;

#[async_trait::async_trait]
impl Embedder for StubEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        use rand::{Rng, SeedableRng};

        let seed = seed_from_text(text);
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let vector: Vec<f32> = (0..EMBEDDING_DIM)
            .map(|_| rng.gen_range(-1.0f32..1.0f32))
            .collect();
        Ok(vector)
    }
}

/// A simple, dependency-free string hash (FNV-1a) used only to seed the
/// stub's PRNG — not a cryptographic hash, and not meant to be one.
fn seed_from_text(text: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Real Gemini embedding API client. Not wired up yet — there is no
/// `GEMINI_API_KEY` available in this environment.
pub struct GeminiEmbedder {
    pub api_key: String,
}

#[async_trait::async_trait]
impl Embedder for GeminiEmbedder {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        // TODO(phase2-api-key): wire real Gemini embedding API call once
        // GEMINI_API_KEY is available. Should POST to the
        // `gemini-embedding-001` embeddings endpoint, map non-2xx/network
        // failures to `RagError::Embed`, and assert the response vector is
        // exactly `EMBEDDING_DIM` wide before returning it.
        unimplemented!(
            "GeminiEmbedder::embed is not implemented until GEMINI_API_KEY is available"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stub_embedder_produces_correct_dimension() {
        let embedder = StubEmbedder;
        let vector = embedder.embed("some chunk text").await.unwrap();
        assert_eq!(vector.len(), EMBEDDING_DIM);
    }

    #[tokio::test]
    async fn stub_embedder_is_deterministic() {
        let embedder = StubEmbedder;
        let a = embedder.embed("repeatable text").await.unwrap();
        let b = embedder.embed("repeatable text").await.unwrap();
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn stub_embedder_differs_for_different_text() {
        let embedder = StubEmbedder;
        let a = embedder.embed("text one").await.unwrap();
        let b = embedder.embed("text two").await.unwrap();
        assert_ne!(a, b);
    }
}
