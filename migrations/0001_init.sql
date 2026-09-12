-- 0001_init.sql
-- Digi Guru — Phase 1 schema.
-- Forward-only migration. Do not edit after it has been applied to any environment.
-- Source of truth: IMPLEMENTATION_PLAN.md §3.1.

CREATE EXTENSION IF NOT EXISTS pgcrypto;   -- gen_random_uuid()
CREATE EXTENSION IF NOT EXISTS citext;     -- case-insensitive email

-- Academic hierarchy: Program > Semester > Course > Block
-- (carried over from the validated prototype model on origin/master; see §13)

CREATE TYPE user_role     AS ENUM ('super_admin', 'sub_admin', 'student');
CREATE TYPE user_status   AS ENUM ('active', 'inactive', 'suspended');
CREATE TYPE entity_status AS ENUM ('active', 'inactive');

CREATE TABLE users (
  id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  role           user_role   NOT NULL,
  status         user_status NOT NULL DEFAULT 'active',
  email          CITEXT      NOT NULL UNIQUE,
  password_hash  TEXT        NOT NULL,          -- argon2id
  last_login_at  TIMESTAMPTZ,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE admins (                            -- super_admin and sub_admin profile
  id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id   UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
  full_name TEXT NOT NULL
);

CREATE TABLE lscs (                              -- Learner Support Centre (an entity, not a string)
  id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  code     TEXT NOT NULL UNIQUE,
  name     TEXT NOT NULL,
  location TEXT,
  status   entity_status NOT NULL DEFAULT 'active'
);

CREATE TABLE programs (                          -- e.g. "BA Malayalam"
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  code        TEXT NOT NULL UNIQUE,
  name        TEXT NOT NULL,
  description TEXT,
  status      entity_status NOT NULL DEFAULT 'active'
);

CREATE TABLE semesters (
  id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  program_id      UUID NOT NULL REFERENCES programs(id) ON DELETE CASCADE,
  semester_number SMALLINT NOT NULL CHECK (semester_number BETWEEN 1 AND 12),
  name            TEXT NOT NULL,
  status          entity_status NOT NULL DEFAULT 'active',
  UNIQUE (program_id, semester_number)
);

CREATE TABLE courses (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  program_id  UUID NOT NULL REFERENCES programs(id)  ON DELETE CASCADE,
  semester_id UUID NOT NULL REFERENCES semesters(id) ON DELETE CASCADE,
  code        TEXT NOT NULL,
  name        TEXT NOT NULL,
  description TEXT,
  UNIQUE (program_id, semester_id, code)
);

CREATE TABLE blocks (                            -- a teaching unit inside a course
  id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  course_id UUID NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
  block_no  SMALLINT NOT NULL,
  title     TEXT NOT NULL,
  UNIQUE (course_id, block_no)
);

CREATE TABLE students (
  id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id          UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
  full_name        TEXT NOT NULL,
  roll_number      TEXT NOT NULL UNIQUE,
  phone_number     TEXT NOT NULL,
  program_id       UUID NOT NULL REFERENCES programs(id)  ON DELETE RESTRICT,
  semester_id      UUID NOT NULL REFERENCES semesters(id) ON DELETE RESTRICT,
  lsc_id           UUID NOT NULL REFERENCES lscs(id)      ON DELETE RESTRICT,
  current_block_id UUID REFERENCES blocks(id),              -- context persistence target
  is_first_login   BOOLEAN NOT NULL DEFAULT TRUE,           -- NN-2
  locale           TEXT NOT NULL DEFAULT 'ml-IN',
  timezone         TEXT NOT NULL DEFAULT 'Asia/Kolkata',
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TYPE enrollment_status AS ENUM ('active', 'completed', 'dropped');

CREATE TABLE student_courses (                   -- which courses a student may actually study
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  student_id  UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  course_id   UUID NOT NULL REFERENCES courses(id)  ON DELETE CASCADE,
  status      enrollment_status NOT NULL DEFAULT 'active',
  assigned_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (student_id, course_id)
);

CREATE TABLE sub_admin_scopes (                  -- which programs a sub-admin may touch
  user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  program_id UUID NOT NULL REFERENCES programs(id),
  PRIMARY KEY (user_id, program_id)
);

CREATE TABLE documents (
  id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  block_id       UUID NOT NULL REFERENCES blocks(id),
  uploaded_by    UUID NOT NULL REFERENCES users(id),
  title          TEXT NOT NULL,
  storage_key    TEXT NOT NULL,
  sha256         TEXT NOT NULL UNIQUE,           -- dedupe re-uploads
  page_count     INT  NOT NULL,
  ocr_confidence REAL,                           -- NULL = born-digital, no OCR needed
  status         TEXT NOT NULL DEFAULT 'pending',
                 -- pending|parsing|pending_review|embedded|failed
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TYPE session_status AS ENUM ('in_progress', 'completed', 'abandoned');

-- The classroom session. Named `learning_sessions` to keep it distinct from
-- `auth_sessions` (refresh tokens) — the prototype's naming, and worth keeping.
CREATE TABLE learning_sessions (
  id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  student_id      UUID NOT NULL REFERENCES students(id),
  course_id       UUID NOT NULL REFERENCES courses(id),
  block_id        UUID NOT NULL REFERENCES blocks(id),
  started_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  ended_at        TIMESTAMPTZ,
  active_voice_ms BIGINT NOT NULL DEFAULT 0,     -- NN-3, server-authoritative
  status          session_status NOT NULL DEFAULT 'in_progress',
  end_reason      TEXT,                          -- quota|idle|user|jailbreak|error
  resume_summary  TEXT,                          -- feeds the next session recap
  last_topic      TEXT,
  last_page       INT
);

CREATE TABLE auth_sessions (                     -- refresh-token / device sessions
  id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id            UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  refresh_token_hash TEXT NOT NULL UNIQUE,
  device_info        TEXT,
  ip_address         INET,
  created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at         TIMESTAMPTZ NOT NULL,
  revoked_at         TIMESTAMPTZ
);

CREATE TABLE password_resets (
  id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  token_hash TEXT NOT NULL UNIQUE,
  expires_at TIMESTAMPTZ NOT NULL,
  used_at    TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE audit_logs (
  id          BIGSERIAL PRIMARY KEY,
  user_id     UUID REFERENCES users(id) ON DELETE SET NULL,
  action      TEXT NOT NULL,
  ip_address  INET,
  device_info TEXT,
  metadata    JSONB,                             -- must be PII-free (H-43)
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE safety_incidents (
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  session_id  UUID REFERENCES learning_sessions(id),
  student_id  UUID NOT NULL REFERENCES students(id),
  kind        TEXT NOT NULL,                     -- jailbreak|toxicity|out_of_scope
  tier        SMALLINT NOT NULL,                 -- 0 = pre-LLM tap, 1 = transcript, 2 = output
  excerpt     TEXT NOT NULL,                     -- PII-redacted
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE board_events (                      -- whiteboard audit / replay
  id         BIGSERIAL PRIMARY KEY,
  session_id UUID NOT NULL REFERENCES learning_sessions(id),
  turn_seq   INT  NOT NULL,
  op         JSONB NOT NULL,
  emitted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  acked_ms   INT                                 -- client ACK latency; NULL = never acked
);

-- Reminder ledger for exported notes (see .claude/rules/pedagogy.md).
-- Deliberately NOT a content store: it points at a board_events range and a date.
CREATE TABLE note_reminders (
  id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  student_id     UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
  session_id     UUID NOT NULL REFERENCES learning_sessions(id) ON DELETE CASCADE,
  event_from_id  BIGINT NOT NULL REFERENCES board_events(id),
  event_to_id    BIGINT NOT NULL REFERENCES board_events(id),
  remind_at      DATE NOT NULL,
  surfaced_at    TIMESTAMPTZ,                    -- set when shown on login
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
  CHECK (event_to_id >= event_from_id)
);
