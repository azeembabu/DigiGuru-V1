/**
 * Permanent removal of academic content, and the preview that must precede it.
 *
 * The four levels share one shape on the wire, so the console shares one
 * dialogue for them (`RemoveDialog`) instead of four that could drift apart in
 * how loudly they warn.
 *
 * `documents` is the route segment for what the UI calls a **unit** — the
 * storage-side name for the uploaded PDF. Mapped here, once, rather than at
 * each call site.
 */

import { z } from "zod";

import { apiFetch } from "@/lib/api";

export type RemovableKind = "program" | "course" | "block" | "unit";

const SEGMENT: Record<RemovableKind, string> = {
  program: "programs",
  course: "courses",
  block: "blocks",
  unit: "documents",
};

export const deletionImpactSchema = z.object({
  semesters: z.number().int(),
  courses: z.number().int(),
  blocks: z.number().int(),
  units: z.number().int(),
  exams: z.number().int(),
  exam_attempts: z.number().int(),
  questions: z.number().int(),
  flashcards: z.number().int(),
  learning_sessions: z.number().int(),
  board_events: z.number().int(),
  enrollments: z.number().int(),
  students_affected: z.number().int(),
  /** `false` when the gateway would refuse — e.g. a programme with students. */
  can_delete: z.boolean(),
  blocked_reason: z.string().nullable(),
});

export type DeletionImpact = z.infer<typeof deletionImpactSchema>;

/** What removing this would destroy. Read before showing the confirm button. */
export async function loadDeletionImpact(
  kind: RemovableKind,
  id: string,
): Promise<DeletionImpact> {
  const body = await apiFetch<unknown>(`/admin/${SEGMENT[kind]}/${id}/deletion-impact`);
  return deletionImpactSchema.parse(body);
}

/** Permanent. There is no undo and no archive fallback on this path. */
export async function removeItem(kind: RemovableKind, id: string): Promise<void> {
  await apiFetch<unknown>(`/admin/${SEGMENT[kind]}/${id}`, { method: "DELETE" });
}
