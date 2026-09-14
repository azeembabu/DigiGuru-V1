-- 0011_document_thumbnails.sql
-- A cover image for each uploaded unit.
--
-- The student's unit list (`GET /student/blocks/{id}/units`) was a stack of
-- title-and-page-count rows, which all look alike; a student picking "Unit 4"
-- out of six had nothing to recognise it by. The first page of the PDF is the
-- thing they actually remember, so the ingest worker rasterises it once and
-- records where it put it.
--
-- Nullable, and deliberately so:
--
--   * every document uploaded before this migration has no thumbnail, and
--     backfilling would mean re-reading every stored PDF inside a migration;
--   * rasterising needs a native pdfium library that this build does not
--     vendor (`crates/rag/src/thumbnail.rs`), so a deployment without it
--     ingests exactly as before and simply leaves this NULL.
--
-- NULL therefore means "no cover available", never "not ingested" —
-- `documents.status` remains the only authority on ingestion state, and the
-- client renders a generated tile in place of a missing cover.
--
-- Like `storage_key`, this is a server-side path and never goes on the wire;
-- the bytes are served through a scoped route that re-checks enrolment.

ALTER TABLE documents ADD COLUMN thumbnail_key TEXT;

COMMENT ON COLUMN documents.thumbnail_key IS
  'Server-side path to the rasterised first page (WebP). NULL = no cover available; never serialised to a client.';
