-- 0010_question_pool_course_scope.sql
-- The question pool is built per **course**; the unit/module is optional.
--
-- The owner's taxonomy is Program > Semester > Course > Unit/Module, and a pool
-- is pre-populated for a course. Until now a question hung off a block and its
-- course was derived through it, which made the module link mandatory — an
-- admin could not author a course-wide question at all.
--
-- Forward-only, and 0006-0009 are applied and untouched: this adds the column,
-- backfills it from the block every existing row already points at, and only
-- then applies NOT NULL, so no row is lost and no applied checksum moves.
--
-- Sampling moves with it (`crates/db/src/models/question_pool.rs`): a paper is
-- drawn from one course's pool, not from the whole program-semester.

ALTER TABLE question_pool ADD COLUMN course_id UUID REFERENCES courses(id) ON DELETE CASCADE;

-- Backfill before the NOT NULL. `block_id` is still NOT NULL at this point, so
-- every existing row resolves to exactly one course and the update cannot leave
-- a NULL behind.
UPDATE question_pool q
SET course_id = b.course_id
FROM blocks b
WHERE b.id = q.block_id AND q.course_id IS NULL;

ALTER TABLE question_pool ALTER COLUMN course_id SET NOT NULL;

-- `block_id` becomes the OPTIONAL unit/module pointer. The FK stays, so a
-- module link still has to name a real block.
--
-- The rule "when present, the block must belong to `course_id`" is deliberately
-- NOT here: it needs a subquery (`blocks.course_id`), and a CHECK constraint
-- cannot contain one. It is enforced in the gateway write path instead — the
-- create and bulk-import handlers resolve the block and reject a mismatch
-- before inserting. A trigger was the alternative and was not taken: a silent
-- rewrite or abort inside the database is harder to surface as a per-row
-- `VALIDATION_ERROR` than an explicit check in the handler that already knows
-- the row number.
ALTER TABLE question_pool ALTER COLUMN block_id DROP NOT NULL;

-- The new sampling path: one course's active pool for one assessment type.
-- Leading with `course_id` because that is the equality predicate that selects
-- the pool at all; the older `(status, assessment_type, block_id)` index stays
-- useful only for module-filtered admin reads.
CREATE INDEX idx_question_pool_course_sampling
  ON question_pool (course_id, status, assessment_type);
