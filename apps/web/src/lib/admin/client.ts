// Every admin call the console makes, in one place.
//
// Nothing here decides authorization — the gateway does, per handler
// (`.claude/rules/security.md`). These functions exist so no screen builds a
// URL by hand and so the whole console shares one transcription of the
// contract in `.claude/rules/api-conventions.md`.

import { apiFetch, apiFetchPage, query } from "@/lib/api";
import type { ScopeSummary } from "@/lib/me";
import type {
  AdminDocument,
  AdminStats,
  AdminUser,
  AssessmentType,
  AttemptStatus,
  Block,
  BoardEvent,
  BoardLatencyBucket,
  Course,
  DifficultyLevel,
  Enrollment,
  EnrollmentStatus,
  EntityStatus,
  IncidentKind,
  LearningSession,
  Lsc,
  Page,
  PoolQuestion,
  Program,
  QuestionPoolCount,
  QuestionStatus,
  Role,
  SafetyIncident,
  Semester,
  SessionEndReason,
  SessionStatus,
  Student,
  StudentReport,
  StudentPerformance,
  TopPerformer,
  StudentReportRow,
  UserStatus,
} from "@/lib/admin/types";

/** Filters shared by every paginated list. `q` is a server-side search. */
export type ListParams = {
  q?: string;
  limit?: number;
  offset?: number;
};

function json(body: unknown): RequestInit {
  return { body: JSON.stringify(body) };
}

// ---------------------------------------------------------------- programs

export function listPrograms(params: ListParams = {}): Promise<Page<Program>> {
  return apiFetchPage<Program>(`/admin/programs${query({ ...params })}`);
}

export function getProgram(id: string): Promise<Program> {
  return apiFetch<Program>(`/admin/programs/${id}`);
}

export function createProgram(input: {
  code: string;
  name: string;
  description?: string;
}): Promise<Program> {
  return apiFetch<Program>("/admin/programs", { method: "POST", ...json(input) });
}

export function updateProgram(
  id: string,
  input: { name: string; description?: string; status: EntityStatus },
): Promise<void> {
  return apiFetch<void>(`/admin/programs/${id}`, { method: "PATCH", ...json(input) });
}

// --------------------------------------------------------------- semesters

export function listSemesters(programId: string): Promise<Semester[]> {
  return apiFetch<Semester[]>(`/admin/programs/${programId}/semesters`);
}

export function getSemester(id: string): Promise<Semester> {
  return apiFetch<Semester>(`/admin/semesters/${id}`);
}

export function createSemester(input: {
  program_id: string;
  semester_number: number;
  name: string;
}): Promise<Semester> {
  return apiFetch<Semester>("/admin/semesters", { method: "POST", ...json(input) });
}

export function updateSemester(
  id: string,
  input: { name: string; status: EntityStatus },
): Promise<void> {
  return apiFetch<void>(`/admin/semesters/${id}`, { method: "PATCH", ...json(input) });
}

// ----------------------------------------------------------------- courses

export function listCourses(semesterId: string): Promise<Course[]> {
  return apiFetch<Course[]>(`/admin/semesters/${semesterId}/courses`);
}

/**
 * Every course in a program, across all of its semesters, ordered by semester
 * then course code.
 *
 * The semester level still exists in the schema and in the RAG metadata
 * filter — this endpoint flattens it for the console's drill-down only, so an
 * admin reaches a course in one hop instead of two.
 */
export function listCoursesForProgram(
  programId: string,
  params: ListParams = {},
): Promise<Page<Course>> {
  return apiFetchPage<Course>(`/admin/programs/${programId}/courses${query({ ...params })}`);
}

export function getCourse(id: string): Promise<Course> {
  return apiFetch<Course>(`/admin/courses/${id}`);
}

export function createCourse(input: {
  program_id: string;
  semester_id: string;
  code: string;
  name: string;
  description?: string;
}): Promise<Course> {
  return apiFetch<Course>("/admin/courses", { method: "POST", ...json(input) });
}

export function updateCourse(
  id: string,
  input: { name: string; description?: string },
): Promise<void> {
  return apiFetch<void>(`/admin/courses/${id}`, { method: "PATCH", ...json(input) });
}

// ------------------------------------------------------------------ blocks

export function listBlocks(courseId: string): Promise<Block[]> {
  return apiFetch<Block[]>(`/admin/courses/${courseId}/blocks`);
}

export function getBlock(id: string): Promise<Block> {
  return apiFetch<Block>(`/admin/blocks/${id}`);
}

export function createBlock(input: {
  course_id: string;
  block_no: number;
  title: string;
  description?: string;
}): Promise<Block> {
  return apiFetch<Block>("/admin/blocks", { method: "POST", ...json(input) });
}

export function updateBlock(
  id: string,
  input: { title?: string; description?: string; status?: EntityStatus },
): Promise<void> {
  return apiFetch<void>(`/admin/blocks/${id}`, { method: "PATCH", ...json(input) });
}

// -------------------------------------------------------------------- lscs

export function listLscs(params: ListParams = {}): Promise<Page<Lsc>> {
  return apiFetchPage<Lsc>(`/admin/lscs${query({ ...params })}`);
}

export function createLsc(input: {
  code: string;
  name: string;
  location?: string;
}): Promise<Lsc> {
  return apiFetch<Lsc>("/admin/lscs", { method: "POST", ...json(input) });
}

export function updateLsc(
  id: string,
  input: { name: string; location?: string; status: EntityStatus },
): Promise<void> {
  return apiFetch<void>(`/admin/lscs/${id}`, { method: "PATCH", ...json(input) });
}

// ---------------------------------------------------------------- students

export function listStudents(
  params: ListParams & { status?: string } = {},
): Promise<Page<Student>> {
  return apiFetchPage<Student>(`/admin/students${query({ ...params })}`);
}

export function getStudent(id: string): Promise<Student> {
  return apiFetch<Student>(`/admin/students/${id}`);
}

/** All three ids are required together — the gateway revalidates the triple. */
export function updateStudentAcademic(
  id: string,
  input: { program_id: string; semester_id: string; lsc_id: string },
): Promise<void> {
  return apiFetch<void>(`/admin/students/${id}/academic`, { method: "PATCH", ...json(input) });
}

export function setStudentBlock(id: string, blockId: string): Promise<void> {
  return apiFetch<void>(`/admin/students/${id}/current-block`, {
    method: "PATCH",
    ...json({ block_id: blockId }),
  });
}

// ------------------------------------------------------------------- users

export function listUsers(
  params: ListParams & { role?: Role; status?: UserStatus } = {},
): Promise<Page<AdminUser>> {
  return apiFetchPage<AdminUser>(`/admin/users${query({ ...params })}`);
}

export function createAdmin(input: {
  email: string;
  password: string;
  full_name: string;
  role: Exclude<Role, "student">;
  program_scopes: string[];
}): Promise<{ user_id: string }> {
  return apiFetch<{ user_id: string }>("/admin/users", { method: "POST", ...json(input) });
}

/**
 * Anything other than `active` also revokes every one of that user's auth
 * sessions server-side (`apps/gateway/src/admin/users.rs`) — the UI must say
 * so before confirming, because it logs the person out everywhere.
 */
export function setUserStatus(id: string, status: UserStatus): Promise<void> {
  return apiFetch<void>(`/admin/users/${id}/status`, { method: "PATCH", ...json({ status }) });
}

/**
 * Returns resolved programs, not bare ids — the gateway reuses the same
 * `ScopeSummary` shape `GET /me` returns, so there is one scope shape in the
 * API and no caller has to re-join against the programs list.
 */
export function listUserScopes(id: string): Promise<ScopeSummary[]> {
  return apiFetch<ScopeSummary[]>(`/admin/users/${id}/scopes`);
}

export function addUserScope(id: string, programId: string): Promise<void> {
  return apiFetch<void>(`/admin/users/${id}/scopes`, {
    method: "POST",
    ...json({ program_id: programId }),
  });
}

export function removeUserScope(id: string, programId: string): Promise<void> {
  return apiFetch<void>(`/admin/users/${id}/scopes/${programId}`, { method: "DELETE" });
}

// --------------------------------------------------------------- documents

export function listBlockDocuments(blockId: string): Promise<AdminDocument[]> {
  return apiFetch<AdminDocument[]>(`/admin/blocks/${blockId}/documents`);
}

export function getDocument(id: string): Promise<AdminDocument> {
  return apiFetch<AdminDocument>(`/admin/documents/${id}`);
}

/**
 * Set or clear the unit's introductory video.
 *
 * `null` clears it. The gateway normalises the link to a canonical watch URL
 * and rejects anything that is not a single YouTube video, so what comes back
 * is not necessarily the string that was sent — render the response, not the
 * input.
 */
export function setDocumentVideo(
  id: string,
  videoUrl: string | null,
): Promise<AdminDocument> {
  return apiFetch<AdminDocument>(`/admin/documents/${id}/video`, {
    method: "PATCH",
    ...json({ video_url: videoUrl }),
  });
}

/**
 * Upload is the one endpoint that is not JSON: the gateway takes raw PDF bytes
 * as the request body with the title in the query string, not multipart
 * (`apps/gateway/src/admin/documents.rs`). So it overrides `Content-Type`
 * rather than going through the JSON helpers above.
 */
export function uploadDocument(
  blockId: string,
  title: string,
  file: File,
): Promise<{ document_id: string; job_id: string }> {
  return apiFetch<{ document_id: string; job_id: string }>(
    `/admin/blocks/${blockId}/documents${query({ title })}`,
    { method: "POST", body: file, headers: { "Content-Type": "application/pdf" } },
  );
}

// ------------------------------------------------------------------- stats

export function loadStats(): Promise<AdminStats> {
  return apiFetch<AdminStats>("/admin/stats");
}

// -------------------------------------------------------------- drill-downs
//
// The record lists behind the dashboard's metrics. All four are paginated
// exactly like the lists above — bare array body, total in `X-Total-Count` —
// so they all go through `apiFetchPage`.

export function listEnrollments(
  params: ListParams & { status?: EnrollmentStatus; course_id?: string; student_id?: string } = {},
): Promise<Page<Enrollment>> {
  return apiFetchPage<Enrollment>(`/admin/enrollments${query({ ...params })}`);
}

export function listSessions(
  params: ListParams & {
    status?: SessionStatus;
    end_reason?: SessionEndReason;
    student_id?: string;
    block_id?: string;
    course_id?: string;
    from?: string;
    to?: string;
  } = {},
): Promise<Page<LearningSession>> {
  return apiFetchPage<LearningSession>(`/admin/sessions${query({ ...params })}`);
}

/**
 * Ordered `turn_seq` ascending when `session_id` is given and `emitted_at`
 * descending otherwise — one session reads as a transcript, the firehose reads
 * as a feed. The ordering is the gateway's; nothing is re-sorted here.
 */
export function listBoardEvents(
  params: ListParams & {
    session_id?: string;
    violations_only?: string;
    bucket?: BoardLatencyBucket;
  } = {},
): Promise<Page<BoardEvent>> {
  return apiFetchPage<BoardEvent>(`/admin/board-events${query({ ...params })}`);
}

export function listSafetyIncidents(
  params: ListParams & {
    tier?: string;
    kind?: IncidentKind;
    student_id?: string;
    session_id?: string;
    from?: string;
    to?: string;
  } = {},
): Promise<Page<SafetyIncident>> {
  return apiFetchPage<SafetyIncident>(`/admin/safety-incidents${query({ ...params })}`);
}

// ------------------------------------------------------------ question pool
//
// AMENDMENT A1.3. Two ingestion channels — one question at a time from the
// form, or a whole file in one atomic import. Both land on
// `/api/v1/admin/question-pool`; the version prefix is never dropped.

export function listPoolQuestions(
  params: ListParams & {
    program_id?: string;
    semester_id?: string;
    /** A2.1: the pool is per course, so this is the filter that matters most. */
    course_id?: string;
    block_id?: string;
    assessment_type?: AssessmentType;
    difficulty_level?: DifficultyLevel;
    status?: QuestionStatus;
  } = {},
): Promise<Page<PoolQuestion>> {
  return apiFetchPage<PoolQuestion>(`/admin/question-pool${query({ ...params })}`);
}

/**
 * Pool size per course for a whole program, in one request.
 *
 * Unpaginated by contract and includes empty courses, ordered by semester then
 * course code. This replaces a per-course `limit=1` count loop — N requests to
 * read N headers, which also could not report a course that had no rows.
 */
export function getQuestionPoolCounts(programId: string): Promise<QuestionPoolCount[]> {
  return apiFetch<QuestionPoolCount[]>(`/admin/programs/${programId}/question-pool-counts`);
}

export function createPoolQuestion(input: {
  /** A2.1: required. The pool belongs to the course. */
  course_id: string;
  /** A2.1: optional unit/module pointer. Must belong to `course_id` when sent. */
  block_id?: string;
  topic: string;
  question_text: string;
  options: string[];
  correct_option_index: number;
  explanation: string;
  assessment_type: AssessmentType;
  difficulty_level: DifficultyLevel;
}): Promise<PoolQuestion> {
  return apiFetch<PoolQuestion>("/admin/question-pool", { method: "POST", ...json(input) });
}

export function updatePoolQuestion(
  id: string,
  input: Partial<{
    /** `null` detaches the question from its unit/module (A2.1: optional). */
    block_id: string | null;
    topic: string;
    question_text: string;
    options: string[];
    correct_option_index: number;
    explanation: string;
    assessment_type: AssessmentType;
    difficulty_level: DifficultyLevel;
    status: QuestionStatus;
  }>,
): Promise<PoolQuestion> {
  return apiFetch<PoolQuestion>(`/admin/question-pool/${id}`, { method: "PATCH", ...json(input) });
}

/** A2.3: four accepted ingestion formats. */
export type BulkFormat = "json" | "csv" | "xlsx" | "docx";

const BULK_CONTENT_TYPE: Record<BulkFormat, string> = {
  json: "application/json",
  csv: "text/csv",
  xlsx: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
  docx: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
};

/**
 * Bulk import — ALL or NOTHING.
 *
 * The gateway validates every row before writing any row and rejects the whole
 * file with a per-row `VALIDATION_ERROR` message, because a half-imported pool
 * is worse than a rejected one (A1.3). So there is no partial-success shape to
 * model here: either a count comes back, or an `ApiError` does.
 *
 * `body` is a string for the text formats and a `File` for `.xlsx`/`.docx`,
 * which are ZIP containers and cannot survive being pasted into a textarea. The
 * declared format rides in the query string and the `Content-Type` is set for
 * completeness, but neither is authoritative: the gateway sniffs the magic bytes
 * (A2.3), so a mislabelled file is rejected by content rather than trusted.
 */
export function bulkImportPoolQuestions(
  body: string | File,
  format: BulkFormat,
): Promise<{ imported: number }> {
  return apiFetch<{ imported: number }>(`/admin/question-pool/bulk${query({ format })}`, {
    method: "POST",
    body,
    headers: { "Content-Type": BULK_CONTENT_TYPE[format] },
  });
}

// ---------------------------------------------------------- student reports
//
// AMENDMENT A2.4. Scoped exactly as `/admin/analytics`: the caller's role
// decides which query runs, and a sub-admin with no scopes gets an empty array
// rather than a platform-wide fallback. Nothing is re-filtered here.

export function listStudentReports(
  params: ListParams & {
    program_id?: string;
    semester_id?: string;
    course_id?: string;
    student_id?: string;
    exam_id?: string;
    assessment_type?: AssessmentType;
    status?: AttemptStatus;
    from?: string;
    to?: string;
  } = {},
): Promise<Page<StudentReportRow>> {
  return apiFetchPage<StudentReportRow>(`/admin/student-reports${query({ ...params })}`);
}

export function getStudentReport(studentId: string): Promise<StudentReport> {
  return apiFetch<StudentReport>(`/admin/students/${studentId}/report`);
}

// ------------------------------------------------------------- performance

export function getStudentPerformance(studentId: string): Promise<StudentPerformance> {
  return apiFetch<StudentPerformance>(`/admin/students/${studentId}/performance`);
}

/** The best-performers board. `min_attempts` defaults server-side to the same
 *  threshold a trophy needs, so the board and the trophy agree. */
export function listTopPerformers(
  params: { limit?: number; min_attempts?: number } = {},
): Promise<TopPerformer[]> {
  return apiFetch<TopPerformer[]>(`/admin/top-performers${query({ ...params })}`);
}
