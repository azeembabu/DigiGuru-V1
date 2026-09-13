// Academic reference data for the signup form.
//
// Signup must submit real `program_id` / `semester_id` / `lsc_id` UUIDs —
// `IMPLEMENTATION_PLAN.md` §4.1 item 3 is explicit that these "must resolve to
// existing rows, not free text", and `apps/gateway/src/auth/signup.rs` types
// them as `Uuid`. So the form has to read the live lists rather than hardcode
// options.
//
// GAP: the only list endpoints that exist today are under `/api/v1/admin/*`
// (`apps/gateway/src/admin/mod.rs`), and every one of them sits behind a
// capability check. An anonymous visitor on /signup cannot call them. The
// unauthenticated read-only endpoints below are therefore NOT implemented yet
// on the gateway; this module is written against the contract they need, and
// `loadAcademicReference` degrades to an inline notice until they land. The
// backend half of the repo owns that change (`CLAUDE.md`, machine ownership).

import { apiFetch } from "@/lib/api";

export type ReferenceOption = {
  id: string;
  name: string;
};

type ProgramRow = { id: string; name: string; code?: string };
type SemesterRow = { id: string; name: string; semester_number?: number };
type LscRow = { id: string; name: string; code?: string };

export type AcademicReference = {
  programs: ReferenceOption[];
  lscs: ReferenceOption[];
};

function toOption(row: { id: string; name: string }): ReferenceOption {
  return { id: row.id, name: row.name };
}

/** Programs and LSCs — both are needed before the form can be submitted. */
export async function loadAcademicReference(): Promise<AcademicReference> {
  const [programs, lscs] = await Promise.all([
    apiFetch<ProgramRow[]>("/reference/programs"),
    apiFetch<LscRow[]>("/reference/lscs"),
  ]);

  return {
    programs: programs.map(toOption),
    lscs: lscs.map(toOption),
  };
}

/**
 * Semesters belong to a program (`Program > Semester > Course > Block`), so
 * this is fetched only once a program is chosen — mirroring the gateway's own
 * `/admin/programs/{program_id}/semesters` shape.
 */
export async function loadSemesters(programId: string): Promise<ReferenceOption[]> {
  const rows = await apiFetch<SemesterRow[]>(`/reference/programs/${programId}/semesters`);
  return rows.map((row) => ({
    id: row.id,
    name: row.name || `Semester ${row.semester_number ?? ""}`.trim(),
  }));
}
