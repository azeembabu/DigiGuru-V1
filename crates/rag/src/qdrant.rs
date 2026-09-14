//! Thin wrapper around `qdrant-client` for the `curriculum` collection
//! (`IMPLEMENTATION_PLAN.md` §3.2). Payload indexes on `program_id`,
//! `semester_no`, `course_id`, and `block_no` are mandatory (E-25) — they
//! are what makes "strictly this student's syllabus" enforceable at query
//! time (`.claude/rules/rag-pipeline.md`).

use qdrant_client::qdrant::{CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, DeletePointsBuilder, Distance, FieldType, Filter, NamedVectors, PointStruct, ScrollPointsBuilder, SparseVectorConfig, SparseVectorParams, VectorParams, VectorsConfig, vectors_config::Config as VectorsConfigOneOf};
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


/// Every distinct `chapter` currently indexed for a block, in reading order
/// (`block_no` ascending by `page`, then `para_index`).
///
/// Exists because the block outline is a property of the BLOCK, not of one
/// document. Building it from a single ingest run's chunks — which is what
/// used to happen — wrote a `pcache:{block_id}` entry containing only the
/// document that happened to be ingested last, and the tutor was then handed
/// that as the block's whole syllabus. Observed live: Block 1 has six units,
/// its outline read "1. Unit 4 Water Resources", and the tutor kept leaving
/// the unit the student had opened to teach Unit 4 instead.
pub async fn chapters_for_block(
    client: &Qdrant,
    program_id: dg_core::ProgramId,
    semester_no: i32,
    course_id: dg_core::CourseId,
    block_no: i32,
) -> Result<Vec<String>> {
    // The same four mandatory dimensions retrieval filters on (E-25), so the
    // outline describes exactly the material this block can actually serve.
    let filter = Filter::must([
        qdrant_client::qdrant::Condition::matches("program_id", program_id.to_string()),
        qdrant_client::qdrant::Condition::matches("semester_no", semester_no as i64),
        qdrant_client::qdrant::Condition::matches("course_id", course_id.to_string()),
        qdrant_client::qdrant::Condition::matches("block_no", block_no as i64),
    ]);

    let mut chapters: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut offset = None;

    // Scrolled rather than searched: this wants every chunk in the block, and
    // a vector query would cap at `limit` and silently truncate the outline.
    loop {
        let mut builder = ScrollPointsBuilder::new(COLLECTION_NAME)
            .filter(filter.clone())
            .limit(256)
            .with_payload(true)
            .with_vectors(false);
        if let Some(offset) = offset.take() {
            builder = builder.offset(offset);
        }

        let page = client.scroll(builder).await?;
        for point in &page.result {
            if let Some(value) = point.payload.get("chapter") {
                if let Some(text) = value.as_str() {
                    if !text.trim().is_empty() && seen.insert(text.to_string()) {
                        chapters.push(text.to_string());
                    }
                }
            }
        }

        match page.next_page_offset {
            Some(next) => offset = Some(next),
            None => break,
        }
    }

    chapters.sort();
    Ok(chapters)
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

/// Normalises a configured Qdrant URL to the **gRPC** endpoint.
///
/// Qdrant listens on two ports and they are not interchangeable: `6333` speaks
/// REST, `6334` speaks gRPC. `qdrant_client::Qdrant` is a gRPC client, so
/// pointing it at `6333` does not fail with "wrong port" — it fails with
///
/// ```text
/// qdrant operation failed: Error in the response: Unknown error
/// h2 protocol error: http2 error
/// ```
///
/// which reads like a network fault and sends you looking in the wrong place.
/// Observed exactly that: every retrieval in a live session failed this way, and
/// because the tutor treats a retrieval failure as an abstention (the correct,
/// safe direction for NN-4), the only visible symptom was a tutor that refused
/// to teach anything.
///
/// `QDRANT_URL` is documented and defaulted as the REST URL across this
/// workspace, so rather than change that contract in every deployment, the gRPC
/// port is derived here. An explicit `6334` (or any other port) is left alone —
/// only the known-REST default is remapped.
pub fn grpc_url(configured: &str) -> String {
    match configured.rsplit_once(":6333") {
        // Only remap when `:6333` ends the URL or is followed by a path, so a
        // host that merely contains "6333" is untouched.
        Some((head, tail)) if tail.is_empty() || tail.starts_with('/') => {
            format!("{head}:6334{tail}")
        }
        _ => configured.to_string(),
    }
}

#[cfg(test)]
mod grpc_url_tests {
    use super::grpc_url;

    #[test]
    fn the_rest_port_is_remapped_to_the_grpc_port() {
        assert_eq!(grpc_url("http://localhost:6333"), "http://localhost:6334");
        assert_eq!(grpc_url("http://qdrant:6333/"), "http://qdrant:6334/");
    }

    #[test]
    fn an_explicit_grpc_port_is_left_alone() {
        assert_eq!(grpc_url("http://localhost:6334"), "http://localhost:6334");
    }

    #[test]
    fn an_unrelated_port_is_left_alone() {
        // A managed Qdrant behind a gateway may expose gRPC on 443.
        assert_eq!(grpc_url("https://xyz.cloud.qdrant.io:443"), "https://xyz.cloud.qdrant.io:443");
    }

    #[test]
    fn a_host_that_merely_contains_the_digits_is_not_mangled() {
        assert_eq!(grpc_url("http://host6333.internal:6334"), "http://host6333.internal:6334");
    }
}
