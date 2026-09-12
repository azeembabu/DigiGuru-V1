-- Phase 2 — ingestion outbox table (IMPLEMENTATION_PLAN.md §5.1: "job
-- enqueued (Postgres outbox -> worker)"). The gateway upload endpoint
-- inserts a row here after virus-scan + sha256 dedupe; the ingestion worker
-- polls `status = 'pending'` and drives it through parse/clean/chunk/enrich/
-- embed/upsert, updating `status` (and `last_error` on failure) as it goes.

CREATE TYPE ingestion_job_status AS ENUM ('pending', 'processing', 'completed', 'failed');

CREATE TABLE ingestion_jobs (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  status      ingestion_job_status NOT NULL DEFAULT 'pending',
  attempts    INT NOT NULL DEFAULT 0,
  last_error  TEXT,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- The worker's polling query filters on `status` (e.g. `WHERE status =
-- 'pending' ORDER BY created_at`), so this is the hot-path index.
CREATE INDEX idx_ingestion_jobs_status ON ingestion_jobs(status);
CREATE INDEX idx_ingestion_jobs_document_id ON ingestion_jobs(document_id);
