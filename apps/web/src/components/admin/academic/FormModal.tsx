"use client";

import { type FormEvent, type ReactNode } from "react";

import { AdminButton } from "@/components/admin/controls";
import { Modal } from "@/components/admin/Modal";
import { Banner } from "@/components/admin/primitives";

/**
 * A create/edit form in a dialog.
 *
 * The action row lives inside the `<form>` rather than in `Modal`'s `footer`
 * slot so the submit button is a real submit — that is what makes Enter in any
 * field save the row, which is the whole point of a data-entry form an admin
 * uses dozens of times a sitting.
 *
 * Fields are passed in as children; everything generic (the error banner, the
 * pending state, Cancel) lives here so no screen re-invents it.
 */
export function FormModal({
  open,
  title,
  description,
  submitLabel,
  pending,
  error,
  onSubmit,
  onClose,
  children,
}: {
  open: boolean;
  title: string;
  description?: string;
  submitLabel: string;
  pending: boolean;
  error: string | null;
  onSubmit: () => void;
  onClose: () => void;
  children: ReactNode;
}) {
  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    onSubmit();
  }

  return (
    <Modal open={open} title={title} description={description} onClose={onClose}>
      <form onSubmit={handleSubmit} noValidate className="space-y-4">
        {error ? <Banner tone="danger">{error}</Banner> : null}
        {children}
        <div className="flex justify-end gap-2 pt-2">
          <AdminButton
            type="button"
            variant="outline-light"
            onClick={onClose}
            disabled={pending}
          >
            Cancel
          </AdminButton>
          <AdminButton type="submit" pending={pending} pendingLabel="Saving…">
            {submitLabel}
          </AdminButton>
        </div>
      </form>
    </Modal>
  );
}

/** The `active | inactive` picker every editable academic entity shares. */
export function StatusOptions() {
  return (
    <>
      <option value="active">Active</option>
      <option value="inactive">Inactive</option>
    </>
  );
}
