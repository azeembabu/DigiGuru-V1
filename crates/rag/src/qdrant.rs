//! Thin wrapper around `qdrant-client` for the `curriculum` collection
//! (`IMPLEMENTATION_PLAN.md` §3.2). Payload indexes on `program_id`,
//! `semester_no`, `course_id`, and `block_no` are mandatory (E-25) — they
//! are what makes "strictly this student's syllabus" enforceable at query
//! time (`.claude/rules/rag-pipeline.md`).

use qdrant_client::qdrant::{
    vectors_config::Config as VectorsConfigOneOf, CreateCollection, Distance, FieldType,
    PointStruct, SparseVectorParams, SparseVectorsConfig, VectorParams, VectorsConfig,
};
use qdrant_client::Qdrant;

use crate::enrich::EnrichedChunk;
use crate::error::{RagError, Result};

pub const COLLECTION_NAME: &str = "curriculum";
const DENSE_VECTOR_NAME: &str = "dense";
const SPARSE_VECTOR_NAME: &str = "bm25";

/// Creates the `curriculum` collection with the exact schema from
/// `IMPLEMENTATION_PLAN.md` §3.2 if it doesn't already exist. Idempotent:
/// checks existence first rather than relying on the server to no-op on a
/// duplicate create.
pub async fn ensure_collection(client: &Qdrant) -> Result<()> {
    let exists = client
        .collection_exists(COLLECTION_NAME)
        .await
        .map_err(|e| RagError::Qdrant(e.to_string()))?;

    if exists {
        return Ok(());
    }

    let dense_config = VectorsConfig {
        config: Some(VectorsConfigOneOf::Params(VectorParams {
            size: crate::embed::EMBEDDING_DIM as u64,
            distance: Distance::Cosine.into(),
            ..Default::default()
        })),
    };

    let mut sparse_map = std::collections::HashMap::new();
    sparse_map.insert(SPARSE_VECTOR_NAME.to_string(), SparseVectorParams::default());

    client
        .create_collection(CreateCollection {
            collection_name: COLLECTION_NAME.to_string(),
            vectors_config: Some(dense_config),
            sparse_vectors_config: Some(SparseVectorsConfig { map: sparse_map }),
            ..Default::default()
        })
        .await
        .map_err(|e| RagError::Qdrant(e.to_string()))?;

    // Mandatory payload indexes (E-25): program_id, semester_no, course_id,
    // block_no. No exceptions per `.claude/rules/rag-pipeline.md`.
    for (field, field_type) in [
        ("program_id", FieldType::Keyword),
        ("semester_no", FieldType::Integer),
        ("course_id", FieldType::Keyword),
        ("block_no", FieldType::Integer),
    ] {
        client
            .create_field_index(COLLECTION_NAME, field, field_type, None, None)
            .await
            .map_err(|e| RagError::Qdrant(e.to_string()))?;
    }

    Ok(())
}

/// Upserts a batch of enriched chunks with their dense embeddings. Assumes
/// `chunks.len() == embeddings.len()`, positionally paired — the caller
/// (`ingest.rs`) is the only producer of both and keeps them in lockstep.
pub async fn upsert_chunks(
    client: &Qdrant,
    chunks: &[EnrichedChunk],
    embeddings: &[Vec<f32>],
) -> Result<()> {
    if chunks.len() != embeddings.len() {
        return Err(RagError::Qdrant(format!(
            "chunk/embedding count mismatch: {} chunks, {} embeddings",
            chunks.len(),
            embeddings.len()
        )));
    }

    let points: Vec<PointStruct> = chunks
        .iter()
        .zip(embeddings.iter())
        .map(|(chunk, embedding)| {
            let id = uuid::Uuid::new_v4().to_string();
            let mut payload = std::collections::HashMap::new();
            payload.insert(
                "program_id".to_string(),
                chunk.program_id.to_string().into(),
            );
            payload.insert("semester_no".to_string(), (chunk.semester_no as i64).into());
            payload.insert("course_id".to_string(), chunk.course_id.to_string().into());
            payload.insert("block_no".to_string(), (chunk.block_no as i64).into());
            payload.insert(
                "document_id".to_string(),
                chunk.document_id.to_string().into(),
            );
            payload.insert(
                "chapter".to_string(),
                chunk.chapter.clone().unwrap_or_default().into(),
            );
            payload.insert(
                "topic".to_string(),
                chunk.topic.clone().unwrap_or_default().into(),
            );
            payload.insert("page".to_string(), (chunk.page as i64).into());
            payload.insert("para_index".to_string(), (chunk.para_index as i64).into());
            payload.insert("text".to_string(), chunk.text.clone().into());
            payload.insert("lang".to_string(), chunk.lang.clone().into());

            PointStruct::new(id, [(DENSE_VECTOR_NAME.to_string(), embedding.clone())], payload)
        })
        .collect();

    client
        .upsert_points(qdrant_client::qdrant::UpsertPointsBuilder::new(
            COLLECTION_NAME,
            points,
        ))
        .await
        .map_err(|e| RagError::Qdrant(e.to_string()))?;

    Ok(())
}
