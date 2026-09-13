-- 0007_question_pool_metadata.sql
-- Product-owner refinement of the question pool, landed as a separate forward
-- migration because 0006 is already applied and an applied migration is never
-- edited (`CLAUDE.md`): its checksum is what proves every database in the
-- fleet got the same DDL.
--
-- Three changes, all to `question_pool`:
--   1. Mandatory `assessment_type` and `difficulty_level`.
--   2. A paper is exactly A/B/C/D — four options, never 2..6.
--   3. `explanation` is mandatory: it drives the post-exam feedback screen, so
--      a question without one cannot be remediated and must not be authorable.
--
-- `program_id`/`semester_id` are deliberately still NOT columns here even
-- though sampling is now semester-wide: they are reached through
-- `question_pool -> blocks -> courses`, and a second copy is a second thing to
-- keep true.

-- Which paper a question may be drawn into. Sampling filters on this, so it is
-- part of the pool's identity rather than a tag on the exam alone.
CREATE TYPE assessment_type AS ENUM ('assignment', 'mid_term_quiz', 'semester_exam');

-- Mirrors the adaptive-difficulty tiers in `.claude/rules/pedagogy.md`, so a
-- question authored as `beginner` means the same thing the tutor's tier 0 does.
CREATE TYPE difficulty_level AS ENUM ('beginner', 'intermediate', 'advanced');

-- `assessment_type` has no sensible default — which paper a question belongs to
-- is an editorial decision — but NOT NULL needs one for the rewrite of any
-- existing row. The pool shipped empty in 0006 and nothing writes it yet, so
-- the default is applied to nothing and is dropped immediately afterwards to
-- keep the column a required input at the API boundary.
ALTER TABLE question_pool
  ADD COLUMN assessment_type  assessment_type  NOT NULL DEFAULT 'assignment',
  ADD COLUMN difficulty_level difficulty_level NOT NULL DEFAULT 'beginner';

ALTER TABLE question_pool ALTER COLUMN assessment_type DROP DEFAULT;

-- Exactly four options. Replacing the 2..6 constraint rather than adding a
-- second one: two overlapping CHECKs on the same column mean a future reader
-- has to intersect them to know what is legal.
ALTER TABLE question_pool DROP CONSTRAINT question_pool_options_is_string_array;
ALTER TABLE question_pool DROP CONSTRAINT question_pool_correct_index_in_range;

ALTER TABLE question_pool
  ADD CONSTRAINT question_pool_options_is_four_strings CHECK (
    jsonb_typeof(options) = 'array'
    AND jsonb_array_length(options) = 4
    AND NOT jsonb_path_exists(options, '$[*] ? (@.type() != "string")')
    AND NOT jsonb_path_exists(options, '$[*] ? (@ == "")')
  );

-- 0..3 stated as its own constraint as well as being implied by the array
-- length above: the answer key is the field a bad import is most likely to get
-- wrong, and an explicit range reads in the error message.
ALTER TABLE question_pool
  DROP CONSTRAINT question_pool_correct_option_index_check;

ALTER TABLE question_pool
  ADD CONSTRAINT question_pool_correct_option_index_check
    CHECK (correct_option_index BETWEEN 0 AND 3);

ALTER TABLE question_pool
  ALTER COLUMN explanation SET NOT NULL,
  ADD CONSTRAINT question_pool_explanation_not_blank
    CHECK (length(btrim(explanation)) > 0);

-- The admin list filters on this pair.
CREATE INDEX idx_question_pool_assessment_status ON question_pool (assessment_type, status);

-- Sampling is program + semester wide (A1.2): the pool is every active question
-- whose `block -> course` is in the student's program and semester, narrowed by
-- `assessment_type`. This index is the `question_pool` side of that join —
-- the planner reaches `blocks -> courses` from it, and the leading columns are
-- the two equality predicates every sample applies.
CREATE INDEX idx_question_pool_sampling
  ON question_pool (status, assessment_type, block_id);
