//! PDF ingestion worker binary. Not yet implemented — Phase 2.
//!
//! See `IMPLEMENTATION_PLAN.md` §5 for the ingestion workflow this will run:
//! layout parse -> clean -> paragraph chunk -> enrich -> embed -> upsert to
//! Qdrant, consuming jobs from a Postgres outbox.

#[tokio::main]
async fn main() {
    println!("ingest-worker: not yet implemented — Phase 2. See IMPLEMENTATION_PLAN.md §5.");
}
