// `GET /api/v1/admin/analytics` — the dashboard's metrics and chart series.
//
// Additive to `/admin/stats`, which is unchanged and still served
// (`.claude/rules/api-conventions.md`). Scoping happens entirely server-side:
// a sub-admin's figures already cover only its `sub_admin_scopes` programs, so
// nothing here filters or re-aggregates.
//
// The contract guarantees a shape this file nevertheless checks at runtime:
// the console is a separate deployable from the gateway, so a version skew is
// possible, and a silently-`undefined` metric would render as a confident
// "0" — a claim we would be making up. A shape mismatch throws the same
// `ApiError` envelope every other call throws, so the screen shows its error
// state instead.
//
// Hand-written rather than Zod: `zod` is not a declared dependency of
// `apps/web` (it only resolves transitively), and adding one was out of scope
// for this screen. `lib/api.ts` narrows its error envelope the same way.

import { ApiError, apiFetch } from "@/lib/api";

// ------------------------------------------------------------------- types

export type CatalogueMetrics = {
  programs: number;
  semesters: number;
  courses: number;
  blocks: number;
  blocks_active: number;
  blocks_inactive: number;
  lscs: number;
  blocks_without_documents: number;
  courses_without_blocks: number;
  semesters_without_courses: number;
};

export type StudentMetrics = {
  total: number;
  active: number;
  inactive: number;
  suspended: number;
  first_login_pending: number;
  new_last_30d: number;
  without_enrollment: number;
  enrollments_active: number;
  enrollments_completed: number;
  enrollments_dropped: number;
};

export type DocumentMetrics = {
  total: number;
  pending: number;
  parsing: number;
  pending_review: number;
  embedded: number;
  failed: number;
  total_pages: number;
  /** `null`, never `0`, when nothing has been OCR'd — an unmeasured value. */
  avg_ocr_confidence: number | null;
  jobs_pending: number;
  jobs_processing: number;
  jobs_completed: number;
  jobs_failed: number;
  jobs_retried: number;
};

export type SessionMetrics = {
  total: number;
  in_progress: number;
  completed: number;
  abandoned: number;
  last_7d: number;
  active_voice_ms_total: number;
  active_voice_ms_avg: number | null;
  ended_quota: number;
  ended_idle: number;
  ended_user: number;
  ended_jailbreak: number;
  ended_error: number;
};

export type WhiteboardMetrics = {
  ops_total: number;
  ops_acked: number;
  ops_unacked: number;
  ack_p50_ms: number | null;
  ack_p95_ms: number | null;
  /** `ops_unacked / ops_total`, 0..1 — the same quantity CI's `wb_violation` tracks. */
  violation_rate: number | null;
};

export type SafetyMetrics = {
  total: number;
  tier0: number;
  tier1: number;
  tier2: number;
  last_7d: number;
  jailbreak: number;
  toxicity: number;
  out_of_scope: number;
};

/** A gap-filled day in one of the `*_daily` series: exactly 30, oldest first. */
export type DayPoint = { day: string; count: number };
export type SessionDayPoint = DayPoint & { voice_ms: number };
/** A fixed-label bucket: the full label set, in a fixed order, zeros included. */
export type LabelPoint = { label: string; count: number };

export type AnalyticsSeries = {
  sessions_daily: SessionDayPoint[];
  students_daily: DayPoint[];
  documents_daily: DayPoint[];
  incidents_daily: DayPoint[];
  documents_by_status: LabelPoint[];
  session_end_reasons: LabelPoint[];
  ack_latency_buckets: LabelPoint[];
  top_blocks: LabelPoint[];
};

export type AdminAnalytics = {
  catalogue: CatalogueMetrics;
  students: StudentMetrics;
  documents: DocumentMetrics;
  sessions: SessionMetrics;
  whiteboard: WhiteboardMetrics;
  safety: SafetyMetrics;
  series: AnalyticsSeries;
};

/**
 * The last `ack_latency_buckets` label. Past the NN-1 `HOLD_MAX` ceiling, so a
 * non-zero count here is a whiteboard-first violation, not a slow-ish tail.
 */
export const ACK_VIOLATION_BUCKET = ">400ms";

// -------------------------------------------------------------- validation

function fail(): never {
  throw new ApiError(
    "INTERNAL_ERROR",
    "The server returned an unexpected summary. Please try again later.",
    0,
  );
}

function obj(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) fail();
  return value as Record<string, unknown>;
}

/** A non-negative integer counter. */
function count(source: Record<string, unknown>, key: string): number {
  const value = source[key];
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) fail();
  return value;
}

/** An average or percentile: a finite number, or `null` when unmeasured. */
function maybe(source: Record<string, unknown>, key: string): number | null {
  const value = source[key];
  if (value === null) return null;
  if (typeof value !== "number" || !Number.isFinite(value)) fail();
  return value;
}

function text(source: Record<string, unknown>, key: string): string {
  const value = source[key];
  if (typeof value !== "string") fail();
  return value;
}

function list(source: Record<string, unknown>, key: string): unknown[] {
  const value = source[key];
  if (!Array.isArray(value)) fail();
  return value;
}

function days(source: Record<string, unknown>, key: string): DayPoint[] {
  return list(source, key).map((row) => {
    const point = obj(row);
    return { day: text(point, "day"), count: count(point, "count") };
  });
}

function labels(source: Record<string, unknown>, key: string): LabelPoint[] {
  return list(source, key).map((row) => {
    const point = obj(row);
    return { label: text(point, "label"), count: count(point, "count") };
  });
}

function parse(body: unknown): AdminAnalytics {
  const root = obj(body);
  const catalogue = obj(root.catalogue);
  const students = obj(root.students);
  const documents = obj(root.documents);
  const sessions = obj(root.sessions);
  const whiteboard = obj(root.whiteboard);
  const safety = obj(root.safety);
  const series = obj(root.series);

  return {
    catalogue: {
      programs: count(catalogue, "programs"),
      semesters: count(catalogue, "semesters"),
      courses: count(catalogue, "courses"),
      blocks: count(catalogue, "blocks"),
      blocks_active: count(catalogue, "blocks_active"),
      blocks_inactive: count(catalogue, "blocks_inactive"),
      lscs: count(catalogue, "lscs"),
      blocks_without_documents: count(catalogue, "blocks_without_documents"),
      courses_without_blocks: count(catalogue, "courses_without_blocks"),
      semesters_without_courses: count(catalogue, "semesters_without_courses"),
    },
    students: {
      total: count(students, "total"),
      active: count(students, "active"),
      inactive: count(students, "inactive"),
      suspended: count(students, "suspended"),
      first_login_pending: count(students, "first_login_pending"),
      new_last_30d: count(students, "new_last_30d"),
      without_enrollment: count(students, "without_enrollment"),
      enrollments_active: count(students, "enrollments_active"),
      enrollments_completed: count(students, "enrollments_completed"),
      enrollments_dropped: count(students, "enrollments_dropped"),
    },
    documents: {
      total: count(documents, "total"),
      pending: count(documents, "pending"),
      parsing: count(documents, "parsing"),
      pending_review: count(documents, "pending_review"),
      embedded: count(documents, "embedded"),
      failed: count(documents, "failed"),
      total_pages: count(documents, "total_pages"),
      avg_ocr_confidence: maybe(documents, "avg_ocr_confidence"),
      jobs_pending: count(documents, "jobs_pending"),
      jobs_processing: count(documents, "jobs_processing"),
      jobs_completed: count(documents, "jobs_completed"),
      jobs_failed: count(documents, "jobs_failed"),
      jobs_retried: count(documents, "jobs_retried"),
    },
    sessions: {
      total: count(sessions, "total"),
      in_progress: count(sessions, "in_progress"),
      completed: count(sessions, "completed"),
      abandoned: count(sessions, "abandoned"),
      last_7d: count(sessions, "last_7d"),
      active_voice_ms_total: count(sessions, "active_voice_ms_total"),
      active_voice_ms_avg: maybe(sessions, "active_voice_ms_avg"),
      ended_quota: count(sessions, "ended_quota"),
      ended_idle: count(sessions, "ended_idle"),
      ended_user: count(sessions, "ended_user"),
      ended_jailbreak: count(sessions, "ended_jailbreak"),
      ended_error: count(sessions, "ended_error"),
    },
    whiteboard: {
      ops_total: count(whiteboard, "ops_total"),
      ops_acked: count(whiteboard, "ops_acked"),
      ops_unacked: count(whiteboard, "ops_unacked"),
      ack_p50_ms: maybe(whiteboard, "ack_p50_ms"),
      ack_p95_ms: maybe(whiteboard, "ack_p95_ms"),
      violation_rate: maybe(whiteboard, "violation_rate"),
    },
    safety: {
      total: count(safety, "total"),
      tier0: count(safety, "tier0"),
      tier1: count(safety, "tier1"),
      tier2: count(safety, "tier2"),
      last_7d: count(safety, "last_7d"),
      jailbreak: count(safety, "jailbreak"),
      toxicity: count(safety, "toxicity"),
      out_of_scope: count(safety, "out_of_scope"),
    },
    series: {
      sessions_daily: list(series, "sessions_daily").map((row) => {
        const point = obj(row);
        return {
          day: text(point, "day"),
          count: count(point, "count"),
          voice_ms: count(point, "voice_ms"),
        };
      }),
      students_daily: days(series, "students_daily"),
      documents_daily: days(series, "documents_daily"),
      incidents_daily: days(series, "incidents_daily"),
      documents_by_status: labels(series, "documents_by_status"),
      session_end_reasons: labels(series, "session_end_reasons"),
      ack_latency_buckets: labels(series, "ack_latency_buckets"),
      top_blocks: labels(series, "top_blocks"),
    },
  };
}

export async function loadAnalytics(): Promise<AdminAnalytics> {
  return parse(await apiFetch<unknown>("/admin/analytics"));
}
