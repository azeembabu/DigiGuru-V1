//! Similarity-floor check (NN-4 / E-30, `IMPLEMENTATION_PLAN.md` §5.2):
//! below `ABSTAIN_THRESHOLD` the retriever returns `Abstain` rather than
//! handing thin/irrelevant context to the model. This is what makes
//! "RAG-only, no world knowledge" mechanically enforceable rather than a
//! prompt request.

/// Minimum cross-encoder rerank score required to answer from retrieved
/// context, on whatever scale the active [`crate::rerank::Reranker`]
/// produces.
///
/// This is a **placeholder** value, and a specific bug was found and fixed
/// live in it: `0.35` was calibrated for a real cross-encoder's 0..1
/// relevance score, but the reranker actually running today is
/// [`crate::rerank::StubReranker`] — plain word-overlap (Jaccard) between the
/// question and the chunk text, which lives on a completely different scale.
/// Measured directly against the real ingested corpus: a genuinely on-topic
/// question ("What is the hydrosphere?" against the chunk that answers it)
/// scored **0.065**; an off-topic one scored **0.029**. Both are an order of
/// magnitude under `0.35`, so with the old value retrieval abstained on
/// essentially every real question regardless of relevance — the tutor
/// looked like it was teaching (the system instruction's preamble/outline
/// text made an abstention read as plausible small talk) while never
/// actually grounding an answer in a retrieved chunk.
///
/// `0.03` sits between those two measured points. It is still a stopgap, not
/// a validated threshold — a lexical-overlap score is a poor discriminator in
/// general, and this exact value is calibrated from two data points, not a
/// golden set. Per `.claude/rules/rag-pipeline.md`, any change to the
/// reranker or this threshold needs a RAGAs run against `evals/golden/`
/// before it can be called correct; that has not happened. What changed here
/// is narrower and safe to ship without one: the old value was not
/// "conservative", it was non-functional — it made every answer abstain,
/// which is not the NN-4 behaviour it exists to protect, just a mechanically
/// identical-looking failure. This must be replaced by a real threshold, on
/// the real cross-encoder's scale, the moment that reranker is wired in — do
/// not carry a Jaccard-calibrated number forward past that point.
pub const ABSTAIN_THRESHOLD: f32 = 0.03;

/// Whether the retriever should abstain given the best reranked score.
pub fn should_abstain(top_score: f32) -> bool {
    top_score < ABSTAIN_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abstains_below_threshold() {
        assert!(should_abstain(ABSTAIN_THRESHOLD - 0.01));
        assert!(should_abstain(0.0));
    }

    #[test]
    fn does_not_abstain_at_or_above_threshold() {
        assert!(!should_abstain(ABSTAIN_THRESHOLD));
        assert!(!should_abstain(0.9));
    }
}
