-- 0002_indexes.sql
-- Hot-path filter indexes plus every foreign key not already covered by a
-- UNIQUE constraint's leading column. Source: IMPLEMENTATION_PLAN.md §3.1
-- ("Every foreign key above is indexed, plus the filter columns used on hot paths...").

-- Hot-path composite filters called out explicitly in the plan.
CREATE INDEX IF NOT EXISTS idx_users_email  ON users (email);
CREATE INDEX IF NOT EXISTS idx_users_role   ON users (role);
CREATE INDEX IF NOT EXISTS idx_users_status ON users (status);

CREATE INDEX IF NOT EXISTS idx_students_roll_number  ON students (roll_number);
CREATE INDEX IF NOT EXISTS idx_students_program_id   ON students (program_id);
CREATE INDEX IF NOT EXISTS idx_students_semester_id  ON students (semester_id);
CREATE INDEX IF NOT EXISTS idx_students_lsc_id       ON students (lsc_id);
CREATE INDEX IF NOT EXISTS idx_students_phone_number ON students (phone_number);

CREATE INDEX IF NOT EXISTS idx_student_courses_student_id ON student_courses (student_id);
CREATE INDEX IF NOT EXISTS idx_student_courses_course_id  ON student_courses (course_id);
CREATE INDEX IF NOT EXISTS idx_student_courses_status     ON student_courses (status);

CREATE INDEX IF NOT EXISTS idx_learning_sessions_student_id  ON learning_sessions (student_id);
CREATE INDEX IF NOT EXISTS idx_learning_sessions_started_at  ON learning_sessions (started_at);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_refresh_token_hash ON auth_sessions (refresh_token_hash);
CREATE INDEX IF NOT EXISTS idx_auth_sessions_expires_at         ON auth_sessions (expires_at);
CREATE INDEX IF NOT EXISTS idx_auth_sessions_revoked_at         ON auth_sessions (revoked_at);

CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id    ON audit_logs (user_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action     ON audit_logs (action);
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs (created_at);

-- Remaining foreign keys not already indexed via a UNIQUE constraint's
-- leading column.

-- students
CREATE INDEX IF NOT EXISTS idx_students_current_block_id ON students (current_block_id);

-- semesters (program_id is the leading column of UNIQUE(program_id, semester_number),
-- but an explicit index keeps program-scoped listing queries independent of that constraint).
CREATE INDEX IF NOT EXISTS idx_semesters_program_id ON semesters (program_id);

-- courses
CREATE INDEX IF NOT EXISTS idx_courses_program_id  ON courses (program_id);
CREATE INDEX IF NOT EXISTS idx_courses_semester_id ON courses (semester_id);

-- blocks
CREATE INDEX IF NOT EXISTS idx_blocks_course_id ON blocks (course_id);

-- sub_admin_scopes (user_id is covered by the composite PK's leading column)
CREATE INDEX IF NOT EXISTS idx_sub_admin_scopes_program_id ON sub_admin_scopes (program_id);

-- documents
CREATE INDEX IF NOT EXISTS idx_documents_block_id    ON documents (block_id);
CREATE INDEX IF NOT EXISTS idx_documents_uploaded_by ON documents (uploaded_by);

-- learning_sessions
CREATE INDEX IF NOT EXISTS idx_learning_sessions_course_id ON learning_sessions (course_id);
CREATE INDEX IF NOT EXISTS idx_learning_sessions_block_id  ON learning_sessions (block_id);

-- auth_sessions
CREATE INDEX IF NOT EXISTS idx_auth_sessions_user_id ON auth_sessions (user_id);

-- password_resets
CREATE INDEX IF NOT EXISTS idx_password_resets_user_id ON password_resets (user_id);

-- safety_incidents
CREATE INDEX IF NOT EXISTS idx_safety_incidents_session_id ON safety_incidents (session_id);
CREATE INDEX IF NOT EXISTS idx_safety_incidents_student_id ON safety_incidents (student_id);

-- board_events
CREATE INDEX IF NOT EXISTS idx_board_events_session_id ON board_events (session_id);

-- note_reminders
CREATE INDEX IF NOT EXISTS idx_note_reminders_student_id    ON note_reminders (student_id);
CREATE INDEX IF NOT EXISTS idx_note_reminders_session_id    ON note_reminders (session_id);
CREATE INDEX IF NOT EXISTS idx_note_reminders_event_from_id ON note_reminders (event_from_id);
CREATE INDEX IF NOT EXISTS idx_note_reminders_event_to_id   ON note_reminders (event_to_id);
