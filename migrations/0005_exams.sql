-- 0005_exams.sql
-- Exams and attempts — the record behind the student dashboard's "recent
-- exams" score cards and behind a student's progress over a block.
--
-- Forward-only: two new types and two new tables, nothing existing altered,
-- so every shipped query keeps working unchanged.
--
-- An exam hangs off a `block`, the teachable unit, exactly as `documents`
-- does. Program, semester and course are reachable through
-- `exams -> blocks -> courses` and are deliberately NOT duplicated here:
-- a second copy of `program_id` is a second thing to keep true, and the
-- admin scope check already walks that path (`admin/blocks.rs`).

CREATE TYPE exam_status AS ENUM ('draft', 'published', 'archived');

-- Attempt lifecycle. `submitted` is "handed in, not yet marked"; `graded` is
-- the only state in which `score` is guaranteed non-NULL (enforced below).
CREATE TYPE exam_attempt_status AS ENUM ('in_progress', 'submitted', 'graded', 'abandoned');

CREATE TABLE exams (
  id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  block_id         UUID NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
  title            TEXT NOT NULL,
  description      TEXT,
  -- Scores are numbers, never a rendered "18/20" string — formatting is the
  -- UI's job. DOUBLE PRECISION rather than NUMERIC because the workspace's
  -- sqlx build carries no decimal feature, and half-mark granularity is
  -- exact in binary floating point.
  max_score        DOUBLE PRECISION NOT NULL CHECK (max_score > 0),
  -- NULL = untimed. Capped at ten hours, which is well past any block exam.
  duration_minutes SMALLINT CHECK (duration_minutes BETWEEN 1 AND 600),
  status           exam_status NOT NULL DEFAULT 'draft',
  created_by       UUID NOT NULL REFERENCES users(id),
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (block_id, title)
);

CREATE TABLE exam_attempts (
  id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  exam_id      UUID NOT NULL REFERENCES exams(id)    ON DELETE CASCADE,
  student_id   UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  -- A student may sit the same exam more than once. `attempt_no` makes the
  -- ordering of two attempts unambiguous even when their timestamps tie, so
  -- "the most recent attempt" the dashboard renders is always one row.
  attempt_no   SMALLINT NOT NULL CHECK (attempt_no >= 1),
  -- NULL until marked. `max_score` is snapshotted from `exams` at attempt
  -- time: re-weighting an exam later must not silently restate a score a
  -- student has already been shown.
  score        DOUBLE PRECISION,
  max_score    DOUBLE PRECISION NOT NULL CHECK (max_score > 0),
  status       exam_attempt_status NOT NULL DEFAULT 'in_progress',
  started_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  submitted_at TIMESTAMPTZ,
  UNIQUE (exam_id, student_id, attempt_no),
  CHECK (score IS NULL OR (score >= 0 AND score <= max_score)),
  CHECK (status <> 'graded' OR score IS NOT NULL),
  CHECK (submitted_at IS NULL OR submitted_at >= started_at)
);

-- What the dashboard actually queries: one student's attempts, newest first.
-- `submitted_at` leads because a graded card is ordered by when it was handed
-- in; `started_at` is the tie-break for an attempt still in progress.
CREATE INDEX idx_exam_attempts_student_recent
  ON exam_attempts (student_id, submitted_at DESC NULLS LAST, started_at DESC);

-- The admin side: every attempt at one exam.
CREATE INDEX idx_exam_attempts_exam_id ON exam_attempts (exam_id);

-- A block's exams, the other list route.
CREATE INDEX idx_exams_block_id   ON exams (block_id);
CREATE INDEX idx_exams_created_by ON exams (created_by);
