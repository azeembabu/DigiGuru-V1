"use client";

import {
  useEffect,
  useId,
  useRef,
  useState,
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
  type TextareaHTMLAttributes,
} from "react";

import { adminControlBorder, adminControlClass } from "@/components/admin/primitives";
import { buttonClass } from "@/components/ui/Button";

// Light-surface form controls, search, and pagination for the admin console.
// The dark-surface equivalents in `components/ui/*` are the auth/marketing
// versions of the same roles — see the header comment in `primitives.tsx` for
// why they are not shared.

export function AdminField({
  id,
  label,
  error,
  hint,
  required = false,
  children,
}: {
  id: string;
  label: string;
  error?: string;
  hint?: string;
  required?: boolean;
  children: ReactNode;
}) {
  return (
    <div>
      <label htmlFor={id} className="block text-sm font-medium text-gray-900">
        {label}
        {required ? (
          <span className="ml-0.5 text-danger" aria-hidden="true">
            *
          </span>
        ) : null}
      </label>
      <div className="mt-1">{children}</div>
      {error ? (
        <p id={`${id}-error`} role="alert" className="mt-1 flex items-start gap-1.5 text-xs text-danger">
          <span aria-hidden="true">&#9888;</span>
          <span>{error}</span>
        </p>
      ) : hint ? (
        <p id={`${id}-hint`} className="mt-1 text-xs text-gray-500">
          {hint}
        </p>
      ) : null}
    </div>
  );
}

function describedBy(id: string, hasError: boolean, hasHint: boolean): string | undefined {
  if (hasError) return `${id}-error`;
  if (hasHint) return `${id}-hint`;
  return undefined;
}

export function AdminInput({
  id,
  hasError = false,
  hasHint = false,
  className = "",
  ...rest
}: InputHTMLAttributes<HTMLInputElement> & { id: string; hasError?: boolean; hasHint?: boolean }) {
  return (
    <input
      id={id}
      name={id}
      aria-invalid={hasError || undefined}
      aria-describedby={describedBy(id, hasError, hasHint)}
      className={`${adminControlClass} ${adminControlBorder(hasError)} ${className}`}
      {...rest}
    />
  );
}

export function AdminTextarea({
  id,
  hasError = false,
  hasHint = false,
  className = "",
  ...rest
}: TextareaHTMLAttributes<HTMLTextAreaElement> & {
  id: string;
  hasError?: boolean;
  hasHint?: boolean;
}) {
  return (
    <textarea
      id={id}
      name={id}
      rows={3}
      aria-invalid={hasError || undefined}
      aria-describedby={describedBy(id, hasError, hasHint)}
      className={`${adminControlClass} ${adminControlBorder(hasError)} resize-y ${className}`}
      {...rest}
    />
  );
}

/**
 * A native `<select>` on purpose.
 *
 * The dark auth route uses a custom listbox (`ui/SelectInput.tsx`) because it
 * had to be styled into the key art. The admin console has no such constraint,
 * and a data-dense CRUD form is exactly where the native control's keyboard
 * behaviour, type-ahead, and mobile picker are worth more than matching
 * chrome.
 */
export function AdminSelect({
  id,
  hasError = false,
  hasHint = false,
  className = "",
  children,
  ...rest
}: SelectHTMLAttributes<HTMLSelectElement> & {
  id: string;
  hasError?: boolean;
  hasHint?: boolean;
}) {
  return (
    <select
      id={id}
      name={id}
      aria-invalid={hasError || undefined}
      aria-describedby={describedBy(id, hasError, hasHint)}
      className={`${adminControlClass} ${adminControlBorder(hasError)} ${className}`}
      {...rest}
    >
      {children}
    </select>
  );
}

/** Primary/secondary action button on the light surface. */
export function AdminButton({
  variant = "primary-light",
  pending = false,
  pendingLabel = "Working…",
  children,
  className = "",
  ...rest
}: React.ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary-light" | "outline-light";
  pending?: boolean;
  pendingLabel?: string;
  children: ReactNode;
}) {
  return (
    <button
      disabled={pending || rest.disabled}
      aria-busy={pending || undefined}
      className={buttonClass(
        variant,
        `py-2 text-sm disabled:cursor-not-allowed disabled:opacity-60 ${className}`,
      )}
      {...rest}
    >
      {pending ? pendingLabel : children}
    </button>
  );
}

/**
 * Debounced search box.
 *
 * Search is a server round trip (`q=` on the admin list endpoints), so it is
 * debounced rather than fired per keystroke — 300 ms, matching the prototype's
 * students screen. The typed text is local state so it never lags behind the
 * keyboard while a request is in flight; `value` seeds it and nothing else.
 *
 * To reset the box when a screen clears its filters, remount it with a
 * changing `key` rather than pushing a new `value` down — mirroring the prop
 * back into state would cancel an in-flight debounce every time the parent
 * echoed the committed term.
 */
export function SearchInput({
  value,
  onChange,
  placeholder = "Search…",
  label = "Search",
}: {
  value: string;
  onChange: (next: string) => void;
  placeholder?: string;
  label?: string;
}) {
  const id = useId();
  const [draft, setDraft] = useState(value);
  // Held in refs, not deps: a parent that recreates `onChange` each render
  // would otherwise restart the timer on every render and the debounce would
  // never fire.
  const onChangeRef = useRef(onChange);
  const committedRef = useRef(value);

  useEffect(() => {
    onChangeRef.current = onChange;
  });

  useEffect(() => {
    if (draft === committedRef.current) return;

    const timer = setTimeout(() => {
      committedRef.current = draft;
      onChangeRef.current(draft);
    }, 300);

    return () => clearTimeout(timer);
  }, [draft]);

  return (
    <div className="min-w-[200px] flex-1">
      <label htmlFor={id} className="sr-only">
        {label}
      </label>
      <input
        id={id}
        type="search"
        value={draft}
        placeholder={placeholder}
        onChange={(event) => setDraft(event.target.value)}
        className={`${adminControlClass} ${adminControlBorder(false)}`}
      />
    </div>
  );
}

/**
 * Offset pagination against `X-Total-Count`.
 *
 * Prev/Next only — the gateway exposes `limit`/`offset` with no cursor, so a
 * numbered pager would promise stability across pages that the API cannot
 * actually give while rows are being created underneath it.
 */
export function Pagination({
  offset,
  limit,
  total,
  onOffsetChange,
}: {
  offset: number;
  limit: number;
  total: number;
  onOffsetChange: (next: number) => void;
}) {
  const first = total === 0 ? 0 : offset + 1;
  const last = Math.min(offset + limit, total);
  const canPrev = offset > 0;
  const canNext = offset + limit < total;

  return (
    <div className="flex items-center justify-between gap-4 text-sm text-gray-500">
      <p aria-live="polite">
        {total === 0 ? "No results" : `${first}–${last} of ${total}`}
      </p>
      <div className="flex gap-2">
        <AdminButton
          type="button"
          variant="outline-light"
          disabled={!canPrev}
          onClick={() => onOffsetChange(Math.max(0, offset - limit))}
        >
          Previous
        </AdminButton>
        <AdminButton
          type="button"
          variant="outline-light"
          disabled={!canNext}
          onClick={() => onOffsetChange(offset + limit)}
        >
          Next
        </AdminButton>
      </div>
    </div>
  );
}
