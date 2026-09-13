// Where each dashboard number goes when you click it.
//
// Every destination here is a real filtered list route — the drill-down
// vocabulary frozen in `.claude/rules/api-conventions.md`
// (`### Drill-down list endpoints`). A metric with no list that answers it
// (a sum of pages, an average, a percentile, a ratio) deliberately has no
// entry and stays plain text: a link that lands on the wrong records is worse
// than no link.

/** Query-string builder that drops empty values and encodes the rest. */
function href(path: string, params?: Record<string, string | undefined>): string {
  if (!params) return path;
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value) query.set(key, value);
  }
  const search = query.toString();
  return search ? `${path}?${search}` : path;
}

export type StudentStatus = "active" | "inactive" | "suspended";
export type EnrollmentStatus = "active" | "completed" | "dropped";
export type SessionStatus = "in_progress" | "completed" | "abandoned";
export type EndReason = "quota" | "idle" | "user" | "jailbreak" | "error";
export type IncidentKind = "jailbreak" | "toxicity" | "out_of_scope";

export const toPrograms = (): string => "/admin/programs";
export const toLscs = (): string => "/admin/lscs";

export const toStudents = (status?: StudentStatus): string =>
  href("/admin/students", status ? { status } : undefined);

export const toEnrollments = (status?: EnrollmentStatus): string =>
  href("/admin/enrollments", status ? { status } : undefined);

export const toSessions = (params?: {
  status?: SessionStatus;
  end_reason?: EndReason;
  from?: string;
}): string => href("/admin/sessions", params);

export const toBoardEvents = (params?: { violations_only?: "true"; bucket?: string }): string =>
  href("/admin/board-events", params);

export const toIncidents = (params?: {
  tier?: "0" | "1" | "2";
  kind?: IncidentKind;
  from?: string;
}): string => href("/admin/safety-incidents", params);

/**
 * The RFC3339 instant `days` ago, for the `from` filter that both the sessions
 * and the safety-incident lists accept. This is what makes a "last 7 days"
 * tile openable at all — there is no relative-window parameter.
 */
export function since(days: number): string {
  return new Date(Date.now() - days * 24 * 60 * 60 * 1000).toISOString();
}

/** `12` -> `"12 "`, `0` -> `"no "` — so an aria-label reads as a sentence. */
export function countWords(value: number): string {
  return value === 0 ? "no " : `${value.toLocaleString()} `;
}
