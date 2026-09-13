// Every number the quiet-luxury dashboard shows, derived from the four student
// payloads and nothing else.
//
// This module is pure and framework-free on purpose: the honest-null rules are
// the part of the dashboard most likely to regress into a cheerful `0%`, so they
// live where they can be read (and unit-tested) without rendering anything.
//
// The one rule everything here obeys:
//
//   `null` means "not measured". `0` means "measured, and it was zero".
//
// A student with no graded attempt has no average — not an average of zero. A
// course with one attempt has no trend — not a flat one. Every function below
// returns `null` for the first case and a real number only for the second, and
// no caller is allowed to coalesce one into the other. (The seeded smoke student
// has a genuine 0% attempt, which is exactly why the distinction has to survive
// all the way to the axis.)

import type {
  ExamAttemptRow,
  ExamHistoryEntry,
  SavedResources,
  StudentExam,
} from "@/lib/student";

/** A percentage 0..100, or `null` when there is nothing to measure. */
export type Percentage = number | null;

// --------------------------------------------------------------- percentages

/**
 * An attempt's percentage, derived from the numbers on the wire.
 *
 * `score` is `null` until the attempt is marked, and a `max_score` of zero
 * cannot produce a percentage at all — both are "not measured", not zero.
 */
export function attemptPercentage(score: number | null, maxScore: number): Percentage {
  if (score === null || !Number.isFinite(score)) return null;
  if (!Number.isFinite(maxScore) || maxScore <= 0) return null;
  return (score / maxScore) * 100;
}

/** True when an attempt contributes a real measurement. */
export function isGraded(attempt: ExamAttemptRow): boolean {
  return attemptPercentage(attempt.score, attempt.max_score) !== null;
}

/** Mean of whatever was actually measured; `null` when nothing was. */
export function mean(values: number[]): Percentage {
  const usable = values.filter((n) => Number.isFinite(n));
  if (usable.length === 0) return null;
  return usable.reduce((a, b) => a + b, 0) / usable.length;
}

// ------------------------------------------------------------ speaking time

export type SpeakingTime = {
  usedMinutes: number;
  maxMinutes: number;
  /** 0..1 for the meter fill. Always a real number — the quota always exists. */
  fraction: number;
  remainingMinutes: number;
  isLocked: boolean;
};

/**
 * NN-3, as the dark contrast card shows it.
 *
 * Every field is a straight read of the server's Redis ledger. Nothing here
 * derives the day boundary, the elapsed time, or the remaining allowance — the
 * client's clock is not trusted for any of them, and `resets_at` is formatted
 * elsewhere as a display of the server's instant.
 */
export function speakingTime(quota: {
  minutes_used: number;
  minutes_max: number;
  ms_remaining: number;
  is_locked: boolean;
}): SpeakingTime {
  const maxMinutes = quota.minutes_max > 0 ? quota.minutes_max : 20;
  const usedMinutes = Math.min(Math.max(quota.minutes_used, 0), maxMinutes);
  return {
    usedMinutes,
    maxMinutes,
    fraction: maxMinutes > 0 ? usedMinutes / maxMinutes : 0,
    remainingMinutes: Math.max(0, Math.ceil(quota.ms_remaining / 60_000)),
    isLocked: quota.is_locked,
  };
}

// ----------------------------------------------------------- revision backlog

export type RevisionBacklog = {
  flashcardsDue: number;
  revisionsDue: number;
  total: number;
  /** True when the student has saved nothing at all, as opposed to owing nothing. */
  nothingSavedYet: boolean;
};

/**
 * Substitutes for the requested "study hours logged" tile.
 *
 * A per-student cumulative session time is NOT on any student endpoint — it
 * exists only in admin analytics — so there is no honest way to render it here.
 * This is the nearest real, actionable number the payload carries.
 *
 * `nothingSavedYet` separates the two zeroes: a student with no flashcards at
 * all gets an explanation of where flashcards come from, while a student who has
 * cleared their queue gets told they are clear. Both would read "0" otherwise.
 */
export function revisionBacklog(saved: SavedResources): RevisionBacklog {
  const flashcardsDue = Math.max(0, saved.flashcards_due_for_review);
  const revisionsDue = Math.max(0, saved.revisions_due_count);
  return {
    flashcardsDue,
    revisionsDue,
    total: flashcardsDue + revisionsDue,
    nothingSavedYet:
      saved.flashcards_total === 0 &&
      saved.revisions_due_count === 0 &&
      saved.preserved_notes_count === 0,
  };
}

// -------------------------------------------------------- assessment progress

export type AssessmentProgress = {
  attempted: number;
  total: number;
  /** 0..1, or `null` when the semester has no published assessment to measure. */
  fraction: number | null;
};

/**
 * Substitutes for the requested "assignments completed 28/30" tile.
 *
 * Derived honestly from `GET /student/exams`: an assessment counts as attempted
 * when `attempts_used > 0`. That is genuinely "attempted", not "completed" or
 * "passed" — the label in the UI says so, because a student who scored 0% on
 * every one of them has still attempted them all and must not be shown a
 * completion claim.
 *
 * With no published assessments the fraction is `null`: 0/0 is not 0%.
 */
export function assessmentProgress(exams: StudentExam[]): AssessmentProgress {
  const total = exams.length;
  const attempted = exams.filter((e) => e.attempts_used > 0).length;
  return { attempted, total, fraction: total > 0 ? attempted / total : null };
}

// ------------------------------------------------------------- weak topics

export type WeakTopic = { topic: string; count: number };

/**
 * Substitutes for the requested "interactive learning map by region".
 *
 * There is no geographic data anywhere in this product, so a map would have to
 * be invented wholesale. The weak topics the grader already writes onto every
 * attempt are the real version of the same idea: where to put your attention.
 *
 * Counted across attempts, most-missed first. A topic flagged on three separate
 * attempts is a stronger signal than one flagged once, which is the whole point
 * of counting rather than de-duplicating into a flat list. Ties break
 * alphabetically so the order is total and the panel does not reshuffle between
 * refreshes.
 */
export function weakTopics(history: ExamHistoryEntry[]): WeakTopic[] {
  const counts = new Map<string, number>();

  for (const entry of history) {
    for (const raw of entry.weak_topics) {
      const topic = raw.trim();
      if (topic === "") continue;
      counts.set(topic, (counts.get(topic) ?? 0) + 1);
    }
  }

  return [...counts.entries()]
    .map(([topic, count]) => ({ topic, count }))
    .sort((a, b) => b.count - a.count || a.topic.localeCompare(b.topic));
}

// ------------------------------------------------------- performance series

export type PerformancePoint = {
  attemptId: string;
  examTitle: string;
  courseCode: string;
  attemptNo: number;
  percentage: number;
  /** The instant the bar is placed at — a submitted attempt always has one. */
  submittedAt: string;
  submittedMs: number;
};

export type PerformanceSeries = {
  /** Graded attempts only, oldest first so the chart reads left to right. */
  points: PerformancePoint[];
  /** Attempts excluded because they carry no score yet. Reported, not plotted. */
  ungradedCount: number;
  best: PerformancePoint | null;
  average: Percentage;
};

/**
 * The main graph's data: one column per graded attempt, over `submitted_at`.
 *
 * An attempt with no score is counted and named in a footnote rather than
 * plotted — plotting it would put it on the axis at zero, which is a real score
 * someone else earned. An attempt that is graded but has no `submitted_at` is
 * also excluded, because it has no position on a time axis.
 */
export function performanceSeries(attempts: ExamAttemptRow[]): PerformanceSeries {
  const points: PerformancePoint[] = [];
  let ungradedCount = 0;

  for (const attempt of attempts) {
    const percentage = attemptPercentage(attempt.score, attempt.max_score);
    const submittedMs = attempt.submitted_at === null ? Number.NaN : Date.parse(attempt.submitted_at);

    if (percentage === null || attempt.submitted_at === null || !Number.isFinite(submittedMs)) {
      ungradedCount += 1;
      continue;
    }

    points.push({
      attemptId: attempt.id,
      examTitle: attempt.exam_title,
      courseCode: attempt.course_code,
      attemptNo: attempt.attempt_no,
      percentage,
      submittedAt: attempt.submitted_at,
      submittedMs,
    });
  }

  points.sort((a, b) => a.submittedMs - b.submittedMs || a.attemptNo - b.attemptNo);

  // `reduce` rather than a sort copy: the best is a single pass, and ties keep
  // the earlier attempt so the emphasis does not hop between equal scores.
  const best = points.reduce<PerformancePoint | null>(
    (acc, p) => (acc === null || p.percentage > acc.percentage ? p : acc),
    null,
  );

  return { points, ungradedCount, best, average: mean(points.map((p) => p.percentage)) };
}

/** The timeframe filter's options. Every one filters real rows. */
export const TIMEFRAMES = [
  { id: "all", label: "All time", days: null },
  { id: "90d", label: "Last 90 days", days: 90 },
  { id: "30d", label: "Last 30 days", days: 30 },
  { id: "7d", label: "Last 7 days", days: 7 },
] as const;

export type TimeframeId = (typeof TIMEFRAMES)[number]["id"];

export function timeframeDays(id: TimeframeId): number | null {
  return TIMEFRAMES.find((t) => t.id === id)?.days ?? null;
}

/**
 * Filter points to a timeframe. `nowMs` is injected rather than read from the
 * clock so this stays pure and a test can pin it (`.claude/rules/testing.md`).
 */
export function withinTimeframe(
  points: PerformancePoint[],
  id: TimeframeId,
  nowMs: number,
): PerformancePoint[] {
  const days = timeframeDays(id);
  if (days === null) return points;
  const floor = nowMs - days * 86_400_000;
  return points.filter((p) => p.submittedMs >= floor);
}

// ------------------------------------------------------------- course table

export type CourseRow = {
  courseId: string;
  courseCode: string;
  courseName: string;
  assessmentCount: number;
  attemptsUsed: number;
  /** Best across the course's assessments; `null` when none is graded. */
  bestPercentage: Percentage;
  /** Distinct assessment types present, in a stable order. */
  assessmentTypes: string[];
  /**
   * Oldest-first percentages for the mini trendline. Under two points this is
   * empty and the cell renders a dash — a one-point "trend" is not a trend, and
   * a flat line drawn through a single reading implies a stability nobody
   * measured.
   */
  trend: number[];
};

/**
 * The per-course breakdown table, grouped from `GET /student/exams` and given
 * its trendline by `GET /student/exam-attempts`.
 *
 * Grouped from the exams list rather than the attempts list so a course with a
 * published assessment the student has never opened still gets a row — dropping
 * it would hide exactly the work that needs doing.
 */
export function courseRows(exams: StudentExam[], attempts: ExamAttemptRow[]): CourseRow[] {
  const byCourse = new Map<string, CourseRow>();

  for (const exam of exams) {
    const row = byCourse.get(exam.course_id) ?? {
      courseId: exam.course_id,
      courseCode: exam.course_code,
      courseName: exam.course_name,
      assessmentCount: 0,
      attemptsUsed: 0,
      bestPercentage: null,
      assessmentTypes: [],
      trend: [],
    };

    row.assessmentCount += 1;
    row.attemptsUsed += Math.max(0, exam.attempts_used);

    // `null` never wins the max: an ungraded assessment must not drag a real
    // best down, nor invent one where there is none.
    if (exam.best_percentage !== null && Number.isFinite(exam.best_percentage)) {
      row.bestPercentage =
        row.bestPercentage === null
          ? exam.best_percentage
          : Math.max(row.bestPercentage, exam.best_percentage);
    }

    if (!row.assessmentTypes.includes(exam.assessment_type)) {
      row.assessmentTypes.push(exam.assessment_type);
    }

    byCourse.set(exam.course_id, row);
  }

  // One pass over the attempts, bucketed by course, so the table is O(n+m)
  // rather than a scan of every attempt per row.
  const trends = new Map<string, PerformancePoint[]>();
  for (const point of performanceSeries(attempts).points) {
    const attempt = attempts.find((a) => a.id === point.attemptId);
    if (attempt === undefined) continue;
    const bucket = trends.get(attempt.course_id) ?? [];
    bucket.push(point);
    trends.set(attempt.course_id, bucket);
  }

  for (const [courseId, points] of trends) {
    const row = byCourse.get(courseId);
    if (row === undefined) continue;
    // `performanceSeries` already sorted oldest-first.
    row.trend = points.length >= 2 ? points.map((p) => p.percentage) : [];
  }

  return [...byCourse.values()].sort((a, b) => a.courseCode.localeCompare(b.courseCode));
}

// ------------------------------------------------------------ course progress

/**
 * The "course completion index" the brief asked for, under its honest name.
 *
 * It is `continue_learning.progress_percentage` — blocks with a completed
 * session over blocks in the course — and nothing more. `null` when the student
 * has never started a session, because an unstarted course has no measured
 * progress. It is NOT a composite index and the UI does not call it one.
 */
export function courseProgress(
  resume: { progress_percentage: number } | null,
): Percentage {
  if (resume === null) return null;
  if (!Number.isFinite(resume.progress_percentage)) return null;
  return Math.min(Math.max(resume.progress_percentage, 0), 100);
}

// -------------------------------------------------------------- attention feed

export type AttentionItem = {
  id: string;
  text: string;
  href: string;
  tone: "info" | "warn";
};

/**
 * What the header bell actually reports.
 *
 * There is no notifications endpoint, so rather than render a decorative bell
 * this derives its items from real state the payloads already carry. An empty
 * list is an explicit "nothing needs your attention", never a silent badge.
 */
export function attentionItems(input: {
  backlog: RevisionBacklog;
  speaking: SpeakingTime;
  progress: AssessmentProgress;
  ungradedAttempts: number;
}): AttentionItem[] {
  const items: AttentionItem[] = [];

  if (input.speaking.isLocked) {
    items.push({
      id: "quota",
      text: "Today’s 20 minutes of speaking time are used. Voice resumes after midnight in your timezone.",
      href: "/exams",
      tone: "warn",
    });
  }

  if (input.backlog.flashcardsDue > 0) {
    items.push({
      id: "flashcards",
      text: `${input.backlog.flashcardsDue} flashcard${input.backlog.flashcardsDue === 1 ? "" : "s"} due for review`,
      href: "/dashboard#revision",
      tone: "info",
    });
  }

  if (input.backlog.revisionsDue > 0) {
    items.push({
      id: "revisions",
      text: `${input.backlog.revisionsDue} saved note${input.backlog.revisionsDue === 1 ? "" : "s"} due for revision`,
      href: "/dashboard#revision",
      tone: "info",
    });
  }

  const notAttempted = input.progress.total - input.progress.attempted;
  if (notAttempted > 0) {
    items.push({
      id: "assessments",
      text: `${notAttempted} assessment${notAttempted === 1 ? "" : "s"} not attempted yet`,
      href: "/exams",
      tone: "info",
    });
  }

  if (input.ungradedAttempts > 0) {
    items.push({
      id: "ungraded",
      text: `${input.ungradedAttempts} attempt${input.ungradedAttempts === 1 ? "" : "s"} not marked yet`,
      href: "/exams",
      tone: "info",
    });
  }

  return items;
}

// ------------------------------------------------------------------ formats

/** `72%`. Input is 0..100. A non-measurement is an em-dash, never `0%`. */
export function formatPct(value: Percentage, digits = 0): string {
  if (value === null || !Number.isFinite(value)) return "—";
  return `${value.toFixed(digits)}%`;
}

/** `13 Sep` — compact enough for an axis tick. */
export function formatShortDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";
  return date.toLocaleDateString(undefined, { day: "numeric", month: "short" });
}

export function formatDateTime(iso: string | null): string {
  if (iso === null) return "—";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";
  return date.toLocaleString(undefined, {
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  });
}
