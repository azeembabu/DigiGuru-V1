// The student's own exam record — `GET /api/v1/student/exam-attempts`.
//
// These types mirror `MyExamAttemptResponse` in
// `apps/gateway/src/student/exams.rs` field for field. That shape is
// deliberately *not* the admin `ExamAttemptResponse`: it carries no
// `student_name`, `roll_number`, or `student_id` at all, which is what makes it
// safe on a screen that is also cached and logged (`.claude/rules/security.md`).
// Keep this file in step with that struct, and do not add an identity field
// here — there is nothing on the wire to populate one from.
//
// Fetched from the browser rather than a Server Component for the same reason
// as `student-context.ts`: auth is an httpOnly cookie set by the gateway on its
// own host.

import { apiFetch, apiFetchPage, query } from "@/lib/api";

/**
 * Mirrors `crates/core/src/domain.rs::ExamAttemptStatus` (serde `snake_case`).
 * `graded` is the only state in which `score` is guaranteed present — the
 * schema enforces that, so the UI keys off this rather than null-checking
 * `score` and guessing why it was absent.
 */
export type ExamAttemptStatus = "in_progress" | "submitted" | "graded" | "abandoned";

export type MyExamAttempt = {
  id: string;
  exam_id: string;
  exam_title: string;
  block_id: string;
  block_no: number;
  block_title: string;
  course_id: string;
  course_code: string;
  course_name: string;
  /** Which sitting this was, 1-based. */
  attempt_no: number;
  /** `null` until marked. A number on the wire — "18 / 20" is this UI's job. */
  score: number | null;
  max_score: number;
  status: ExamAttemptStatus;
  started_at: string;
  /** `null` while the attempt is still open. */
  submitted_at: string | null;
};

/**
 * Newest first, paginated like every other list in this API (bare array body,
 * total in `X-Total-Count`).
 *
 * The gateway caps `limit` at 200 and rejects an out-of-range value with
 * `400 VALIDATION_ERROR` rather than clamping, so callers pass a real page size.
 */
export function loadMyExamAttempts(limit = 4): Promise<{ items: MyExamAttempt[]; total: number }> {
  return apiFetchPage<MyExamAttempt>(`/student/exam-attempts${query({ limit })}`);
}

/**
 * One attempt, in the same shape the list returns — the gateway serves an
 * identical struct from both routes, so a review screen codes against one card
 * type rather than a second detail-only shape.
 *
 * An id belonging to another student comes back `404`, not `403`: the gateway
 * binds ownership inside the query's `WHERE` clause, so a foreign id simply
 * selects no row. Treat a 404 here as "not yours or not there" and say neither.
 */
export function loadMyExamAttempt(id: string): Promise<MyExamAttempt> {
  return apiFetch<MyExamAttempt>(`/student/exam-attempts/${encodeURIComponent(id)}`);
}

/** Drop a trailing `.0` so whole marks read as "18", not "18.0". */
function tidy(n: number): string {
  return Number.isInteger(n) ? String(n) : n.toFixed(1);
}

/**
 * `18 / 20` — formatted here, never on the wire.
 *
 * Returns `null` for anything not yet graded, which the card renders as a
 * status chip instead. A submitted-but-unmarked attempt showing `0 / 20` would
 * read as a failure rather than as "not marked yet".
 */
export function formatScore(attempt: MyExamAttempt): string | null {
  if (attempt.status !== "graded" || attempt.score === null) return null;
  return `${tidy(attempt.score)} / ${tidy(attempt.max_score)}`;
}

/**
 * The percentage, for the meter only — `null` when ungraded, and when
 * `max_score` is zero, which would otherwise divide to `Infinity`.
 */
export function scorePercent(attempt: MyExamAttempt): number | null {
  if (attempt.status !== "graded" || attempt.score === null) return null;
  if (attempt.max_score <= 0) return null;
  return Math.max(0, Math.min(100, (attempt.score / attempt.max_score) * 100));
}

/**
 * The date a student would recognise the attempt by: when they submitted it,
 * falling back to when they started it for one still open or abandoned.
 */
export function attemptDate(attempt: MyExamAttempt): string {
  const parsed = new Date(attempt.submitted_at ?? attempt.started_at);
  if (Number.isNaN(parsed.getTime())) return "—";
  return parsed.toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

export const STATUS_LABEL: Record<ExamAttemptStatus, string> = {
  in_progress: "In progress",
  submitted: "Awaiting marks",
  graded: "Graded",
  abandoned: "Not completed",
};

// There is deliberately no `passed` helper in this file. No pass mark exists
// anywhere in the schema or the API, so any threshold chosen here (50 %? 40 %?)
// would be invented by the UI and shown to a student as though the institution
// had set it. The card shows the score and lets the student judge it.
