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

use qdrant_client::client::QdrantClient;
use qdrant_client::qdrant::{value::Kind as QdrantValueKind, Condition, Filter, ScoredPoint, SearchPoints, SparseVector, Value as QdrantValue};

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
    client: &QdrantClient,
    query: &RetrievalQuery,
    embedder: &dyn Embedder,
) -> Result<RetrievalOutcome, RagError> {
    let (normalised_question, lang) = normalise_and_detect_language(&query.question);

    let filter = mandatory_filter(query);

    let dense_vector = embedder.embed(&normalised_question).await?;

    let dense_results = search_dense(client, &filter, dense_vector).await?;
    let sparse_results = search_sparse(client, &filter, &normalised_question, lang).await?;

    let fused = rrf_fuse(dense_results, sparse_results);
    if fused.is_empty() {
        return Ok(RetrievalOutcome::Abstain);
    }

    let reranked = StubReranker.rerank(&normalised_question, fused);

    let top_score = reranked.first().map(|c| c.score).unwrap_or(0.0);
    if should_abstain(top_score) {
        return Ok(RetrievalOutcome::Abstain);
    }

    Ok(RetrievalOutcome::Chunks(reranked))
}

/// The mandatory `program_id AND semester_no AND course_id AND block_no`
/// filter (E-25). Both search legs in [`retrieve`] go through this — there
/// is no code path here that builds a Qdrant query without it.
fn mandatory_filter(query: &RetrievalQuery) -> Filter {
    Filter::must([
        Condition::matches("program_id", query.program_id.to_string()),
        Condition::matches("semester_no", query.semester_no as i64),
        Condition::matches("course_id", query.course_id.to_string()),
        Condition::matches("block_no", query.block_no as i64),
    ])
}

async fn search_dense(
    client: &QdrantClient,
    filter: &Filter,
    dense_vector: Vec<f32>,
) -> Result<Vec<RetrievedChunk>, RagError> {
    let request = SearchPoints {
        collection_name: COLLECTION.to_string(),
        vector: dense_vector,
        vector_name: Some("dense".to_string()),
        filter: Some(filter.clone()),
        limit: TOP_K,
        with_payload: Some(true.into()),
        ..Default::default()
    };

    let response = client.search_points(&request).await?;
    Ok(response
        .result
        .into_iter()
        .filter_map(scored_point_to_chunk)
        .collect())
}

/// Naive placeholder BM25-ish sparse encoding: hashed-token term frequency.
///
/// TODO(phase2-bm25): this must match whatever sparse encoding the
/// ingestion side actually upserts into the `bm25` sparse vector at
/// ingest time (owned by the parallel ingestion agent) — if the encodings
/// diverge, sparse search silently returns garbage rather than failing
/// loudly. Flagged as the top cross-agent risk in the task report.
fn sparse_encode(text: &str) -> SparseVector {
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

    let mut indices: Vec<u32> = Vec::with_capacity(term_counts.len());
    let mut values: Vec<f32> = Vec::with_capacity(term_counts.len());
    for (index, value) in term_counts {
        indices.push(index);
        values.push(value);
    }

    SparseVector { indices, values }
}

async fn search_sparse(
    client: &QdrantClient,
    filter: &Filter,
    question: &str,
    _lang: &str,
) -> Result<Vec<RetrievedChunk>, RagError> {
    let sparse = sparse_encode(question);

    let request = SearchPoints {
        collection_name: COLLECTION.to_string(),
        sparse_vector: Some(sparse),
        vector_name: Some("bm25".to_string()),
        filter: Some(filter.clone()),
        limit: TOP_K,
        with_payload: Some(true.into()),
        ..Default::default()
    };

    let response = client.search_points(&request).await?;
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
