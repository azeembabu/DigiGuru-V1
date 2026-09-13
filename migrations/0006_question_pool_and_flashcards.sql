-- 0006_question_pool_and_flashcards.sql
-- The MCQ question bank behind the exam module, the per-attempt paper it
-- samples, and the student's flashcard notebook.
--
-- Forward-only: one new type, three new tables, and one added nullable column
-- on `exams`. Nothing existing is rewritten, so every shipped query and every
-- `query_as!` call site keeps compiling unchanged.
--
-- A question hangs off a `block`, like `exams` and `documents` do. Program and
-- semester are reached through `question_pool -> blocks -> courses` and are
-- deliberately not duplicated: a second copy of `program_id` is a second thing
-- to keep true, and the admin scope check already walks that path.

CREATE TYPE question_status AS ENUM ('active', 'retired');

CREATE TABLE question_pool (
  id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  block_id             UUID NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
  -- Drives weak-area tagging: an attempt's wrong answers are rolled up by
  -- this value, so it is the unit a student is told to revise and is required
  -- rather than nullable.
  topic                TEXT NOT NULL CHECK (length(btrim(topic)) > 0),
  question_text        TEXT NOT NULL CHECK (length(btrim(question_text)) > 0),
  -- A JSON array of 2..6 option strings. Stored as JSONB rather than a child
  -- table because options have no identity of their own — they are never
  -- referenced, reordered or scored individually, only rendered and indexed
  -- into — and a one-row-per-option table would make every sampling query a
  -- second aggregation.
  options              JSONB NOT NULL,
  correct_option_index SMALLINT NOT NULL CHECK (correct_option_index >= 0),
  explanation          TEXT,
  status               question_status NOT NULL DEFAULT 'active',
  created_by           UUID NOT NULL REFERENCES users(id),
  created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
  -- Shape of `options`, enforced here and not only in the handler: a row that
  -- reaches the table by any other path (a fixup script, a future importer)
  -- must still render as an MCQ.
  CONSTRAINT question_pool_options_is_string_array CHECK (
    jsonb_typeof(options) = 'array'
    AND jsonb_array_length(options) BETWEEN 2 AND 6
    -- jsonpath rather than `NOT EXISTS (SELECT ...)`: a CHECK constraint may
    -- not contain a subquery, and `jsonb_path_exists` is immutable, so this is
    -- the only way to assert a property of every element inline.
    AND NOT jsonb_path_exists(options, '$[*] ? (@.type() != "string")')
    AND NOT jsonb_path_exists(options, '$[*] ? (@ == "")')
  ),
  -- The answer key must point at an option that exists. Without this a
  -- question can be authored that no attempt can ever get right, and the
  -- failure surfaces only as a student's wrong score.
  CONSTRAINT question_pool_correct_index_in_range CHECK (
    correct_option_index < jsonb_array_length(options)
  )
);

-- The sampling query (`ORDER BY random() LIMIT n`) and the admin list both
-- filter on exactly this pair.
CREATE INDEX idx_question_pool_block_status ON question_pool (block_id, status);
CREATE INDEX idx_question_pool_created_by   ON question_pool (created_by);

-- The per-attempt paper. Rows are written when the attempt starts, which is
-- what makes a reload show the same questions in the same order: the sample
-- is a stored fact about the attempt, not something re-drawn per request.
-- ("No two attempts identical" is a property across attempts, not within one.)
CREATE TABLE exam_attempt_answers (
  id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  attempt_id            UUID NOT NULL REFERENCES exam_attempts(id) ON DELETE CASCADE,
  -- No ON DELETE CASCADE, deliberately: a retired question stays referenced by
  -- the attempts that asked it, so a graded paper remains reviewable. Retiring
  -- is `status = 'retired'`, not a delete.
  question_id           UUID NOT NULL REFERENCES question_pool(id),
  -- 1..N, the order *this* attempt saw. The exam runner pages by this, so it
  -- is the stable address of a question within the attempt.
  question_seq          SMALLINT NOT NULL CHECK (question_seq >= 1),
  -- NULL = unanswered. Distinct from 0, which is a real choice of option A.
  selected_option_index SMALLINT CHECK (selected_option_index >= 0),
  -- NULL until graded. Grading is server-side and happens once, at submit.
  is_correct            BOOLEAN,
  UNIQUE (attempt_id, question_seq),
  -- One attempt never asks the same question twice, which is also what makes
  -- `ORDER BY random()` sampling safe to insert without de-duplicating first.
  UNIQUE (attempt_id, question_id)
);

-- Fetching a whole paper in `question_seq` order is the hot read: both the
-- runner and the grader do it.
CREATE INDEX idx_exam_attempt_answers_attempt ON exam_attempt_answers (attempt_id, question_seq);
CREATE INDEX idx_exam_attempt_answers_question ON exam_attempt_answers (question_id);

-- The student's flashcard notebook, with an SM-2-lite spaced-repetition
-- ledger. A card belongs to one student: there is no shared deck, so every
-- query in `crates/db` binds `student_id` and self-only is a schema property
-- rather than a handler convention.
CREATE TABLE flashcards (
  id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  student_id        UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  block_id          UUID NOT NULL REFERENCES blocks(id)   ON DELETE CASCADE,
  front             TEXT NOT NULL CHECK (length(btrim(front)) > 0),
  back              TEXT NOT NULL CHECK (length(btrim(back)) > 0),
  topic             TEXT,
  -- Which live session produced the card, when one did. Nullable because a
  -- card may also be authored outside a session; ON DELETE SET NULL so
  -- pruning old session rows never destroys a student's notebook.
  source_session_id UUID REFERENCES learning_sessions(id) ON DELETE SET NULL,
  -- A DATE, not a timestamp: "due today" is a calendar question, answered in
  -- the student's own zone by the caller passing that day's date in. Storing
  -- an instant would invite a UTC comparison and the NN-3 bug class with it.
  due_on            DATE NOT NULL DEFAULT current_date,
  interval_days     SMALLINT NOT NULL DEFAULT 0 CHECK (interval_days BETWEEN 0 AND 3650),
  -- Ease factor in permille (250 = 2.50), kept integral so review arithmetic
  -- is exact and reproducible. Floor of 130 matches SM-2.
  ease              SMALLINT NOT NULL DEFAULT 250 CHECK (ease BETWEEN 130 AND 500),
  reviewed_at       TIMESTAMPTZ,
  created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- The notebook's two reads: one student's cards, and the due subset the
-- dashboard counts.
CREATE INDEX idx_flashcards_student_due   ON flashcards (student_id, due_on);
CREATE INDEX idx_flashcards_student_block ON flashcards (student_id, block_id);
CREATE INDEX idx_flashcards_source_session ON flashcards (source_session_id);

-- How many questions this exam's paper draws from its block's pool. NULL = not
-- an MCQ exam (a written or oral assessment scored by hand), which is why it
-- is nullable rather than defaulted: every exam that shipped before this
-- migration predates the question pool and must not start claiming a paper
-- size it has no questions for.
ALTER TABLE exams
  ADD COLUMN question_count SMALLINT CHECK (question_count BETWEEN 1 AND 200);
