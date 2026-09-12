//! `rag` — ingest, chunk, embed, retrieve, rerank. The curriculum-bounded
//! retrieval pipeline: PDF ingestion through Qdrant hybrid search, cross-
//! encoder reranking, and the similarity-floor abstention check (NN-4).
//!
//! `retrieve`, `rerank`, and `abstain` (the retrieval half of the pipeline)
//! are implemented here. `ingest`, `parse`, `clean`, `chunk`, `enrich`,
//! `embed`, `qdrant`, and `error` (the ingestion half, plus the shared
//! `Embedder`/`RagError` types `retrieve.rs` depends on) are owned by a
//! parallel agent and declared alongside these once they land — see the
//! interface-assumption note at the top of `retrieve.rs`.

pub mod abstain;
pub mod rerank;
pub mod retrieve;
