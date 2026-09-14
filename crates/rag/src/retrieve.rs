//! Hybrid retrieval pipeline — `IMPLEMENTATION_PLAN.md` §5.2:
//!
//! ```text
//! question -> normalise + language detect
//!   -> hybrid search in Qdrant (dense + BM25, RRF fusion), top_k = 20   [E-26]
//!      MANDATORY filter: program_id AND semester_no AND course_id AND block_no [E-25]
//!   -> cross-encoder rerank -> top 2..3                                 [E-27]
//!   -> similarity floor check -> Abstain below ABSTAIN_THRESHOLD        [NN-4 / E-30]
//! ```
//!
//! ## Interface assumptions
//!
//! This module is written against two types owned by the parallel
//! ingestion agent, which may not exist yet in this worktree at the time
//! this file was written:
//!
//! - `crate::embed::Embedder` — assumed to be an `async_trait`-based,
//!   object-safe trait: `async fn embed(&self, text: &str) ->
//!   Result<Vec<f32>, RagError>`, returning the 3072-dim dense embedding
//!   (`gemini-embedding-001`, `IMPLEMENTATION_PLAN.md` §2.1/§3.2).
//! - `crate::error::RagError` — assumed to implement `std::error::Error`
//!   and to have a `From<qdrant_client::QdrantError>` (or equivalent)
//!   conversion, so `?` composes against Qdrant client calls.
//!
//! If either assumption doesn't match what actually lands, only the
//! imports/error-plumbing in this file should need adjusting — the
//! filter-construction, fusion, and abstention logic below do not depend on
//! their internals. Flagged in the task report.
//!
//! The exact `qdrant-client` wire-level API (field names on `SearchPoints`,
//! sparse-vector construction) also varies across crate versions; this is
//! written against the "classic" `QdrantClient`/`SearchPoints` shape the
//! task brief names explicitly, with the BM25 sparse-vector encoding left
//! as an explicit stub (see `sparse_encode` below) since the real BM25
//! term-weighting used at ingest time is owned by the ingestion side
//! (`crates/rag/src/qdrant.rs` / `embed.rs`) and must match whatever this
//! function does bit-for-bit to be useful — noted as the single biggest
//! cross-agent interface risk in the task report.

use std::collections::HashMap;

use qdrant_client::qdrant::{
    value::Kind as QdrantValueKind, Condition, Filter, Query, QueryPointsBuilder, ScoredPoint,
    Value as QdrantValue, VectorInput,
};
use qdrant_client::Qdrant;

use dg_core::{CourseId, DocumentId, ProgramId};

use crate::abstain::should_abstain;
use crate::embed::Embedder;
use crate::error::RagError;
use crate::rerank::{Reranker, StubReranker};

/// The Qdrant collection name (`IMPLEMENTATION_PLAN.md` §3.2).
pub const COLLECTION: &str = "curriculum";

/// Candidates pulled from each of the dense/sparse legs before RRF fusion
/// and rerank (E-26).
const TOP_K: u64 = 20;

/// RRF fusion constant (`1 / (k + rank)`), using the conventional default
/// of 60 also used by Qdrant's own built-in RRF fusion.
const RRF_K: f32 = 60.0;

/// The four mandatory filter dimensions (E-25) plus the question text.
/// Every field is required (no `Option`s) by design: there is no way to
/// construct a `RetrievalQuery` — and therefore no way to call [`retrieve`]
/// — without all four, so an unfiltered query is a compile error, not a
/// runtime bug to catch in review.
#[derive(Debug, Clone)]
pub struct RetrievalQuery {
    pub question: String,
    pub program_id: ProgramId,
    pub semester_no: i32,
    pub course_id: CourseId,
    pub block_no: i32,
    /// Narrows to one uploaded unit within the block, when the student opened
    /// the classroom on a specific one.
    ///
    /// Strictly **additional**: the four mandatory filters are applied whether
    /// this is set or not, so it can only ever shrink the result set, never
    /// widen it past the block. That is what keeps E-25 intact — this is not a
    /// fifth alternative to the four, it is a narrowing inside them.
    ///
    /// `None` searches the whole block, which is the right default: a student
    /// who navigated to a block rather than a unit, or a block holding a single
    /// unit, should not have retrieval scoped more tightly than they asked for.
    pub document_id: Option<DocumentId>,
}

/// One retrieved (and, post-rerank, reranked) chunk, carrying the full
/// citation payload so the tutor cites `chapter`/`topic`/`page` from real
/// metadata rather than from memory (F-34).
#[derive(Debug, Clone)]
pub struct RetrievedChunk {
    pub document_id: DocumentId,
    pub chapter: String,
    pub topic: String,
    pub page: i32,
    pub para_index: i32,
    pub text: String,
    pub lang: String,
    /// RRF fusion score until reranked, then overwritten with the rerank
    /// score by whichever [`Reranker`] is in use.
    pub score: f32,
}

/// Result of the retrieval pipeline. `Abstain` is a first-class outcome
/// (NN-4 / E-30) — callers must handle it explicitly rather than treating
/// an empty `Vec` as equivalent.
#[derive(Debug, Clone)]
pub enum RetrievalOutcome {
    Chunks(Vec<RetrievedChunk>),
    Abstain,
}

/// Placeholder normaliser + language detector.
///
/// TODO(phase2-langdetect): replace with a real language identifier (e.g.
/// `whatlang`, or fastText `lid.176`) that distinguishes Malayalam from
/// English. For now every question is tagged `"ml"`, matching the
/// majority-Malayalam target corpus, per `IMPLEMENTATION_PLAN.md` §5.2.
fn normalise_and_detect_language(question: &str) -> (String, &'static str) {
    (question.trim().to_string(), "ml")
}

/// Runs the full retrieval pipeline for one student question.
pub async fn retrieve(
    client: &Qdrant,
    query: &RetrievalQuery,
    embedder: &dyn Embedder,
) -> Result<RetrievalOutcome, RagError> {
    let (normalised_question, lang) = normalise_and_detect_language(&query.question);

    let filter = mandatory_filter(query);

    let dense_vector = embedder.embed(&normalised_question).await?;

    let dense_results = search_dense(client, &filter, dense_vector).await?;
    let sparse_results = search_sparse(client, &filter, &normalised_question, lang).await?;

    // The abstention signal is the best DENSE COSINE, captured before fusion
    // and before reranking. See `abstain::ABSTAIN_THRESHOLD` for why it is this
    // number and not the RRF score or the reranker's: in short, RRF scores live
    // on a scale where no threshold is meaningful, and the stub reranker scores
    // lexical overlap, which is zero for a Malayalam question over an English
    // corpus and made every question abstain.
    //
    // `search_dense` returns Qdrant's ordering, so the first result is the best
    // match. An empty dense leg means nothing cleared the four mandatory
    // filters, which is an abstention on its own terms.
    let retrieval_score = dense_results.first().map(|c| c.score).unwrap_or(0.0);

    let fused = rrf_fuse(dense_results, sparse_results);
    if fused.is_empty() {
        return Ok(RetrievalOutcome::Abstain);
    }

    if should_abstain(retrieval_score) {
        return Ok(RetrievalOutcome::Abstain);
    }

    // Reranking decides ORDER and the 2..3 cut (E-27). It does not decide
    // whether we retrieved anything worth teaching.
    let reranked = StubReranker.rerank(&normalised_question, fused);

    Ok(RetrievalOutcome::Chunks(reranked))
}

/// The mandatory `program_id AND semester_no AND course_id AND block_no`
/// filter (E-25), plus an optional `document_id` narrowing. Both search legs
/// in [`retrieve`] go through this — there is no code path here that builds a
/// Qdrant query without it.
///
/// The four mandatory conditions are pushed unconditionally and first; the
/// document condition is only ever appended. There is deliberately no branch
/// that produces a filter *without* the four, because the failure mode that
/// protects against — a query that escapes its course or block — is a
/// correctness bug, not a relevance one (`rag-pipeline.md`).
fn mandatory_filter(query: &RetrievalQuery) -> Filter {
    let mut conditions = vec![
        Condition::matches("program_id", query.program_id.to_string()),
        Condition::matches("semester_no", query.semester_no as i64),
        Condition::matches("course_id", query.course_id.to_string()),
        Condition::matches("block_no", query.block_no as i64),
    ];
    if let Some(document_id) = query.document_id {
        conditions.push(Condition::matches("document_id", document_id.to_string()));
    }
    Filter::must(conditions)
}

async fn search_dense(
    client: &Qdrant,
    filter: &Filter,
    dense_vector: Vec<f32>,
) -> Result<Vec<RetrievedChunk>, RagError> {
    let response = client
        .query(
            QueryPointsBuilder::new(COLLECTION)
                .query(Query::new_nearest(dense_vector))
                .using(crate::qdrant::DENSE_VECTOR_NAME)
                .filter(filter.clone())
                .limit(TOP_K)
                .with_payload(true),
        )
        .await?;

    Ok(response
        .result
        .into_iter()
        .filter_map(scored_point_to_chunk)
        .collect())
}

async fn search_sparse(
    client: &Qdrant,
    filter: &Filter,
    question: &str,
    _lang: &str,
) -> Result<Vec<RetrievedChunk>, RagError> {
    // Shared with `qdrant::upsert_chunks` — see `crate::sparse` doc comment.
    // Encoding query text and chunk text with different schemes would put
    // them in unrelated vector spaces and make sparse search silently
    // useless.
    let sparse = crate::sparse::sparse_encode(question);

    let response = client
        .query(
            QueryPointsBuilder::new(COLLECTION)
                .query(Query::new_nearest(VectorInput::new_sparse(
                    sparse.indices,
                    sparse.values,
                )))
                .using(crate::qdrant::SPARSE_VECTOR_NAME)
                .filter(filter.clone())
                .limit(TOP_K)
                .with_payload(true),
        )
        .await?;

    Ok(response
        .result
        .into_iter()
        .filter_map(scored_point_to_chunk)
        .collect())
}

/// Client-side Reciprocal Rank Fusion of the dense and sparse result lists
/// (E-26). Identity for dedup/merge purposes is `(document_id, para_index)`
/// — the pair that uniquely identifies a chunk in the payload schema
/// (`IMPLEMENTATION_PLAN.md` §3.2).
fn rrf_fuse(dense: Vec<RetrievedChunk>, sparse: Vec<RetrievedChunk>) -> Vec<RetrievedChunk> {
    let mut rrf_scores: HashMap<(DocumentId, i32), f32> = HashMap::new();
    let mut chunks_by_key: HashMap<(DocumentId, i32), RetrievedChunk> = HashMap::new();

    for (rank, chunk) in dense.into_iter().enumerate() {
        let key = (chunk.document_id, chunk.para_index);
        *rrf_scores.entry(key).or_insert(0.0) += 1.0 / (RRF_K + rank as f32 + 1.0);
        chunks_by_key.entry(key).or_insert(chunk);
    }
    for (rank, chunk) in sparse.into_iter().enumerate() {
        let key = (chunk.document_id, chunk.para_index);
        *rrf_scores.entry(key).or_insert(0.0) += 1.0 / (RRF_K + rank as f32 + 1.0);
        chunks_by_key.entry(key).or_insert(chunk);
    }

    let mut fused: Vec<RetrievedChunk> = chunks_by_key
        .into_iter()
        .map(|(key, mut chunk)| {
            chunk.score = rrf_scores[&key];
            chunk
        })
        .collect();

    fused.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    fused.truncate(TOP_K as usize);
    fused
}

fn payload_str(payload: &HashMap<String, QdrantValue>, key: &str) -> String {
    payload
        .get(key)
        .and_then(|v| v.kind.as_ref())
        .and_then(|k| match k {
            QdrantValueKind::StringValue(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn payload_int(payload: &HashMap<String, QdrantValue>, key: &str) -> i32 {
    payload
        .get(key)
        .and_then(|v| v.kind.as_ref())
        .and_then(|k| match k {
            QdrantValueKind::IntegerValue(i) => Some(*i as i32),
            _ => None,
        })
        .unwrap_or_default()
}

fn scored_point_to_chunk(point: ScoredPoint) -> Option<RetrievedChunk> {
    let payload = &point.payload;
    let document_id = payload_str(payload, "document_id").parse::<uuid::Uuid>().ok()?;

    Some(RetrievedChunk {
        document_id: DocumentId::from(document_id),
        chapter: payload_str(payload, "chapter"),
        topic: payload_str(payload, "topic"),
        page: payload_int(payload, "page"),
        para_index: payload_int(payload, "para_index"),
        text: payload_str(payload, "text"),
        lang: payload_str(payload, "lang"),
        score: point.score,
    })
}

#[cfg(test)]
mod filter_tests {
    use super::*;
    use uuid::Uuid;

    fn query(document_id: Option<DocumentId>) -> RetrievalQuery {
        RetrievalQuery {
            question: "What is sandhi?".to_string(),
            program_id: ProgramId::from_uuid(Uuid::nil()),
            semester_no: 1,
            course_id: CourseId::from_uuid(Uuid::nil()),
            block_no: 3,
            document_id,
        }
    }

    /// E-25: the four mandatory dimensions are present on every query this
    /// module can build. Asserted on the condition count because a filter that
    /// silently lost one would still be a valid `Filter` — cross-course leakage
    /// is a correctness bug, not a relevance one (`rag-pipeline.md`).
    #[test]
    fn the_four_mandatory_filters_are_always_present() {
        assert_eq!(mandatory_filter(&query(None)).must.len(), 4);
    }

    /// Choosing a unit narrows within the block; it never replaces any of the
    /// four. The count going to five — not staying at four — is the property:
    /// a document filter that displaced `block_no` would search one document
    /// without checking it is in this block.
    #[test]
    fn a_chosen_unit_adds_a_fifth_condition_rather_than_replacing_one() {
        let narrowed = mandatory_filter(&query(Some(DocumentId::from_uuid(Uuid::new_v4()))));
        assert_eq!(narrowed.must.len(), 5);
    }
}

#[cfg(test)]
mod abstain_tests {
    use super::*;

    fn chunk(text: &str, para_index: i32) -> RetrievedChunk {
        RetrievedChunk {
            document_id: DocumentId::from(uuid::Uuid::nil()),
            chapter: "Unit 3 Forest Resources".to_string(),
            topic: "Deforestation".to_string(),
            page: 5,
            para_index,
            text: text.to_string(),
            lang: "en".to_string(),
            score: 0.0,
        }
    }

    /// A question in one language, against a corpus in another, must NOT
    /// abstain when retrieval actually found the passage.
    ///
    /// Regression test for a live failure. `StubReranker` replaces `score`
    /// with Jaccard word overlap between question and chunk; abstention was
    /// read off that. A Malayalam question over an English corpus has zero
    /// overlap BY CONSTRUCTION, so every question abstained and the tutor told
    /// the student, topic after topic, that their syllabus did not cover it —
    /// while the passage sat in the corpus. The floor belongs on the retrieval
    /// score, which is language-agnostic because the embedding is.
    #[test]
    fn a_cross_language_question_does_not_abstain_when_retrieval_succeeded() {
        // What Qdrant actually returns for an on-topic Malayalam question,
        // measured against the real corpus: a dense cosine of ~0.62.
        let mut hit = chunk("deforestation is the destruction of forests", 1);
        hit.score = 0.6182;
        let dense = vec![hit];
        let retrieval_score = dense[0].score;

        let fused = rrf_fuse(dense, vec![]);
        let reranked = StubReranker.rerank("വനനാശം എന്താണ്", fused);
        assert_eq!(
            reranked[0].score, 0.0,
            "precondition: lexical overlap across scripts is zero"
        );

        assert!(
            !crate::abstain::should_abstain(retrieval_score),
            "retrieval found the passage; abstaining would deny a covered topic"
        );
    }

    /// The floor still does its job in the other direction: an off-topic
    /// question, measured at ~0.56 against this corpus, must still abstain.
    #[test]
    fn an_off_topic_question_still_abstains() {
        assert!(crate::abstain::should_abstain(0.5625));
        assert!(rrf_fuse(vec![], vec![]).is_empty());
    }
}
