-- 0013_unit_assessments.sql
-- The tutor's assessment of one student's conversational performance on one unit.
--
-- # Why a session has to record its unit first
--
-- `session_init` has always accepted a `document_id`, but only used it to
-- narrow retrieval — it was never stored. So nothing in the database knew which
-- unit a student had actually studied, and "how did I do on Unit 3" had no row
-- to be answered from. `learning_sessions.document_id` closes that.
--
-- Nullable, deliberately: a student may open a whole block rather than one
-- unit, and that session is still a real session. NULL means "the whole block",
-- never "unknown".
--
-- # What an assessment is, and what it is not
--
-- It is the tutor's judgement of how the student engaged in conversation during
-- one session on one unit: how much they spoke, what they asked, how they
-- answered the comprehension checks. It is NOT an exam result. Exam marks live
-- in `exam_attempts` and are computed from a stored answer key; nothing here
-- touches them, and the two are shown separately so a student can never mistake
-- a conversational rating for a graded paper.
--
-- The counters are stored alongside the verdict on purpose. The model writes
-- the prose, but the numbers it was given are recorded next to it, so a student
-- who asks "why did I get three stars" can be shown the turns, questions and
-- minutes the rating was actually based on. A verdict with no evidence behind
-- it is not defensible to the person receiving it.
--
-- # One row per session, not per unit
--
-- A student studies a unit more than once and should be able to see that they
-- improved. `UNIQUE (session_id)` keeps one assessment per sitting; the
-- assessment page shows the latest per unit and keeps the rest as history.

ALTER TABLE learning_sessions
  ADD COLUMN document_id UUID REFERENCES documents(id) ON DELETE SET NULL;

COMMENT ON COLUMN learning_sessions.document_id IS
  'The unit the student chose for this session. NULL = the whole block, never unknown.';

-- Sessions are read by unit on the assessment page, always for one student.
CREATE INDEX idx_learning_sessions_student_document
  ON learning_sessions (student_id, document_id);

CREATE TABLE unit_assessments (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  student_id  UUID NOT NULL REFERENCES students(id)  ON DELETE CASCADE,
  document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  -- Denormalised from the document so the assessment survives as a block-level
  -- record and can be grouped without a join through `documents`.
  block_id    UUID NOT NULL REFERENCES blocks(id)    ON DELETE CASCADE,

  -- The sitting this verdict is about. Nulled rather than cascaded when a
  -- session is removed: the assessment is the student's record of their own
  -- progress and outlives the socket that produced it.
  session_id  UUID REFERENCES learning_sessions(id) ON DELETE SET NULL,

  -- The verdict. `stars` and `mark` are both required: stars are what the
  -- student reads at a glance, the mark is what makes two sittings comparable.
  stars       SMALLINT NOT NULL CHECK (stars BETWEEN 1 AND 5),
  mark        DOUBLE PRECISION NOT NULL CHECK (mark >= 0 AND mark <= 100),
  -- gold|silver|bronze, or NULL for a sitting that earned none. There is no
  -- consolation trophy: a medal for a poor session is not an honest signal.
  trophy      TEXT CHECK (trophy IN ('gold', 'silver', 'bronze')),

  -- The detailed breakdown the student reads. NOT NULL because a rating with no
  -- explanation is exactly the thing this feature exists to avoid.
  summary     TEXT NOT NULL,
  -- Short bullet lists, stored as JSONB arrays of strings. JSONB rather than
  -- TEXT[] so the shape matches what the model returns and what the API sends,
  -- with no array-literal round trip in between.
  strengths    JSONB NOT NULL DEFAULT '[]'::jsonb,
  improvements JSONB NOT NULL DEFAULT '[]'::jsonb,

  -- The evidence the verdict was based on, counted by the gateway itself and
  -- never by the model. These are measurements; the prose above is judgement,
  -- and keeping them in separate columns keeps that distinction visible.
  student_turns        INT NOT NULL DEFAULT 0,
  questions_asked      INT NOT NULL DEFAULT 0,
  comprehension_passed INT NOT NULL DEFAULT 0,
  comprehension_failed INT NOT NULL DEFAULT 0,
  active_voice_ms      BIGINT NOT NULL DEFAULT 0,

  created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

  -- One verdict per sitting. A retry of the assessment job must replace its
  -- row rather than add a second opinion on the same conversation.
  UNIQUE (session_id)
);

-- The assessment page reads "this student's assessments for these units,
-- newest first".
CREATE INDEX idx_unit_assessments_student_document
  ON unit_assessments (student_id, document_id, created_at DESC);

-- The admin's per-student view groups by block.
CREATE INDEX idx_unit_assessments_block ON unit_assessments (block_id);

COMMENT ON TABLE unit_assessments IS
  'The tutor''s judgement of one student''s conversational engagement in one unit sitting. Not an exam result; exam marks live in exam_attempts.';
