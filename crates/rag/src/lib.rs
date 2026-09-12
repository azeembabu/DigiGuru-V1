//! `rag` — ingest, chunk, embed, retrieve, rerank. The curriculum-bounded
//! retrieval pipeline: PDF ingestion through Qdrant hybrid search, cross-
//! encoder reranking, and the similarity-floor abstention check (NN-4).
//!
//! This module owns **ingestion** (`parse`, `clean`, `chunk`, `enrich`,
//! `embed`, `qdrant`, `ingest`) per `IMPLEMENTATION_PLAN.md` §5.1.
//! Retrieval (`retrieve`, `rerank`, `abstain`) is owned and added
//! separately — do not add those modules here.

pub mod chunk;
pub mod clean;
pub mod embed;
pub mod enrich;
pub mod error;
pub mod ingest;
pub mod parse;
pub mod qdrant;

pub use error::{RagError, Result};
