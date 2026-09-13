// The student self-service API boundary — `/api/v1/student/*`.
//
// Every shape here is parsed with Zod rather than cast, which is the rule in
// `.claude/rules/code-style.md` ("Zod schemas for every API boundary"). It
// earns its keep on these routes in particular: the gateway half is built
// alongside this one, so a field that is missing or a `score` that arrives as
// a string is a real possibility, and a silent `as` cast would turn it into a
// render-time crash three components deep instead of one honest error state.
//
// Fetched from the browser, not a Server Component, for the reason documented
// at the top of `lib/student-context.ts`: auth is an httpOnly cookie set by the
// gateway on its own host, and a Server Component would have to re-forward it
// by hand.

import { z } from "zod";

import { apiFetch, apiFetchPage, query } from "@/lib/api";

const uuid = z.string().min(1);
const rfc3339 = z.string().min(1);

/** AMENDMENT A1.1. Kept as a closed union so a new server value fails loudly
 *  at the boundary rather than rendering as a raw snake_case token. */
export const assessmentTypeSchema = z.enum([
  "assignment",
  "mid_term_quiz",
  "semester_exam",
]);
export const difficultyLevelSchema = z.enum(["beginner", "intermediate", "advanced"]);

export type AssessmentType = z.infer<typeof assessmentTypeSchema>;
export type DifficultyLevel = z.infer<typeof difficultyLevelSchema>;

/** Human labels for the wire enums. Never show the token itself. */
export const ASSESSMENT_TYPE_LABEL: Record<AssessmentType, string> = {
  assignment: "Assignment",
  mid_term_quiz: "Mid-term quiz",
  semester_exam: "Semester exam",
};

export const DIFFICULTY_LABEL: Record<DifficultyLevel, string> = {
  beginner: "Beginner",
  intermediate: "Intermediate",
  advanced: "Advanced",
};

/** A/B/C/D. Papers are exactly four options after AMENDMENT A1.1. */
export const OPTION_LETTERS = ["A", "B", "C", "D"] as const;

// ---------------------------------------------------------------------------
// GET /student/dashboard — one request hydrates all six dashboard components.
// ---------------------------------------------------------------------------

export const studentInfoSchema = z.object({
  name: z.string(),
  roll_number: z.string(),
  program: z.string(),
  program_id: uuid,
  semester: z.number().int(),
  avatar_url: z.string().nullable(),
});

export const continueLearningSchema = z.object({
  session_id: uuid,
  course_id: uuid,
  course_title: z.string(),
  block_id: uuid,
  block_no: z.number().int(),
  block_title: z.string(),
  chapter_name: z.string().nullable(),
  page_number: z.number().int(),
  // Contract deviation 2: no audio offset exists, and `para_index` is only
  // present when the Redis session state carried one.
  para_index: z.number().int().nullable(),
  progress_percentage: z.number(),
  resume_summary: z.string().nullable(),
  last_active_at: rfc3339,
});

export const quotaStatusSchema = z.object({
  minutes_used: z.number(),
  minutes_max: z.number(),
  ms_used: z.number().int(),
  ms_remaining: z.number().int(),
  is_locked: z.boolean(),
  // NN-3: the next midnight in the STUDENT's own timezone, resolved server
  // side. The client only formats it — it never derives the boundary itself.
  resets_at: rfc3339,
  timezone: z.string(),
});

export const examHistoryEntrySchema = z.object({
  attempt_id: uuid,
  exam_id: uuid,
  exam_title: z.string(),
  score: z.number().nullable(),
  max_score: z.number(),
  percentage: z.number().nullable(),
  status: z.string(),
  submitted_at: rfc3339.nullable(),
  weak_topics: z.array(z.string()),
  // Optional, not required: the dashboard payload in the contract predates
  // AMENDMENT 1 and does not promise it. Rendered when the gateway sends it.
  assessment_type: assessmentTypeSchema.optional(),
});

export const savedResourcesSchema = z.object({
  preserved_notes_count: z.number().int(),
  flashcards_total: z.number().int(),
  flashcards_due_for_review: z.number().int(),
  revisions_due_count: z.number().int(),
});

export const studentDashboardSchema = z.object({
  student_info: studentInfoSchema,
  continue_learning: continueLearningSchema.nullable(),
  quota_status: quotaStatusSchema,
  exam_history: z.array(examHistoryEntrySchema),
  saved_resources: savedResourcesSchema,
});

export type StudentInfo = z.infer<typeof studentInfoSchema>;
export type ContinueLearning = z.infer<typeof continueLearningSchema>;
export type QuotaStatus = z.infer<typeof quotaStatusSchema>;
export type ExamHistoryEntry = z.infer<typeof examHistoryEntrySchema>;
export type SavedResources = z.infer<typeof savedResourcesSchema>;
export type StudentDashboard = z.infer<typeof studentDashboardSchema>;

// ---------------------------------------------------------------------------
// Exams
// ---------------------------------------------------------------------------

export const studentExamSchema = z.object({
  id: uuid,
  title: z.string(),
  description: z.string().nullable(),
  block_id: uuid,
  block_no: z.number().int(),
  block_title: z.string(),
  course_id: uuid,
  course_code: z.string(),
  course_name: z.string(),
  assessment_type: assessmentTypeSchema,
  question_count: z.number().int().nullable(),
  duration_minutes: z.number().int().nullable(),
  max_score: z.number(),
  attempts_used: z.number().int(),
  best_percentage: z.number().nullable(),
});

/** One question as the student sees it: deliberately answer-free. */
export const examQuestionSchema = z.object({
  question_seq: z.number().int(),
  question_id: uuid,
  topic: z.string(),
  question_text: z.string(),
  options: z.array(z.string()),
  selected_option_index: z.number().int().nullable(),
});

export const examPaperSchema = z.object({
  attempt_id: uuid,
  exam_title: z.string(),
  assessment_type: assessmentTypeSchema,
  // AMENDMENT A1.4 made `time_limit_minutes` the countdown source. The older
  // `duration_minutes` is accepted as a fallback so the runner still shows a
  // clock against a gateway build that has not renamed it yet; neither is
  // authoritative — the server decides when an attempt has expired.
  time_limit_minutes: z.number().int().nullable().optional(),
  duration_minutes: z.number().int().nullable().optional(),
  started_at: rfc3339,
  total_questions: z.number().int(),
  questions: z.array(examQuestionSchema),
});

export const reviewedQuestionSchema = z.object({
  question_seq: z.number().int(),
  question_text: z.string(),
  options: z.array(z.string()),
  selected_option_index: z.number().int().nullable(),
  // Disclosed only by the submit response (contract deviation 5). Nothing in
  // `examPaperSchema` carries it, so an in-progress paper cannot leak it even
  // if a future gateway build started sending it.
  correct_option_index: z.number().int(),
  is_correct: z.boolean(),
  explanation: z.string().nullable(),
  topic: z.string(),
});

export const examResultSchema = z.object({
  attempt_id: uuid,
  exam_id: uuid,
  exam_title: z.string(),
  total_questions: z.number().int(),
  correct_answers: z.number().int(),
  score: z.number(),
  max_score: z.number(),
  score_percentage: z.number(),
  weak_topics: z.array(z.string()),
  submitted_at: rfc3339,
  review: z.array(reviewedQuestionSchema),
});

export type StudentExam = z.infer<typeof studentExamSchema>;
export type ExamQuestion = z.infer<typeof examQuestionSchema>;
export type ExamPaper = z.infer<typeof examPaperSchema>;
export type ReviewedQuestion = z.infer<typeof reviewedQuestionSchema>;
export type ExamResult = z.infer<typeof examResultSchema>;

// ---------------------------------------------------------------------------
// Flashcards and revisions
// ---------------------------------------------------------------------------

export const flashcardSchema = z.object({
  id: uuid,
  block_id: uuid,
  block_no: z.number().int().nullable(),
  block_title: z.string().nullable(),
  front: z.string(),
  back: z.string(),
  topic: z.string().nullable(),
  due_on: z.string(),
  interval_days: z.number().int(),
  ease: z.number().int(),
  reviewed_at: rfc3339.nullable(),
  created_at: rfc3339,
});

export const revisionSchema = z.object({
  id: uuid,
  session_id: uuid.nullable(),
  block_id: uuid.nullable(),
  block_no: z.number().int().nullable(),
  block_title: z.string().nullable(),
  topic: z.string().nullable(),
  remind_at: rfc3339,
  created_at: rfc3339,
});

export type Flashcard = z.infer<typeof flashcardSchema>;
export type Revision = z.infer<typeof revisionSchema>;

// ---------------------------------------------------------------------------
// Loaders
// ---------------------------------------------------------------------------

/**
 * Parse a gateway body, converting a schema mismatch into the same `ApiError`
 * shape a transport failure produces.
 *
 * The pages render one error state, so a contract drift has to arrive through
 * the same channel as a 503 rather than as a thrown `ZodError` that React
 * would surface as an unhandled exception. The Zod message is kept out of the
 * user-facing text on purpose — it names internal field paths.
 */
function parse<T>(schema: z.ZodType<T>, body: unknown, what: string): T {
  const result = schema.safeParse(body);
  if (result.success) return result.data;

  // Logged, not shown: the detail is for whoever is debugging the two halves.
  console.error(`Unexpected ${what} payload from the gateway`, result.error.issues);
  throw new SchemaError(what);
}

/**
 * A response the gateway sent but this build cannot read. Separate from
 * `ApiError` so a page can say "we could not read the answer" rather than
 * blaming the student's connection.
 */
export class SchemaError extends Error {
  constructor(readonly what: string) {
    super(`The server sent a ${what} we could not read. This is a bug on our side.`);
    this.name = "SchemaError";
  }
}

export async function loadStudentDashboard(): Promise<StudentDashboard> {
  return parse(studentDashboardSchema, await apiFetch<unknown>("/student/dashboard"), "dashboard");
}

export async function loadStudentExams(
  params: { limit?: number; offset?: number } = {},
): Promise<{ items: StudentExam[]; total: number }> {
  const page = await apiFetchPage<unknown>(
    `/student/exams${query({ limit: params.limit ?? 50, offset: params.offset })}`,
  );
  return { items: parse(z.array(studentExamSchema), page.items, "exam list"), total: page.total };
}

export async function loadFlashcards(
  params: { due_only?: boolean; limit?: number } = {},
): Promise<{ items: Flashcard[]; total: number }> {
  const page = await apiFetchPage<unknown>(
    `/student/flashcards${query({
      due_only: params.due_only === undefined ? undefined : String(params.due_only),
      limit: params.limit ?? 50,
    })}`,
  );
  return { items: parse(z.array(flashcardSchema), page.items, "flashcard list"), total: page.total };
}

export async function reviewFlashcard(id: string, recalled: boolean): Promise<void> {
  await apiFetch<unknown>(`/student/flashcards/${id}/review`, {
    method: "POST",
    body: JSON.stringify({ recalled }),
  });
}

export async function loadRevisions(
  params: { limit?: number } = {},
): Promise<{ items: Revision[]; total: number }> {
  const page = await apiFetchPage<unknown>(
    `/student/revisions${query({ limit: params.limit ?? 50 })}`,
  );
  return { items: parse(z.array(revisionSchema), page.items, "revision list"), total: page.total };
}

/**
 * Start an attempt. A `409 EXAM_ATTEMPT_ACTIVE` is not an error to show: the
 * contract says the conflict body names the live attempt, so the runner
 * resumes that one instead. The code is surfaced to the caller so it can.
 */
export async function startExamAttempt(examId: string): Promise<ExamPaper> {
  return parse(
    examPaperSchema,
    await apiFetch<unknown>(`/student/exams/${examId}/attempts`, { method: "POST" }),
    "exam paper",
  );
}

/** A card from `GET /student/exam-attempts` (`.claude/rules/api-conventions.md`). */
export const examAttemptCardSchema = z.object({
  id: uuid,
  exam_id: uuid,
  exam_title: z.string(),
  attempt_no: z.number().int(),
  score: z.number().nullable(),
  max_score: z.number(),
  status: z.string(),
  started_at: rfc3339,
  submitted_at: rfc3339.nullable(),
});

export type ExamAttemptCard = z.infer<typeof examAttemptCardSchema>;

/**
 * Find the caller's live attempt at one exam.
 *
 * Needed because a `409 EXAM_ATTEMPT_ACTIVE` cannot actually carry the active
 * attempt's id: the error envelope is `{ code, message }` and nothing else
 * (`.claude/rules/api-conventions.md` — "no details"). So the conflict tells
 * the runner *that* there is an attempt, and this reads the already-shipped
 * attempts list to find out *which*. Returns `null` rather than throwing, so a
 * resume that cannot be located degrades to the normal error state.
 */
export async function findActiveAttemptId(examId: string): Promise<string | null> {
  try {
    const page = await apiFetchPage<unknown>("/student/exam-attempts?limit=200");
    const cards = parse(z.array(examAttemptCardSchema), page.items, "attempt list");
    return cards.find((c) => c.exam_id === examId && c.status === "in_progress")?.id ?? null;
  } catch {
    return null;
  }
}

export async function loadExamAttempt(attemptId: string): Promise<ExamPaper> {
  return parse(
    examPaperSchema,
    await apiFetch<unknown>(`/student/exams/attempts/${attemptId}`),
    "exam paper",
  );
}

/** Save-as-you-go. Idempotent, so a retry after a blip is safe. */
export async function saveExamAnswers(
  attemptId: string,
  answers: { question_seq: number; selected_option_index: number }[],
): Promise<void> {
  await apiFetch<unknown>(`/student/exams/attempts/${attemptId}/answers`, {
    method: "PATCH",
    body: JSON.stringify({ answers }),
  });
}

export async function submitExamAttempt(attemptId: string): Promise<ExamResult> {
  return parse(
    examResultSchema,
    await apiFetch<unknown>(`/student/exams/attempts/${attemptId}/submit`, { method: "POST" }),
    "exam result",
  );
}
