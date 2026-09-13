//! Shared BM25-placeholder sparse-vector encoding, used identically by
//! ingestion (`qdrant::upsert_chunks`) and retrieval (`retrieve::search_sparse`).
//!
//! TODO(phase2-bm25): this is a naive hashed-token term-frequency stand-in,
//! not real BM25 (no IDF, no document-length normalisation, no corpus
//! statistics). It must be replaced before the abstention threshold or
//! retrieval-quality evals (`IMPLEMENTATION_PLAN.md` §5.5) can be trusted.
//! Both call sites MUST use this shared function — encoding chunk text and
//! query text with different schemes makes sparse search silently return
//! nothing meaningful, since the vectors would live in unrelated spaces.

use std::collections::HashMap;

use qdrant_client::qdrant::SparseVector;

/// Hashed-token term-frequency encoding of `text` into the `bm25` sparse
/// vector space. Same encoding must run at upsert time and at query time.
pub fn sparse_encode(text: &str) -> SparseVector {
    let mut term_counts: HashMap<u32, f32> = HashMap::new();
    for token in text.to_lowercase().split_whitespace() {
        let mut hash: u32 = 2166136261;
        for byte in token.bytes() {
            hash ^= byte as u32;
            hash = hash.wrapping_mul(16777619);
        }
        let index = hash % 100_003;
        *term_counts.entry(index).or_insert(0.0) += 1.0;
    }

    // Sorted by index, not left in `HashMap` iteration order: that order
    // varies run to run (the hasher is randomly seeded), so the same text
    // would otherwise encode to a differently-ordered vector on each call.
    // Qdrant pairs `indices[i]` with `values[i]`, so the pairing stays correct
    // either way — but an unstable order makes the encoding untestable and
    // makes two encodings of the same text impossible to compare.
    let mut pairs: Vec<(u32, f32)> = term_counts.into_iter().collect();
    pairs.sort_unstable_by_key(|(index, _)| *index);

    let mut indices: Vec<u32> = Vec::with_capacity(pairs.len());
    let mut values: Vec<f32> = Vec::with_capacity(pairs.len());
    for (index, value) in pairs {
        indices.push(index);
        values.push(value);
    }

    SparseVector { indices, values }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_text_same_encoding() {
        let a = sparse_encode("hello world");
        let b = sparse_encode("hello world");
        assert_eq!(a.indices, b.indices);
        assert_eq!(a.values, b.values);
    }

    #[test]
    fn different_text_different_encoding() {
        let a = sparse_encode("hello world");
        let b = sparse_encode("completely different text");
        assert_ne!(a.indices, b.indices);
    }
}
