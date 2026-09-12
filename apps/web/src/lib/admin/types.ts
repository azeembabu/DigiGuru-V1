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
