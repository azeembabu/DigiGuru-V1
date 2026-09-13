"use client";

import Link from "next/link";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useCallback, useMemo, type ReactNode } from "react";

import { AdminSelect } from "@/components/admin/controls";
import { StatusPill } from "@/components/admin/primitives";

// Pieces shared by the four drill-down screens (sessions, enrolments, board
// events, safety incidents).
//
// What makes these screens different from the CRUD lists is that their filters
// live in the **URL**, not in component state: a dashboard metric deep-links
// straight into the filtered view (`/admin/sessions?end_reason=quota`), and the
// view an admin is looking at while diagnosing something has to be pasteable
// into a chat window. So the query string is the single source of truth here
// and `useState` is deliberately absent.

/** Page size for every drill-down list. Within the contract's 1-200 range. */
export const PAGE_SIZE = 25;

/**
 * Read and write the page's filters through the URL.
 *
 * `set` replaces rather than pushes: retyping a filter should not bury the
 * dashboard under a dozen history entries, while the current URL still stays
 * shareable. Any filter change clears `offset`, because page 3 of the old
 * filter is meaningless under the new one.
 */
export function useUrlFilters(): {
  get: (key: string) => string;
  offset: number;
  set: (patch: Record<string, string>) => void;
  setOffset: (next: number) => void;
  clearAll: () => void;
} {
  const params = useSearchParams();
  const router = useRouter();
  const pathname = usePathname();

  const get = useCallback((key: string) => params.get(key) ?? "", [params]);

  const offset = useMemo(() => {
    const parsed = Number.parseInt(params.get("offset") ?? "", 10);
    // A hand-edited or stale `offset` must not be forwarded to the gateway,
    // which validates rather than clamps and would answer 400.
    return Number.isFinite(parsed) && parsed > 0 ? parsed : 0;
  }, [params]);

  const write = useCallback(
    (patch: Record<string, string>) => {
      const next = new URLSearchParams(params.toString());
      for (const [key, value] of Object.entries(patch)) {
        if (value === "") next.delete(key);
        else next.set(key, value);
      }
      const qs = next.toString();
      router.replace(qs ? `${pathname}?${qs}` : pathname, { scroll: false });
    },
    [params, pathname, router],
  );

  const set = useCallback(
    (patch: Record<string, string>) => write({ ...patch, offset: "" }),
    [write],
  );

  const setOffset = useCallback(
    (next: number) => write({ offset: next > 0 ? String(next) : "" }),
    [write],
  );

  const clearAll = useCallback(
    () => router.replace(pathname, { scroll: false }),
    [pathname, router],
  );

  return { get, offset, set, setOffset, clearAll };
}

/** One labelled `<select>` bound to a query parameter. */
export function FilterSelect({
  id,
  label,
  value,
  options,
  anyLabel,
  onChange,
}: {
  id: string;
  label: string;
  value: string;
  options: { value: string; label: string }[];
  anyLabel: string;
  onChange: (next: string) => void;
}) {
  return (
    <div className="min-w-[170px]">
      <label htmlFor={id} className="sr-only">
        {label}
      </label>
      <AdminSelect id={id} value={value} onChange={(event) => onChange(event.target.value)}>
        <option value="">{anyLabel}</option>
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </AdminSelect>
    </div>
  );
}

/** Filter bar: wraps at ~400px instead of pushing the page sideways. */
export function FilterBar({ children }: { children: ReactNode }) {
  return <div className="flex flex-wrap items-center gap-3">{children}</div>;
}

/**
 * "Showing X · clear" line under the heading.
 *
 * The heading itself already says what is being shown in words; this is the
 * escape hatch back to the unfiltered list, which a deep-linked admin has no
 * other way to reach.
 */
export function ActiveFilters({
  labels,
  onClear,
}: {
  labels: string[];
  onClear: () => void;
}) {
  if (labels.length === 0) return null;

  return (
    <div className="flex flex-wrap items-center gap-2 text-sm text-gray-500">
      <span>Filtered by:</span>
      {labels.map((label) => (
        <span
          key={label}
          className="rounded-full bg-lavender-100 px-2.5 py-0.5 text-xs font-medium text-gray-900"
        >
          {label}
        </span>
      ))}
      <button
        type="button"
        onClick={onClear}
        className="rounded-sm text-indigo-500 underline-offset-2 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-lavender-50"
      >
        Clear all
      </button>
    </div>
  );
}

// ------------------------------------------------------------------ formats

/** NN-3 cap: 20 minutes of active voice, enforced server-side. */
export const QUOTA_CAP_MS = 1_200_000;

/** NN-1 SyncGate hold ceiling — an ACK above this is a whiteboard violation. */
export const HOLD_MAX_MS = 400;

/** `active_voice_ms` as `12m 04s` — raw milliseconds are unreadable at a glance. */
export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "—";
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
}

/** Date **and** time: these rows are diagnosed against each other minute by minute. */
export function formatDateTime(iso: string): string {
  const parsed = new Date(iso);
  if (Number.isNaN(parsed.getTime())) return iso;
  return parsed.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

/** Short id for a column that only has to be recognisable, not readable. */
export function shortId(id: string): string {
  return id.slice(0, 8);
}

// ------------------------------------------------------------------ display

export function Blank() {
  return <span className="text-gray-500">&mdash;</span>;
}

export function RowLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <Link
      href={href}
      className="font-medium text-indigo-500 underline-offset-2 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-lavender-50"
    >
      {children}
    </Link>
  );
}

/**
 * A count of NN-1 breaches, never rendered as a neutral number.
 *
 * Zero is a success pill, not muted text: on a table where most rows are fine,
 * an admin scanning for trouble needs the bad row to be the one that differs,
 * and "0" and "3" in the same grey read identically at a glance.
 */
export function ViolationCount({ count }: { count: number }) {
  if (count <= 0) return <StatusPill tone="success">none</StatusPill>;
  return (
    <StatusPill tone="danger">
      {count} violation{count === 1 ? "" : "s"}
    </StatusPill>
  );
}
