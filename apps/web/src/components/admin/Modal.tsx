"use client";

import { useEffect, useId, useRef, type ReactNode } from "react";

import { AdminButton } from "@/components/admin/controls";

/**
 * Built on the native `<dialog>` element, not a hand-rolled overlay.
 *
 * `showModal()` gives focus containment, the inert backdrop, and Escape-to-
 * close from the platform — three things a div-based modal has to reimplement
 * and usually gets wrong for keyboard and screen-reader users. The only thing
 * added here is closing on a backdrop click, which `<dialog>` does not do.
 */
export function Modal({
  open,
  title,
  description,
  onClose,
  children,
  footer,
}: {
  open: boolean;
  title: string;
  description?: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;

    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog
      ref={ref}
      aria-labelledby={titleId}
      // `cancel` covers Escape; `close` covers every other route out, so the
      // parent's `open` state can never drift from the element's own.
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      onClose={onClose}
      onClick={(event) => {
        // A click that lands on the dialog element itself is a backdrop click:
        // the content sits in a child, so anything inside stops here.
        if (event.target === ref.current) onClose();
      }}
      className="m-auto w-[min(38rem,calc(100vw-2rem))] rounded-md border border-lavender-200 bg-white p-0 text-gray-900 backdrop:bg-ink-950/50"
    >
      <div className="px-6 py-5">
        <h2 id={titleId} className="font-display text-lg font-semibold text-gray-900">
          {title}
        </h2>
        {description ? <p className="mt-1 text-sm text-gray-500">{description}</p> : null}
        <div className="mt-4">{children}</div>
        {footer ? <div className="mt-6 flex justify-end gap-2">{footer}</div> : null}
      </div>
    </dialog>
  );
}

/**
 * Confirmation for an action with a consequence the row itself does not show —
 * suspending a user also kills every one of their sessions, for instance. The
 * `consequence` line is mandatory for exactly that reason: a bare "Are you
 * sure?" tells the admin nothing they did not already know.
 */
export function ConfirmDialog({
  open,
  title,
  consequence,
  confirmLabel,
  pending = false,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title: string;
  consequence: string;
  confirmLabel: string;
  pending?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <Modal
      open={open}
      title={title}
      onClose={onCancel}
      footer={
        <>
          <AdminButton type="button" variant="outline-light" onClick={onCancel} disabled={pending}>
            Cancel
          </AdminButton>
          <AdminButton type="button" onClick={onConfirm} pending={pending}>
            {confirmLabel}
          </AdminButton>
        </>
      }
    >
      <p className="text-sm text-gray-500">{consequence}</p>
    </Modal>
  );
}
