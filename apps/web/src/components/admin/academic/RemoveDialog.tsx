"use client";

/**
 * The confirmation step in front of every permanent removal.
 *
 * Removing a programme, course or block is not a tidy-up: `blocks -> exams ->
 * exam_attempts` cascades, so it permanently destroys students' marks. The
 * database will not warn anybody and there is no undo, so the dialogue has two
 * jobs and both matter:
 *
 * 1. **Say what will be lost, in counts, before anything happens.** The figures
 *    come from the gateway's `deletion-impact` route rather than from anything
 *    the console already has on screen, because the screen only knows about the
 *    level it is showing — a block list has no idea how many exam attempts hang
 *    off each row.
 * 2. **Make the confirmation deliberate.** The admin types the item's own name
 *    back. A plain "Are you sure?" is clicked through reflexively; retyping
 *    "Block 3 — Prosody" is not something a hand does by accident.
 *
 * The preview is advisory, not a server-side lock — nothing stops a script
 * calling `DELETE` directly, and the gateway says as much. The real protections
 * are the capability check, the scope check and the audit row. This is the part
 * that protects a person from themselves.
 */

import { useEffect, useState } from "react";

import { ApiError } from "@/lib/api";
import {
  loadDeletionImpact,
  removeItem,
  type DeletionImpact,
  type RemovableKind,
} from "@/lib/admin-removal";

import { AdminButton } from "@/components/admin/controls";

/** Count rows worth showing, in the order an admin cares about them. */
function lines(impact: DeletionImpact): Array<{ label: string; n: number; grave?: boolean }> {
  return [
    { label: "semesters", n: impact.semesters },
    { label: "courses", n: impact.courses },
    { label: "blocks", n: impact.blocks },
    { label: "uploaded units (and their vectors)", n: impact.units },
    { label: "exams", n: impact.exams },
    // The one that is never coming back and that belongs to somebody else.
    { label: "students' exam attempts and marks", n: impact.exam_attempts, grave: true },
    { label: "pool questions", n: impact.questions },
    { label: "flashcards", n: impact.flashcards },
    { label: "tutoring sessions", n: impact.learning_sessions },
    { label: "whiteboard events", n: impact.board_events },
    { label: "course enrolments", n: impact.enrollments },
  ].filter((row) => row.n > 0);
}

export function RemoveDialog({
  kind,
  id,
  name,
  onCancel,
  onRemoved,
}: {
  kind: RemovableKind;
  id: string;
  /** Exactly what the admin must type back. */
  name: string;
  onCancel: () => void;
  onRemoved: (message: string) => void;
}) {
  const [impact, setImpact] = useState<DeletionImpact | null>(null);
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    loadDeletionImpact(kind, id)
      .then((data) => {
        if (active) setImpact(data);
      })
      .catch((caught: unknown) => {
        if (active) {
          setError(
            caught instanceof ApiError
              ? caught.message
              : "Could not work out what removing this would affect.",
          );
        }
      });
    return () => {
      active = false;
    };
  }, [kind, id]);

  // Trimmed, but case-sensitive: these names carry Malayalam and mixed scripts
  // where a case-insensitive compare is not meaningful, and the point is a
  // deliberate act rather than a spelling test.
  const confirmed = typed.trim() === name.trim();
  const rows = impact ? lines(impact) : [];
  const blocked = impact !== null && !impact.can_delete;

  async function remove() {
    setBusy(true);
    setError(null);
    try {
      await removeItem(kind, id);
      onRemoved(`“${name}” was permanently removed.`);
    } catch (caught: unknown) {
      setError(
        caught instanceof ApiError ? caught.message : "This could not be removed.",
      );
      setBusy(false);
    }
  }

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="remove-title"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
    >
      <div className="max-h-[85vh] w-full max-w-lg overflow-y-auto rounded-2xl border border-rose-500/30 bg-white p-6 shadow-2xl">
        <h2 id="remove-title" className="text-[18px] font-bold text-gray-900">
          Remove “{name}”?
        </h2>

        {impact === null && error === null ? (
          <p className="mt-3 text-[14px] text-gray-500">Working out what this would affect…</p>
        ) : null}

        {blocked ? (
          <div className="mt-3 rounded-lg border border-amber-300 bg-amber-50 p-3 text-[14px] text-amber-900">
            {impact?.blocked_reason}
          </div>
        ) : null}

        {impact !== null && !blocked ? (
          <>
            <p className="mt-3 text-[14px] leading-relaxed text-gray-700">
              This is permanent and cannot be undone.
              {rows.length > 0 ? " It will also delete:" : " Nothing else depends on it."}
            </p>

            {rows.length > 0 ? (
              <ul className="mt-3 space-y-1 rounded-lg border border-rose-200 bg-rose-50/60 p-3">
                {rows.map((row) => (
                  <li
                    key={row.label}
                    className={`text-[14px] ${
                      row.grave ? "font-semibold text-rose-800" : "text-gray-700"
                    }`}
                  >
                    <span className="tabular-nums">{row.n}</span> {row.label}
                  </li>
                ))}
              </ul>
            ) : null}

            {impact.students_affected > 0 ? (
              <p className="mt-2 text-[13px] font-semibold text-rose-800">
                {impact.students_affected} student
                {impact.students_affected === 1 ? "" : "s"} will lose work they cannot get back.
              </p>
            ) : null}

            <label className="mt-4 block text-[13px] font-medium text-gray-700">
              Type the name to confirm
              <input
                value={typed}
                onChange={(event) => setTyped(event.target.value)}
                autoComplete="off"
                spellCheck={false}
                placeholder={name}
                className="mt-1 w-full rounded-lg border border-gray-300 px-3 py-2 text-[14px] text-gray-900 outline-none focus:border-rose-500 focus:ring-2 focus:ring-rose-200"
              />
            </label>
          </>
        ) : null}

        {error !== null ? (
          <p className="mt-3 rounded-lg border border-rose-300 bg-rose-50 p-3 text-[14px] text-rose-800">
            {error}
          </p>
        ) : null}

        <div className="mt-5 flex justify-end gap-2">
          <AdminButton type="button" variant="outline-light" onClick={onCancel}>
            Cancel
          </AdminButton>
          <button
            type="button"
            disabled={!confirmed || busy || blocked || impact === null}
            onClick={remove}
            className="rounded-lg bg-rose-600 px-4 py-2 text-[14px] font-semibold text-white transition hover:bg-rose-500 disabled:cursor-not-allowed disabled:opacity-40"
          >
            {busy ? "Removing…" : "Remove permanently"}
          </button>
        </div>
      </div>
    </div>
  );
}
