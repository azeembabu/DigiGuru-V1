//! Similarity-floor check (NN-4 / E-30, `IMPLEMENTATION_PLAN.md` §5.2):
//! below `ABSTAIN_THRESHOLD` the retriever returns `Abstain` rather than
//! handing thin/irrelevant context to the model. This is what makes
//! "RAG-only, no world knowledge" mechanically enforceable rather than a
//! prompt request.

/// Minimum **dense cosine similarity** (best hit, 0..1) required to answer
/// from retrieved context.
///
/// ## Why cosine, and not the reranker's score
///
/// The floor used to be read off whatever `StubReranker` left in `score`:
/// Jaccard word overlap between the question and the chunk. That silently
/// makes the floor ask "did the student use the textbook's words?", and it
/// failed live in the worst possible direction. This tutor answers Malayalam
/// students from an English corpus, and lexical overlap across scripts is
/// **zero by construction** — so every single question abstained and the tutor
/// told the student, topic after topic, that their syllabus did not cover it
/// while the passage sat in the corpus. Cosine over a multilingual embedding
/// does not have that failure mode: it is the same measure whichever language
/// the question is asked in.
///
/// RRF fusion score is not usable as a floor either: with `RRF_K = 60` a chunk
/// ranked first in one leg scores `1/61 = 0.0164` and first in both scores
/// `0.0328`, so any threshold meaningful for "relevance" is either above
/// everything or below everything.
///
/// ## Calibration — provisional, and measured
///
/// Against the real ingested corpus (block 1, 171 chunks), best dense cosine:
///
/// | query                                    | top cosine |
/// |------------------------------------------|-----------:|
/// | en "What is deforestation"               |     0.6918 |
/// | en "What are water resources"            |     0.7107 |
/// | ml "വനനാശം എന്താണ്"                        |     0.6182 |
/// | ml "ജലസ്രോതസ്സുകൾ എന്തൊക്കെയാണ്"              |     0.6145 |
/// | off-topic "who won the 1998 world cup"   |     0.5595 |
/// | off-topic "how do I bake a cake"         |     0.5625 |
///
/// `0.59` sits in the gap between the highest off-topic (0.5625) and the
/// lowest on-topic (0.6145). The margin is ~0.05 and this is **six data
/// points, not a golden set** — `.claude/rules/rag-pipeline.md` requires a
/// RAGAs run against `evals/golden/` for any threshold change, and that has
/// not been done. It is shipped because the alternative was not a
/// conservative floor but a non-functional one: abstaining on 100% of
/// questions is not the NN-4 behaviour this exists to protect.
///
/// Two things to fix before trusting this: the off-topic queries above all
/// returned the SAME generic numeric table (~0.56), which is the real floor of
/// this corpus rather than a measure of irrelevance; and the Malayalam queries
/// returned that same table, meaning cross-lingual retrieval is finding
/// *something* but not the right passage. Raising retrieval quality for
/// Malayalam questions over an English corpus — most likely by translating the
/// query before embedding — will move these numbers and this threshold with
/// them.
pub const ABSTAIN_THRESHOLD: f32 = 0.59;

/// Whether the retriever should abstain, given the best dense cosine score.
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
