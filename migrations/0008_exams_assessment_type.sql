-- 0008_exams_assessment_type.sql
-- Sampling is filtered by "the exam's `assessment_type`" (contract A1.2), but
-- the type lived only on `question_pool` — an exam had no way to say which kind
-- of paper it sets. This adds that one column. Forward-only and additive; 0006
-- and 0007 are applied and are not edited.
--
-- Nullable, and paired with `question_count` the same way: an exam with both
-- NULL is not an MCQ exam (a written or oral assessment scored by hand), which
-- is the state every exam that shipped before the question pool is in. An
-- attempt can only be started on an exam where both are present, so the
-- nullability is checked once, at the start of an attempt, rather than being
-- papered over with a default that would make old rows claim a paper they have
-- no questions for.
ALTER TABLE exams
  ADD COLUMN assessment_type assessment_type,
  ADD CONSTRAINT exams_mcq_fields_together CHECK (
    (assessment_type IS NULL) = (question_count IS NULL)
  );
