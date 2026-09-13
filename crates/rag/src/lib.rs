//! `rag` — ingest, chunk, embed, retrieve, rerank. The curriculum-bounded
//! retrieval pipeline: PDF ingestion through Qdrant hybrid search, cross-
//! encoder reranking, and the similarity-floor abstention check (NN-4).

pub mod abstain;
pub mod chunk;
pub mod clean;
pub mod embed;
pub mod enrich;
pub mod error;
pub mod ingest;
pub mod parse;
pub mod payload;
pub mod preamble;
pub mod qdrant;
pub mod rerank;
pub mod retrieve;
pub mod sparse;

pub use error::{RagError, Result};
