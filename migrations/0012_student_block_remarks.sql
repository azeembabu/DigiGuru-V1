-- 0012_student_block_remarks.sql
-- The student's own written reflection on a block they have studied.
--
-- The assessment page shows a student how they performed on each block and lets
-- them write their own remark against it ("I need to redo the conservation
-- part"). That note is the ONE thing on the page that is not derivable from
-- data the platform already holds — every score, star and weak topic is
-- computed from `exam_attempts`, `learning_sessions` and `board_events` at read
-- time, deliberately, so the student's page and the admin's cannot disagree
-- (the same argument `student_reports` makes for having no mirrored gradebook).
--
-- So this table stores the remark and nothing else. It is not a second copy of
-- any score.
--
-- Numbered 0012, skipping 0011: `0011_document_thumbnails` is applied on the
-- unmerged `worktree-unit-thumbnails` branch, and reusing the number would make
-- the two branches unmergeable (`sqlx` keys applied migrations by version).
--
-- Authorship is the student's alone. There is no admin write path to this
-- table: an admin reading a student's report may see the remark, because it is
-- context for their performance, but a remark an admin could edit would stop
-- being the student's own words.

CREATE TABLE student_block_remarks (
  id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  block_id   UUID NOT NULL REFERENCES blocks(id)   ON DELETE CASCADE,
  -- Free text, capped in the handler rather than by a domain: the limit is a
  -- product decision that will move, and a CHECK on length would need a
  -- migration to change.
  remark     TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

  -- One remark per student per block. The write path is an upsert on this
  -- constraint, so editing a remark replaces it rather than accumulating a
  -- history the page has no way to show.
  UNIQUE (student_id, block_id)
);

-- The read is always "this student's remarks, for these blocks" — the
-- assessment page fetches a whole course at once — so the student leads the
-- index. `UNIQUE (student_id, block_id)` already provides it; this covers the
-- admin's per-student report, which filters on `student_id` alone.
CREATE INDEX idx_student_block_remarks_student ON student_block_remarks (student_id);

COMMENT ON TABLE student_block_remarks IS
  'A student''s own written reflection on one block. Authored only by the student; never a copy of a computed score.';
