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
  Block,
  BoardEvent,
  BoardLatencyBucket,
  Course,
  EntityStatus,
  Enrollment,
  EnrollmentStatus,
  IncidentKind,
  LearningSession,
  Lsc,
  Page,
  Program,
  Role,
  SafetyIncident,
  Semester,
  SessionEndReason,
  SessionStatus,
  Student,
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
