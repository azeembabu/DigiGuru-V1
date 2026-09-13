//! The Qdrant payload every chunk carries, and the mandatory-field gate in
//! front of it.
//!
//! `.claude/rules/rag-pipeline.md`, ingestion rule 5:
//!
//! > Every chunk is tagged with `program_id`, `semester`, `block_no`,
//! > `document_id`, `chapter`, `topic`, `page`, `para_index`, `lang`.
//! > **A chunk missing any of these is a bug — reject it at upsert.**
//!
//! "Reject" is load-bearing: the failure mode this prevents is a chunk with
//! a blank `chapter` being indexed anyway and later cited to a student as an
//! empty citation, or a chunk with a missing `program_id` becoming
//! retrievable from *every* program (the filter in `retrieve.rs` matches on
//! the payload — a chunk lacking the field is simply not excluded by a
//! `must` clause it has no value for). Both are silent correctness bugs, so
//! the check is a hard error on the ingest path, not a warning.
//!
//! [`validate_chunk`] runs on every chunk in [`build_payload`], which is the
//! only way `qdrant::upsert_chunks` constructs a point — there is no code
//! path that reaches Qdrant bypassing it.

use std::collections::HashMap;

use qdrant_client::qdrant::Value as QdrantValue;

use crate::enrich::EnrichedChunk;
use crate::error::{RagError, Result};

/// The nine mandatory payload keys, in the order `rag-pipeline.md` lists
/// them. Used both to validate and to assert the built payload's shape.
pub const MANDATORY_FIELDS: [&str; 9] = [
    "program_id",
    "semester_no",
    "block_no",
    "document_id",
    "chapter",
    "topic",
    "page",
    "para_index",
    "lang",
];

/// Rejects a chunk that is missing any mandatory payload value.
///
/// The id fields (`program_id`, `course_id`, `document_id`) are newtypes over
/// `Uuid` and cannot be absent — but they *can* be nil (`00000000-…`), which
/// means "never populated" just as surely as a missing key, so a nil uuid is
/// treated as missing. Text fields are missing when blank or whitespace-only.
/// Numeric fields are missing when non-positive, since `semester_no`,
/// `block_no` and `page` are all 1-indexed in the schema.
pub fn validate_chunk(chunk: &EnrichedChunk) -> Result<()> {
    let mut missing: Vec<&str> = Vec::new();

    if chunk.program_id.into_uuid().is_nil() {
        missing.push("program_id");
    }
    if chunk.course_id.into_uuid().is_nil() {
        missing.push("course_id");
    }
    if chunk.document_id.into_uuid().is_nil() {
        missing.push("document_id");
    }
    if chunk.semester_no <= 0 {
        missing.push("semester_no");
    }
    if chunk.block_no <= 0 {
        missing.push("block_no");
    }
    if chunk.page <= 0 {
        missing.push("page");
    }
    if chunk.chapter.trim().is_empty() {
        missing.push("chapter");
    }
    if chunk.topic.trim().is_empty() {
        missing.push("topic");
    }
    if chunk.lang.trim().is_empty() {
        missing.push("lang");
    }
    // Not in the mandatory list, but an empty chunk body is never useful and
    // would embed to a meaningless vector.
    if chunk.text.trim().is_empty() {
        missing.push("text");
    }

    if missing.is_empty() {
        return Ok(());
    }

    Err(RagError::MissingMetadata {
        page: chunk.page,
        para_index: chunk.para_index,
        fields: missing.join(", "),
    })
}

/// Builds the validated Qdrant payload for one chunk.
///
/// Returns `Err` rather than a partially-populated payload — the caller
/// (`qdrant::upsert_chunks`) propagates, failing the whole ingestion run, so
/// a document never lands half-indexed with some chunks silently dropped.
pub fn build_payload(chunk: &EnrichedChunk) -> Result<HashMap<String, QdrantValue>> {
    validate_chunk(chunk)?;

    let mut payload: HashMap<String, QdrantValue> = HashMap::new();
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
    payload.insert("chapter".to_string(), chunk.chapter.clone().into());
    payload.insert("topic".to_string(), chunk.topic.clone().into());
    payload.insert("page".to_string(), (chunk.page as i64).into());
    payload.insert("para_index".to_string(), (chunk.para_index as i64).into());
    payload.insert("lang".to_string(), chunk.lang.clone().into());
    payload.insert("text".to_string(), chunk.text.clone().into());
    payload.insert("kind".to_string(), kind_label(chunk.kind).into());

    Ok(payload)
}

/// `kind: table|verse` (`rag-pipeline.md` ingestion rule 4); prose chunks
/// carry `prose` so the field is always present and filterable.
pub fn kind_label(kind: crate::chunk::ChunkKind) -> &'static str {
    match kind {
        crate::chunk::ChunkKind::Prose => "prose",
        crate::chunk::ChunkKind::Table => "table",
        crate::chunk::ChunkKind::Verse => "verse",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkKind;
    use dg_core::{CourseId, DocumentId, ProgramId};
    use uuid::Uuid;

    fn valid_chunk() -> EnrichedChunk {
        EnrichedChunk {
            text: "Real chunk body text.".to_string(),
            token_count: 4,
            kind: ChunkKind::Prose,
            program_id: ProgramId::from_uuid(Uuid::new_v4()),
            semester_no: 1,
            course_id: CourseId::from_uuid(Uuid::new_v4()),
            block_no: 3,
            document_id: DocumentId::from_uuid(Uuid::new_v4()),
            chapter: "Chapter 3".to_string(),
            topic: "Poetry".to_string(),
            page: 57,
            para_index: 12,
            lang: "ml".to_string(),
        }
    }

    #[test]
    fn accepts_a_fully_tagged_chunk() {
        let chunk = valid_chunk();
        let payload = build_payload(&chunk).expect("fully tagged chunk must be accepted");
        for field in MANDATORY_FIELDS {
            assert!(payload.contains_key(field), "payload missing {field}");
        }
        assert!(payload.contains_key("course_id"));
    }

    #[test]
    fn rejects_a_chunk_with_a_blank_chapter() {
        let mut chunk = valid_chunk();
        chunk.chapter = "   ".to_string();

        let err = build_payload(&chunk).expect_err("blank chapter must be rejected at upsert");
        match err {
            RagError::MissingMetadata { fields, .. } => assert!(fields.contains("chapter")),
            other => panic!("expected MissingMetadata, got {other:?}"),
        }
    }

    #[test]
    fn rejects_a_chunk_with_a_nil_program_id() {
        // A nil program_id would make the chunk invisible to the mandatory
        // program filter and therefore effectively cross-program.
        let mut chunk = valid_chunk();
        chunk.program_id = ProgramId::from_uuid(Uuid::nil());

        let err = build_payload(&chunk).expect_err("nil program_id must be rejected");
        match err {
            RagError::MissingMetadata { fields, .. } => assert!(fields.contains("program_id")),
            other => panic!("expected MissingMetadata, got {other:?}"),
        }
    }

    #[test]
    fn rejects_a_chunk_missing_several_fields_and_names_all_of_them() {
        let mut chunk = valid_chunk();
        chunk.topic = String::new();
        chunk.lang = String::new();
        chunk.page = 0;

        let err = build_payload(&chunk).expect_err("must be rejected");
        match err {
            RagError::MissingMetadata { fields, .. } => {
                assert!(fields.contains("topic"), "{fields}");
                assert!(fields.contains("lang"), "{fields}");
                assert!(fields.contains("page"), "{fields}");
            }
            other => panic!("expected MissingMetadata, got {other:?}"),
        }
    }

    /// Blanks one mandatory field so the validator can be asserted on it.
    type FieldMutator = fn(&mut EnrichedChunk);

    #[test]
    fn every_mandatory_field_is_independently_enforced() {
        // Guards against the check silently losing a field: mutate each one
        // to its "missing" form in turn and assert each is caught by name.
        let mutators: Vec<(&str, FieldMutator)> = vec![
            ("program_id", |c| {
                c.program_id = ProgramId::from_uuid(Uuid::nil())
            }),
            ("course_id", |c| {
                c.course_id = CourseId::from_uuid(Uuid::nil())
            }),
            ("document_id", |c| {
                c.document_id = DocumentId::from_uuid(Uuid::nil())
            }),
            ("semester_no", |c| c.semester_no = 0),
            ("block_no", |c| c.block_no = 0),
            ("page", |c| c.page = 0),
            ("chapter", |c| c.chapter = String::new()),
            ("topic", |c| c.topic = String::new()),
            ("lang", |c| c.lang = String::new()),
        ];

        for (field, mutate) in mutators {
            let mut chunk = valid_chunk();
            mutate(&mut chunk);
            match build_payload(&chunk) {
                Err(RagError::MissingMetadata { fields, .. }) => assert!(
                    fields.contains(field),
                    "rejection did not name {field}; named: {fields}"
                ),
                Err(other) => panic!("expected MissingMetadata for {field}, got {other:?}"),
                Ok(_) => panic!("{field} was not enforced — chunk accepted with it missing"),
            }
        }
    }
}
