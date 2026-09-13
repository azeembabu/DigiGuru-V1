//! Thin wrapper around `qdrant-client` for the `curriculum` collection
//! (`IMPLEMENTATION_PLAN.md` §3.2). Payload indexes on `program_id`,
//! `semester_no`, `course_id`, and `block_no` are mandatory (E-25) — they
//! are what makes "strictly this student's syllabus" enforceable at query
//! time (`.claude/rules/rag-pipeline.md`).

use qdrant_client::qdrant::{
    vectors_config::Config as VectorsConfigOneOf, CreateCollectionBuilder,
    CreateFieldIndexCollectionBuilder, DeletePointsBuilder, Distance, FieldType, Filter,
    NamedVectors, PointStruct, SparseVectorConfig, SparseVectorParams, VectorParams, VectorsConfig,
};
use qdrant_client::Qdrant;

use crate::enrich::EnrichedChunk;
use crate::error::Result;
use crate::payload::build_payload;

pub const COLLECTION_NAME: &str = "curriculum";
pub const DENSE_VECTOR_NAME: &str = "dense";
pub const SPARSE_VECTOR_NAME: &str = "bm25";

/// Creates the `curriculum` collection with the schema from
/// `IMPLEMENTATION_PLAN.md` §3.2 if it doesn't already exist. Idempotent:
/// checks existence first rather than relying on the server to no-op on a
/// duplicate create.
pub async fn ensure_collection(client: &Qdrant) -> Result<()> {
    if client.collection_exists(COLLECTION_NAME).await? {
        return Ok(());
    }

    // Dense vectors are *named* (`dense`) rather than the unnamed default,
    // because the collection also carries a named sparse vector (`bm25`) —
    // Qdrant requires every vector to be named once more than one exists,
    // and retrieval searches each leg by name.
    let mut dense_map = std::collections::HashMap::new();
    dense_map.insert(
        DENSE_VECTOR_NAME.to_string(),
        VectorParams {
            size: crate::embed::EMBEDDING_DIM as u64,
            distance: Distance::Cosine.into(),
            ..Default::default()
        },
    );
    let dense_config = VectorsConfig {
        config: Some(VectorsConfigOneOf::ParamsMap(
            qdrant_client::qdrant::VectorParamsMap { map: dense_map },
        )),
    };

    let mut sparse_map = std::collections::HashMap::new();
    sparse_map.insert(
        SPARSE_VECTOR_NAME.to_string(),
        SparseVectorParams::default(),
    );

    client
        .create_collection(
            CreateCollectionBuilder::new(COLLECTION_NAME)
                .vectors_config(dense_config)
                .sparse_vectors_config(SparseVectorConfig { map: sparse_map }),
        )
        .await?;

    // Mandatory payload indexes (E-25): program_id, semester_no, course_id,
    // block_no. No exceptions per `.claude/rules/rag-pipeline.md` — without
    // these the mandatory retrieval filter is a full scan rather than an
    // index lookup, and at corpus scale that is the difference between a
    // filter being applied and a query timing out.
    for (field, field_type) in [
        ("program_id", FieldType::Keyword),
        ("semester_no", FieldType::Integer),
        ("course_id", FieldType::Keyword),
        ("block_no", FieldType::Integer),
        ("document_id", FieldType::Keyword),
    ] {
        client
            .create_field_index(CreateFieldIndexCollectionBuilder::new(
                COLLECTION_NAME,
                field,
                field_type,
            ))
            .await?;
    }

    Ok(())
}

/// Removes every point belonging to `document_id`.
///
/// Called before upserting a document's chunks so a **re-ingest** (a retry,
/// or an admin re-running a document after a failed attempt) replaces the
/// document's chunks instead of adding a second copy alongside them. Chunk
/// point ids are random uuids, so an upsert alone would never overwrite the
/// previous run's points — without this, a retried document doubles the
/// corpus exactly the way `sha256` dedupe exists to prevent.
pub async fn delete_document_points(
    client: &Qdrant,
    document_id: dg_core::DocumentId,
) -> Result<()> {
    let filter = Filter::must([qdrant_client::qdrant::Condition::matches(
        "document_id",
        document_id.to_string(),
    )]);

    client
        .delete_points(
            DeletePointsBuilder::new(COLLECTION_NAME)
                .points(filter)
                .wait(true),
        )
        .await?;

    Ok(())
}

/// Upserts a batch of enriched chunks with their dense embeddings, paired
/// positionally with `chunks`.
///
/// Every point's payload is built by [`crate::payload::build_payload`],
/// which rejects a chunk missing any mandatory field — so an untagged chunk
/// fails the run rather than being indexed. The whole batch is validated
/// *before* any of it is sent, so a bad chunk cannot leave the document
/// half-indexed.
pub async fn upsert_chunks(
    client: &Qdrant,
    chunks: &[EnrichedChunk],
    embeddings: &[Vec<f32>],
) -> Result<()> {
    if chunks.len() != embeddings.len() {
        return Err(crate::error::RagError::Qdrant(format!(
            "chunk/embedding count mismatch: {} chunks, {} embeddings",
            chunks.len(),
            embeddings.len()
        )));
    }

    let mut points: Vec<PointStruct> = Vec::with_capacity(chunks.len());
    for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
        if embedding.len() != crate::embed::EMBEDDING_DIM {
            return Err(crate::error::RagError::Embed(format!(
                "embedding for page {} para {} is {} dims, expected {}",
                chunk.page,
                chunk.para_index,
                embedding.len(),
                crate::embed::EMBEDDING_DIM
            )));
        }

        // Rejects the chunk if any mandatory payload field is missing.
        let payload = build_payload(chunk)?;

        // Shared encoding with `retrieve::search_sparse` (`crate::sparse`)
        // — the two sides MUST use the same scheme or sparse search
        // silently returns nothing meaningful.
        let sparse = crate::sparse::sparse_encode(&chunk.text);
        let vectors = NamedVectors::default()
            .add_vector(DENSE_VECTOR_NAME, embedding.clone())
            .add_vector(SPARSE_VECTOR_NAME, sparse);

        points.push(PointStruct::new(
            uuid::Uuid::new_v4().to_string(),
            vectors,
            payload,
        ));
    }

    if points.is_empty() {
        return Ok(());
    }

    client
        .upsert_points(
            qdrant_client::qdrant::UpsertPointsBuilder::new(COLLECTION_NAME, points).wait(true),
        )
        .await?;

    Ok(())
}
