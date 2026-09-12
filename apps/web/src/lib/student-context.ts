// The student's academic context — `GET /api/v1/me/context`, the "Remember Me"
// payload (`IMPLEMENTATION_PLAN.md` §4.1 item 5, checklist C-15).
//
// These types mirror `crates/core/src/context.rs` field for field. Keep them
// in step with that file: the same shape is carried by the Phase 3 WebSocket
// `session_ready` message, so a drift here is a drift in two places.
//
// This is fetched from the *browser*, not from a Server Component, on purpose.
// Auth is an httpOnly cookie set by the gateway on its own host
// (`apps/gateway/src/auth/cookies.rs`), and the gateway's CORS layer names the
// web origin explicitly with `allow_credentials(true)`. A Server Component
// would have to re-forward that cookie by hand, which only works while the two
// halves share a host — true in dev, not a property to build on.

import { apiFetch } from "@/lib/api";

export type ProgramSummary = {
  id: string;
  code: string;
  name: string;
};

export type SemesterSummary = {
  id: string;
  semester_number: number;
  name: string;
};

export type BlockSummary = {
  id: string;
  course_id: string;
  block_no: number;
  title: string;
};

export type StudentContext = {
  program: ProgramSummary;
  semester: SemesterSummary;
  /** `null` until an admin has placed the student into a block. */
  current_block: BlockSummary | null;
  /** NN-2: the classroom greeting plays only while this is `true`. */
  is_first_login: boolean;
};

export function loadStudentContext(): Promise<StudentContext> {
  return apiFetch<StudentContext>("/me/context");
}

/** An active login of this account — `GET /api/v1/auth/sessions`. */
export type AuthSessionSummary = {
  id: string;
  device_info: string | null;
  ip_address: string | null;
  created_at: string;
  expires_at: string;
};

export function loadAuthSessions(): Promise<AuthSessionSummary[]> {
  return apiFetch<AuthSessionSummary[]>("/auth/sessions");
}

export function revokeAuthSession(id: string): Promise<void> {
  return apiFetch<void>(`/auth/sessions/${id}`, { method: "DELETE" });
}

export function logout(): Promise<void> {
  return apiFetch<void>("/auth/logout", { method: "POST" });
}
