// The caller's own identity — `GET /api/v1/me`.
//
// Distinct from `/me/context` (`lib/student-context.ts`), which is the
// student's *academic* context and 404s for an admin who has no `students`
// row. This endpoint answers "who am I and what may I touch" for every role,
// and it is the only way the admin console can know a sub-admin's program
// scopes: the access token is httpOnly by design (`.claude/rules/security.md`),
// so the browser cannot read the claims itself.

import { apiFetch } from "@/lib/api";

export type Role = "super_admin" | "sub_admin" | "student";

/** One program a sub-admin is scoped to, resolved to a name server-side. */
export type ScopeSummary = {
  program_id: string;
  code: string;
  name: string;
};

export type Me = {
  id: string;
  email: string;
  full_name: string | null;
  role: Role;
  status: string;
  /** Always `[]` for a super-admin (unrestricted) and for a student. */
  scopes: ScopeSummary[];
};

export function loadMe(): Promise<Me> {
  return apiFetch<Me>("/me");
}

export function isAdmin(role: Role): boolean {
  return role === "super_admin" || role === "sub_admin";
}

/**
 * Whether this actor may act on `program_id`.
 *
 * A convenience for hiding controls the gateway would reject anyway — it is
 * never the authorization itself. `Actor::require_scoped` on the gateway is
 * the only check that counts (`.claude/rules/security.md`: no implicit allow).
 */
export function inScope(me: Me, programId: string | null | undefined): boolean {
  if (me.role === "super_admin") return true;
  if (me.role !== "sub_admin" || !programId) return false;
  return me.scopes.some((scope) => scope.program_id === programId);
}
