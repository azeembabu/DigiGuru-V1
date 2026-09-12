//! Similarity-floor check (NN-4 / E-30, `IMPLEMENTATION_PLAN.md` §5.2):
//! below `ABSTAIN_THRESHOLD` the retriever returns `Abstain` rather than
//! handing thin/irrelevant context to the model. This is what makes
//! "RAG-only, no world knowledge" mechanically enforceable rather than a
//! prompt request.

/// Minimum cross-encoder rerank score required to answer from retrieved
/// context, on whatever scale the active [`crate::rerank::Reranker`]
/// produces.
///
/// This is a **placeholder** value. Per `IMPLEMENTATION_PLAN.md` §5.5, the
/// real threshold needs empirical tuning against the golden eval set
/// (`evals/golden/`) once the real cross-encoder (bge-reranker-v2-m3) is
/// wired in — out of scope for this pass. Do not treat `0.35` as validated.
pub const ABSTAIN_THRESHOLD: f32 = 0.35;

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
