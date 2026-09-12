//! Cross-encoder rerank stage (E-27, `IMPLEMENTATION_PLAN.md` §5.2/§5.3):
//! cuts the ~20 hybrid-search candidates down to the top 2-3 chunks that
//! actually get sent to the model — the single largest token-cost lever.

use crate::retrieve::RetrievedChunk;

/// Reorders `candidates` by relevance to `query` and truncates to the top
/// 2-3, overwriting each survivor's [`RetrievedChunk::score`] with the
/// rerank score (replacing whatever hybrid-search/RRF score it carried in).
pub trait Reranker {
    fn rerank(&self, query: &str, candidates: Vec<RetrievedChunk>) -> Vec<RetrievedChunk>;
}

/// Lexical-overlap placeholder reranker.
///
/// TODO(phase2-crossencoder): replace with the real cross-encoder
/// (bge-reranker-v2-m3, self-hosted ONNX — `IMPLEMENTATION_PLAN.md` §2.1)
/// before this ships to students. Jaccard similarity over lowercased word
/// sets is enough to prove the pipeline shape (fetch -> rerank -> truncate
/// -> abstain-check) end to end; it is not a real relevance signal and the
/// `ABSTAIN_THRESHOLD` floor is not meaningful against it.
pub struct StubReranker;

impl StubReranker {
    fn jaccard(query_words: &std::collections::HashSet<&str>, text: &str) -> f32 {
        let text_lower = text.to_lowercase();
        let text_words: std::collections::HashSet<&str> = text_lower.split_whitespace().collect();
        if query_words.is_empty() || text_words.is_empty() {
            return 0.0;
        }
        let intersection = query_words.intersection(&text_words).count();
        let union = query_words.union(&text_words).count();
        if union == 0 {
            0.0
        } else {
            intersection as f32 / union as f32
        }
    }
}

impl Reranker for StubReranker {
    fn rerank(&self, query: &str, mut candidates: Vec<RetrievedChunk>) -> Vec<RetrievedChunk> {
        let query_lower = query.to_lowercase();
        let query_words: std::collections::HashSet<&str> = query_lower.split_whitespace().collect();

        for chunk in &mut candidates {
            chunk.score = Self::jaccard(&query_words, &chunk.text);
        }

        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(3);
        candidates
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dg_core::DocumentId;

    fn chunk(text: &str) -> RetrievedChunk {
        RetrievedChunk {
            document_id: DocumentId::new(),
            chapter: "1".into(),
            topic: "topic".into(),
            page: 1,
            para_index: 0,
            text: text.into(),
            lang: "en".into(),
            score: 0.0,
        }
    }

    /// Proves the stub reranker orders a clearly-more-relevant candidate
    /// above a clearly-irrelevant one, per the task requirement — not a
    /// claim that Jaccard overlap is a good reranker in general.
    #[test]
    fn ranks_relevant_chunk_above_irrelevant_one() {
        let reranker = StubReranker;
        let candidates = vec![
            chunk("the weather today is sunny and warm outside"),
            chunk("photosynthesis converts light energy into chemical energy in plants"),
        ];

        let ranked = reranker.rerank(
            "how does photosynthesis convert light energy into chemical energy",
            candidates,
        );

        assert!(ranked[0].text.contains("photosynthesis"));
        assert!(ranked[0].score > ranked[1].score);
    }

    #[test]
    fn truncates_to_top_three() {
        let reranker = StubReranker;
        let candidates = (0..10).map(|i| chunk(&format!("chunk number {i}"))).collect();
        let ranked = reranker.rerank("chunk", candidates);
        assert!(ranked.len() <= 3);
    }
}
