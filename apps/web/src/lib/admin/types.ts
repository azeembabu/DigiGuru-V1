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
