"use client";

import { useCallback, useEffect, useState } from "react";

import { AdminButton, AdminSelect } from "@/components/admin/controls";
import { ConfirmDialog, Modal } from "@/components/admin/Modal";
import { Banner, EmptyState } from "@/components/admin/primitives";
import { addUserScope, listUserScopes, removeUserScope } from "@/lib/admin/client";
import type { AdminUser, Program } from "@/lib/admin/types";
import { ApiError } from "@/lib/api";
import type { ScopeSummary } from "@/lib/me";

/**
 * Which programs a sub-admin may touch.
 *
 * `GET /admin/users/{id}/scopes` resolves each scope to its program code and
 * name server-side — the same `ScopeSummary` shape `GET /me` returns — so
 * nothing here re-joins against the program catalogue. The catalogue is still
 * needed, but only to offer the programs that are *not* yet assigned.
 */
export function UserScopesModal({
  user,
  programs,
  onClose,
}: {
  user: AdminUser | null;
  programs: Program[];
  onClose: () => void;
}) {
  const [scopes, setScopes] = useState<ScopeSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [pendingAdd, setPendingAdd] = useState(false);
  const [selected, setSelected] = useState("");
  const [removing, setRemoving] = useState<string | null>(null);
  const [removePending, setRemovePending] = useState(false);

  const userId = user?.id ?? null;

  const load = useCallback(async () => {
    if (!userId) return;
    setLoading(true);
    try {
      setScopes(await listUserScopes(userId));
      setError(null);
    } catch (caught: unknown) {
      setScopes([]);
      setError(caught instanceof ApiError ? caught.message : "Could not load scopes.");
    } finally {
      setLoading(false);
    }
  }, [userId]);

  useEffect(() => {
    void (async () => {
      setSelected("");
      setRemoving(null);
      await load();
    })();
  }, [load]);

  const assignable = programs.filter(
    (program) => !scopes.some((scope) => scope.program_id === program.id),
  );

  async function onAdd() {
    if (!userId || !selected) return;
    setPendingAdd(true);
    setError(null);
    try {
      await addUserScope(userId, selected);
      setSelected("");
      await load();
    } catch (caught: unknown) {
      setError(
        caught instanceof ApiError && caught.code === "INVALID_PROGRAM"
          ? "That program no longer exists."
          : caught instanceof ApiError
            ? caught.message
            : "Could not add the scope.",
      );
    } finally {
      setPendingAdd(false);
    }
  }

  async function onRemoveConfirmed() {
    if (!userId || !removing) return;
    setRemovePending(true);
    setError(null);
    try {
      await removeUserScope(userId, removing);
      setRemoving(null);
      await load();
    } catch (caught: unknown) {
      setError(caught instanceof ApiError ? caught.message : "Could not remove the scope.");
    } finally {
      setRemovePending(false);
    }
  }

  return (
    <>
      <Modal
        open={user !== null}
        title="Program scopes"
        description={user ? `Which programs ${user.email} may administer.` : undefined}
        onClose={onClose}
        footer={
          <AdminButton type="button" variant="outline-light" onClick={onClose}>
            Done
          </AdminButton>
        }
      >
        <div className="space-y-4">
          {error ? (
            <Banner tone="danger" onDismiss={() => setError(null)}>
              {error}
            </Banner>
          ) : null}

          {loading ? (
            <p role="status" className="text-sm text-gray-500">
              Loading scopes...
            </p>
          ) : scopes.length === 0 ? (
            <EmptyState
              title="No programs assigned"
              hint="This sub-admin currently sees no students, programs, or documents."
            />
          ) : (
            <ul className="divide-y divide-lavender-200 rounded-sm border border-lavender-200">
              {scopes.map((scope) => (
                <li
                  key={scope.program_id}
                  className="flex items-center justify-between gap-3 px-3 py-2"
                >
                  <span className="text-sm text-gray-900">{scope.name}</span>
                  <button
                    type="button"
                    onClick={() => setRemoving(scope.program_id)}
                    className="rounded-sm text-sm font-medium text-danger underline-offset-2 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-white"
                  >
                    Remove
                    <span className="sr-only"> {scope.name}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}

          <div className="flex flex-wrap items-end gap-2">
            <div className="min-w-[12rem] flex-1">
              <label htmlFor="add-scope" className="block text-sm font-medium text-gray-900">
                Add a program
              </label>
              <div className="mt-1">
                <AdminSelect
                  id="add-scope"
                  value={selected}
                  disabled={assignable.length === 0}
                  onChange={(event) => setSelected(event.target.value)}
                >
                  <option value="">
                    {assignable.length === 0 ? "Every program is assigned" : "Select a program..."}
                  </option>
                  {assignable.map((program) => (
                    <option key={program.id} value={program.id}>
                      {program.code} &mdash; {program.name}
                    </option>
                  ))}
                </AdminSelect>
              </div>
            </div>
            <AdminButton
              type="button"
              onClick={() => void onAdd()}
              disabled={!selected}
              pending={pendingAdd}
              pendingLabel="Adding..."
            >
              Add scope
            </AdminButton>
          </div>
        </div>
      </Modal>

      <ConfirmDialog
        open={removing !== null}
        title="Remove this program scope?"
        consequence={
          removing
            ? `${scopes.find((scope) => scope.program_id === removing)?.name ?? "That program"} will no longer be visible to this admin — they lose access to its students, content, and uploads immediately.`
            : ""
        }
        confirmLabel="Remove scope"
        pending={removePending}
        onConfirm={() => void onRemoveConfirmed()}
        onCancel={() => setRemoving(null)}
      />
    </>
  );
}
