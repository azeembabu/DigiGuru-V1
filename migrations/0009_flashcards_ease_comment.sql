-- 0009_flashcards_ease_comment.sql
-- `flashcards.ease` is in hundredths (250 = 2.50), not permille: the inline
-- comment in 0006 said permille and 0006 is applied, so it cannot be corrected
-- in place. Stating the unit as a real column comment rather than leaving the
-- wrong one as the only note in the schema — a reader who divides by 1000
-- schedules every card four times too soon, and the unit is not recoverable
-- from the CHECK range alone.
COMMENT ON COLUMN flashcards.ease IS
  'SM-2 ease factor in hundredths: 250 = 2.50. Floor 130, ceiling 500.';
