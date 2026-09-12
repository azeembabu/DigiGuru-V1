-- 0001_seed_dev.sql
-- Minimal dev/fixture data for local environments: one LSC, one program
-- (BA Malayalam), 6 semesters, one course per semester, 4 blocks per course.
--
-- This is NOT an sqlx migration (it is not idempotent/forward-only in the
-- migration-history sense) — run it manually against a freshly-migrated
-- local database:
--   psql "$DATABASE_URL" -f migrations/seed/0001_seed_dev.sql

BEGIN;

-- One Learner Support Centre.
INSERT INTO lscs (id, code, name, location, status)
VALUES ('00000000-0000-0000-0000-000000000001', 'LSC-TVM', 'Thiruvananthapuram LSC', 'Thiruvananthapuram, Kerala', 'active');

-- One program: BA Malayalam.
INSERT INTO programs (id, code, name, description, status)
VALUES ('00000000-0000-0000-0000-000000000002', 'BAML', 'BA Malayalam', 'Bachelor of Arts in Malayalam', 'active');

-- 6 semesters.
INSERT INTO semesters (id, program_id, semester_number, name, status) VALUES
  ('00000000-0000-0000-0000-000000000011', '00000000-0000-0000-0000-000000000002', 1, 'Semester 1', 'active'),
  ('00000000-0000-0000-0000-000000000012', '00000000-0000-0000-0000-000000000002', 2, 'Semester 2', 'active'),
  ('00000000-0000-0000-0000-000000000013', '00000000-0000-0000-0000-000000000002', 3, 'Semester 3', 'active'),
  ('00000000-0000-0000-0000-000000000014', '00000000-0000-0000-0000-000000000002', 4, 'Semester 4', 'active'),
  ('00000000-0000-0000-0000-000000000015', '00000000-0000-0000-0000-000000000002', 5, 'Semester 5', 'active'),
  ('00000000-0000-0000-0000-000000000016', '00000000-0000-0000-0000-000000000002', 6, 'Semester 6', 'active');

-- One course per semester.
INSERT INTO courses (id, program_id, semester_id, code, name, description) VALUES
  ('00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000011', 'BAML101', 'Introduction to Malayalam Language', 'Foundations of Malayalam phonology, script, and grammar'),
  ('00000000-0000-0000-0000-000000000022', '00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000012', 'BAML102', 'Malayalam Poetry I', 'Classical and medieval Malayalam poetry'),
  ('00000000-0000-0000-0000-000000000023', '00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000013', 'BAML201', 'Malayalam Prose and Fiction', 'Short stories and the modern Malayalam novel'),
  ('00000000-0000-0000-0000-000000000024', '00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000014', 'BAML202', 'Malayalam Drama', 'Stage drama and dramatic literature in Malayalam'),
  ('00000000-0000-0000-0000-000000000025', '00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000015', 'BAML301', 'Literary Criticism in Malayalam', 'Theory and criticism of Malayalam literature'),
  ('00000000-0000-0000-0000-000000000026', '00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000016', 'BAML302', 'Modern Malayalam Literature', 'Contemporary movements and major modern authors');

-- 4 blocks per course.
INSERT INTO blocks (course_id, block_no, title) VALUES
  ('00000000-0000-0000-0000-000000000021', 1, 'Block 1 — Origins of the Malayalam Script'),
  ('00000000-0000-0000-0000-000000000021', 2, 'Block 2 — Phonology and Sandhi'),
  ('00000000-0000-0000-0000-000000000021', 3, 'Block 3 — Basic Grammar'),
  ('00000000-0000-0000-0000-000000000021', 4, 'Block 4 — Sentence Construction'),

  ('00000000-0000-0000-0000-000000000022', 1, 'Block 1 — Sangam-era Influences'),
  ('00000000-0000-0000-0000-000000000022', 2, 'Block 2 — Cheeraman and Early Poets'),
  ('00000000-0000-0000-0000-000000000022', 3, 'Block 3 — Bhakti Poetry'),
  ('00000000-0000-0000-0000-000000000022', 4, 'Block 4 — Medieval Ballads'),

  ('00000000-0000-0000-0000-000000000023', 1, 'Block 1 — Rise of the Short Story'),
  ('00000000-0000-0000-0000-000000000023', 2, 'Block 2 — The Modern Novel'),
  ('00000000-0000-0000-0000-000000000023', 3, 'Block 3 — Realism and Social Themes'),
  ('00000000-0000-0000-0000-000000000023', 4, 'Block 4 — Regional Voices'),

  ('00000000-0000-0000-0000-000000000024', 1, 'Block 1 — Origins of Malayalam Theatre'),
  ('00000000-0000-0000-0000-000000000024', 2, 'Block 2 — Classical Dramatic Forms'),
  ('00000000-0000-0000-0000-000000000024', 3, 'Block 3 — Modern Playwrights'),
  ('00000000-0000-0000-0000-000000000024', 4, 'Block 4 — Stagecraft and Performance'),

  ('00000000-0000-0000-0000-000000000025', 1, 'Block 1 — Foundations of Literary Theory'),
  ('00000000-0000-0000-0000-000000000025', 2, 'Block 2 — Structuralism and Beyond'),
  ('00000000-0000-0000-0000-000000000025', 3, 'Block 3 — Malayalam Critical Traditions'),
  ('00000000-0000-0000-0000-000000000025', 4, 'Block 4 — Comparative Criticism'),

  ('00000000-0000-0000-0000-000000000026', 1, 'Block 1 — Post-Independence Literature'),
  ('00000000-0000-0000-0000-000000000026', 2, 'Block 2 — Modernist Movements'),
  ('00000000-0000-0000-0000-000000000026', 3, 'Block 3 — Contemporary Authors'),
  ('00000000-0000-0000-0000-000000000026', 4, 'Block 4 — Malayalam Literature Today');

COMMIT;
