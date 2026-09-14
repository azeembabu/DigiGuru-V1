// Wire types for `/api/v1/admin/*`.
//
// These mirror the gateway's response structs field for field
// (`apps/gateway/src/admin/*.rs`, catalogued in `.claude/rules/api-conventions.md`).
// Keep them in step with that contract: the gateway is the source of truth and
// these are a transcription of it, not an independent model.

export type EntityStatus = "active" | "inactive";
export type UserStatus = "active" | "inactive" | "suspended";
export type Role = "super_admin" | "sub_admin" | "student";

export type Program = {
  id: string;
  code: string;
  name: string;
  description: string | null;
  status: EntityStatus;
};

export type Semester = {
  id: string;
  program_id: string;
  semester_number: number;
  name: string;
  status: EntityStatus;
};

export type Course = {
  id: string;
  program_id: string;
  semester_id: string;
  /**
   * Resolved server-side on every course response. The console lists a
   * program's courses flat (across all its semesters), so semester is a column
   * rather than a level of the drill-down — carrying the label on the row is
   * what keeps that from costing a second request per screen.
   */
  semester_number: number;
  semester_name: string;
  code: string;
  name: string;
  description: string | null;
};

export type Block = {
  id: string;
  course_id: string;
  /**
   * Ancestry is denormalised onto every block response so a block page can
   * render its full Program > Semester > Course > Block trail without walking
   * back up the hierarchy one request at a time.
   */
  semester_id: string;
  program_id: string;
  block_no: number;
  title: string;
  description: string | null;
  status: EntityStatus;
};

export type Lsc = {
  id: string;
  code: string;
  name: string;
  location: string | null;
  status: EntityStatus;
};

export type Student = {
  id: string;
  user_id: string;
  full_name: string;
  roll_number: string;
  phone_number: string;
  program_id: string;
  semester_id: string;
  /** Resolved server-side, so a students table never fans out per row. */
  semester_number: number;
  semester_name: string;
  lsc_id: string;
  /** `null` until an admin has placed the student into a block. */
  current_block_id: string | null;
  /** The linked `users.status` — the same field the `status` filter matches. */
  status: UserStatus;
  is_first_login: boolean;
  created_at: string;
  updated_at: string;
};

export type AdminUser = {
  id: string;
  role: Role;
  status: UserStatus;
  email: string;
  last_login_at: string | null;
  created_at: string;
};

/** Ingestion lifecycle of an uploaded PDF (`documents.status`). */
export type DocumentStatus =
  | "pending"
  | "parsing"
  | "pending_review"
  | "embedded"
  | "failed";

/** Progress of the newest `ingestion_jobs` row for a document. */
export type JobStatus = "pending" | "processing" | "completed" | "failed";

export type AdminDocument = {
  id: string;
  block_id: string;
  uploaded_by: string;
  title: string;
  sha256: string;
  page_count: number;
  /** `null` until the parser has scored the document. */
  ocr_confidence: number | null;
  status: DocumentStatus;
  created_at: string;
  /**
   * `documents` has no error column, so the failure reason is carried by the
   * newest ingestion job instead — both are `null` before any job has run.
   */
  job_status: JobStatus | null;
  last_error: string | null;
};

export type AdminStats = {
  programs: number;
  semesters: number;
  courses: number;
  blocks: number;
  lscs: number;
  students: number;
  documents: {
    total: number;
    pending_review: number;
    embedded: number;
    failed: number;
  };
};

/** A list page plus the `X-Total-Count` the gateway reports for it. */
export type Page<T> = {
  items: T[];
  total: number;
};

// ------------------------------------------------- drill-down list rows
//
// The four lists behind the dashboard's metrics
// (`.claude/rules/api-conventions.md` §"Drill-down list endpoints"). Every row
// is denormalised by the gateway so a table renders without a second request
// per row — resist adding a per-row lookup here, that is the N+1 these shapes
// exist to prevent.

export type EnrollmentStatus = "active" | "completed" | "dropped";

export type Enrollment = {
  id: string;
  student_id: string;
  student_name: string;
  roll_number: string;
  course_id: string;
  course_code: string;
  course_name: string;
  semester_number: number;
  status: EnrollmentStatus;
  assigned_at: string;
};

export type SessionStatus = "in_progress" | "completed" | "abandoned";

/** `null` while the session is still running. */
export type SessionEndReason = "quota" | "idle" | "user" | "jailbreak" | "error";

export type LearningSession = {
  id: string;
  student_id: string;
  student_name: string;
  roll_number: string;
  course_id: string;
  course_code: string;
  block_id: string;
  block_no: number;
  block_title: string;
  started_at: string;
  ended_at: string | null;
  /**
   * NN-3: server-authoritative voice time, counted only while voice is active
   * and never above `QUOTA_CAP_MS`. A client clock never contributes to it.
   */
  active_voice_ms: number;
  status: SessionStatus;
  end_reason: SessionEndReason | null;
  last_topic: string | null;
  last_page: number;
  /** Per-session roll-ups, so a bad session is visible without opening it. */
  board_ops: number;
  /** NN-1 breaches: ops acked above `HOLD_MAX` or never acked at all. */
  board_violations: number;
};

export type BoardOpKind = "heading" | "bullets" | "math" | "draw" | "image" | "highlight";

/** The analytics ACK-latency buckets, reused as a board-event filter. */
export type BoardLatencyBucket = "0-100ms" | "100-250ms" | "250-400ms" | ">400ms";

export type BoardEvent = {
  id: number;
  session_id: string;
  turn_seq: number;
  /** Lifted out of `op` by the gateway so a list renders without parsing it. */
  op_kind: BoardOpKind;
  op: unknown;
  emitted_at: string;
  /** `null` when the client never acknowledged the op. */
  acked_ms: number | null;
  is_violation: boolean;
};

export type IncidentKind = "jailbreak" | "toxicity" | "out_of_scope";

export type SafetyIncident = {
  id: string;
  session_id: string | null;
  student_id: string;
  student_name: string;
  roll_number: string;
  kind: IncidentKind;
  /** 0 muted the upstream, 1 tore the socket down, 2 dropped model output. */
  tier: number;
  /** Already PII-redacted at write time — rendered as stored, never re-cleaned. */
  excerpt: string;
  created_at: string;
};

// ------------------------------------------------------------ question pool
//
// AMENDMENT 1 of the exam-module contract. A question hangs off a **block**,
// exactly like `exams` and `documents`; program and semester are reached
// through `blocks -> courses` and are deliberately NOT columns here — a second
// copy of `program_id` is a second thing to keep true. The list response
// denormalises that ancestry for display only.

export type AssessmentType = "assignment" | "mid_term_quiz" | "semester_exam";
export type DifficultyLevel = "beginner" | "intermediate" | "advanced";
export type QuestionStatus = "active" | "retired";

export const ASSESSMENT_TYPES: { value: AssessmentType; label: string }[] = [
  { value: "assignment", label: "Assignment" },
  { value: "mid_term_quiz", label: "Mid-term quiz" },
  { value: "semester_exam", label: "Semester exam" },
];

export const DIFFICULTY_LEVELS: { value: DifficultyLevel; label: string }[] = [
  { value: "beginner", label: "Beginner" },
  { value: "intermediate", label: "Intermediate" },
  { value: "advanced", label: "Advanced" },
];

/**
 * One pooled question.
 *
 * `correct_option_index` and `explanation` are present here because this is the
 * ADMIN shape — the student-facing paper has no field for either until the
 * attempt is submitted.
 *
 * AMENDMENT A2.1: the pool is built per COURSE, so `course_id` is the required
 * link and `block_id` became the OPTIONAL unit/module pointer — hence the three
 * nullable block fields. `course_name` and `semester_number` are optional
 * because the shipped gateway response does not carry them; the UI labels a row
 * from `course_code` and only adds the longer names when they are present,
 * rather than rendering "Sem undefined".
 */
export type PoolQuestion = {
  id: string;
  block_id: string | null;
  block_no: number | null;
  block_title: string | null;
  course_id: string;
  course_code: string;
  course_name?: string;
  semester_id: string;
  semester_number?: number;
  program_id: string;
  topic: string;
  question_text: string;
  /** Exactly four after AMENDMENT A1.1 (A/B/C/D). */
  options: string[];
  correct_option_index: number;
  explanation: string;
  assessment_type: AssessmentType;
  difficulty_level: DifficultyLevel;
  status: QuestionStatus;
  created_by: string;
  created_at: string;
};

// ----------------------------------------------------------- student reports
//
// AMENDMENT A2.4. The academic record behind a student's exam attempts.

export type AttemptStatus = "in_progress" | "submitted" | "graded" | "abandoned";

export const ATTEMPT_STATUSES: { value: AttemptStatus; label: string }[] = [
  { value: "in_progress", label: "In progress" },
  { value: "submitted", label: "Submitted" },
  { value: "graded", label: "Graded" },
  { value: "abandoned", label: "Abandoned" },
];

/**
 * One attempt, denormalised enough to render a row without a second request.
 *
 * `score`, `percentage` and `time_spent_seconds` are `null` rather than `0`
 * until there is a real measurement: an ungraded attempt shown as 0% would
 * misreport it as a failed one, and 0 seconds would claim it took no time.
 */
export type StudentReportRow = {
  attempt_id: string;
  student_id: string;
  student_name: string;
  roll_number: string;
  program_id: string;
  program_name: string;
  semester_number: number;
  course_id: string;
  course_code: string;
  course_name: string;
  exam_id: string;
  exam_title: string;
  assessment_type: AssessmentType;
  attempt_no: number;
  total_questions: number;
  answered_questions: number;
  correct_answers: number;
  score: number | null;
  max_score: number;
  percentage: number | null;
  /** Derived `submitted_at - started_at`; `null` while in progress. */
  time_spent_seconds: number | null;
  status: AttemptStatus;
  started_at: string;
  submitted_at: string | null;
  weak_topics: string[];
};

export type StudentReportCourse = {
  course_id: string;
  course_code: string;
  course_name: string;
  attempts: number;
  average_percentage: number | null;
};

export type StudentReportWeakTopic = {
  topic: string;
  missed_count: number;
};

export type StudentReport = {
  student_id: string;
  student_name: string;
  roll_number: string;
  program_name: string;
  semester_number: number;
  lsc_code: string | null;
  attempts_total: number;
  attempts_graded: number;
  /** `null` with nothing to average — never 0. */
  average_percentage: number | null;
  best_percentage: number | null;
  total_time_spent_seconds: number;
  by_course: StudentReportCourse[];
  /** Descending by `missed_count`, as the gateway orders it. */
  weak_topics: StudentReportWeakTopic[];
};

/**
 * One course's pool size, from
 * `GET /admin/programs/{program_id}/question-pool-counts`.
 *
 * Unpaginated and includes courses with an empty pool, which is the whole point
 * — "which courses still have nothing" cannot be answered by a list that omits
 * them. `active_questions` counts `status='active'` only, across every
 * assessment type.
 */
export type QuestionPoolCount = {
  course_id: string;
  course_code: string;
  course_name: string;
  semester_id: string;
  semester_number: number;
  active_questions: number;
};

// ------------------------------------------------------------- performance
//
// `GET /admin/students/{id}/performance` and `GET /admin/top-performers`.
//
// `level`, `stars` and `trophy` are computed by the GATEWAY, not here. The
// student's own assessment page reads the same derivation over the same
// `exam_attempts` rows, so banding the percentages a second time in the
// browser would let the two screens disagree about the same student. See
// `crates/core/src/performance.rs`.

export type PerformanceLevel =
  | "not_assessed"
  | "needs_work"
  | "developing"
  | "proficient"
  | "strong"
  | "excellent";

export type Trophy = "gold" | "silver" | "bronze";

export type PerformanceWeakTopic = { topic: string; missed_count: number };

/** One block's line in a student's performance record. */
export type BlockPerformance = {
  block_id: string;
  block_no: number;
  block_title: string;
  course_id: string;
  course_code: string;
  course_name: string;
  semester_number: number;
  exams_available: number;
  attempts_total: number;
  attempts_graded: number;
  /** `null` means never assessed; `0` would be a measured zero. */
  average_percentage: number | null;
  best_percentage: number | null;
  level: PerformanceLevel;
  level_label: string;
  /** `null` for an unassessed block — not zero stars. */
  stars: number | null;
  sessions_total: number;
  sessions_completed: number;
  active_voice_ms: number;
  last_studied_at: string | null;
  weak_topics: PerformanceWeakTopic[];
  /**
   * The student's own words. Read-only to an admin: there is no admin write
   * path, because a remark an admin could edit would stop being the student's.
   */
  remark: string | null;
  remark_updated_at: string | null;
};

export type PerformanceSummary = {
  blocks_total: number;
  blocks_assessed: number;
  attempts_graded: number;
  average_percentage: number | null;
  best_percentage: number | null;
  level: PerformanceLevel;
  level_label: string;
  stars: number | null;
  trophy: Trophy | null;
  attempts_until_trophy: number;
  total_active_voice_ms: number;
};

export type StudentPerformance = {
  student_id: string;
  student_name: string;
  roll_number: string;
  program_name: string;
  semester_number: number;
  lsc_code: string | null;
  summary: PerformanceSummary;
  blocks: BlockPerformance[];
};

/**
 * One line of the best-performers board.
 *
 * `rank` is assigned server-side after ordering, so the client renders the
 * position rather than inferring it from an array it might re-sort. Only
 * graded attempts count, and a student below the minimum does not appear at
 * all — ranking one paper against twenty is a sampling artefact.
 */
export type TopPerformer = {
  rank: number;
  student_id: string;
  student_name: string;
  roll_number: string;
  program_name: string;
  semester_number: number;
  attempts_graded: number;
  average_percentage: number;
  best_percentage: number;
  level: PerformanceLevel;
  level_label: string;
  stars: number | null;
  trophy: Trophy | null;
};
