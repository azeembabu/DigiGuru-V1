"use client";

import Link from "next/link";
import { useCallback, useEffect, useState, type ReactNode } from "react";

import { ApiError } from "@/lib/api";

// Pieces shared by the academic drill-down screens (Program > Semester >
// Course > Block) and the LSC list.
//
// These live here rather than in `components/admin/primitives.tsx` because
// they encode hierarchy-specific behaviour — breadcrumbs through the academic
// tree, and the "load once, reload after a write" cycle every one of these
// screens repeats. The generic table/modal/control primitives stay generic.

/**
 * Turn any thrown value into copy an admin can act on.
 *
 * An `ApiError` message is already the gateway's `PublicError` message, which
 * is written to be displayed (`.claude/rules/security.md`). Anything else is a
 * bug on our side and gets the neutral fallback — a raw code or stack trace
 * must never reach the screen.
 */
export function messageFor(caught: unknown, fallback: string): string {
  if (caught instanceof ApiError) return caught.message;
  return fallback;
}

/** Validation messages keyed by wire field name, empty for any other error. */
export function fieldErrorsFor(caught: unknown): Record<string, string> {
  return caught instanceof ApiError ? caught.fieldErrors() : {};
}

/** The status code, or 0 for a non-API failure. Used to special-case 409s. */
export function codeFor(caught: unknown): string {
  return caught instanceof ApiError ? caught.code : "";
}

// ------------------------------------------------------------- data loading

export type Resource<T> = {
  data: T | null;
  loading: boolean;
  error: string | null;
  reload: () => void;
};

/**
 * Fetch on mount and on demand, discarding the result of a request that has
 * been superseded.
 *
 * `loader` must be stable (wrap it in `useCallback` at the call site) — it is
 * the dependency that re-runs the fetch, which is how search and pagination
 * re-query without a second effect.
 */
export function useResource<T>(loader: () => Promise<T>, fallback: string): Resource<T> {
  const [nonce, setNonce] = useState(0);
  // One state cell tagged with the request it answers, rather than separate
  // data/loading/error cells: "is this result still the current one" is then a
  // comparison during render, so nothing has to be set synchronously inside
  // the effect to flip the screen back into its loading state.
  const [result, setResult] = useState<{
    loader: () => Promise<T>;
    nonce: number;
    data: T | null;
    error: string | null;
  } | null>(null);

  useEffect(() => {
    let cancelled = false;

    loader()
      .then((next) => {
        if (!cancelled) setResult({ loader, nonce, data: next, error: null });
      })
      .catch((caught: unknown) => {
        if (!cancelled) {
          setResult({ loader, nonce, data: null, error: messageFor(caught, fallback) });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [loader, fallback, nonce]);

  const reload = useCallback(() => setNonce((n) => n + 1), []);
  const current = result !== null && result.loader === loader && result.nonce === nonce;

  return {
    // Stale rows stay on screen under the skeleton's place until the new page
    // lands, so paging does not blank the layout.
    data: result?.data ?? null,
    loading: !current,
    error: current ? result.error : null,
    reload,
  };
}

// ------------------------------------------------------------------ display

/** Muted em dash for an empty optional cell, matching the prototype. */
export function Blank() {
  return <span className="text-gray-500">&mdash;</span>;
}

/** Neutral pill for a code/tag column. */
export function CodeTag({ children }: { children: ReactNode }) {
  return (
    <span className="inline-flex items-center rounded-sm bg-lavender-100 px-1.5 py-0.5 font-mono text-xs text-gray-900">
      {children}
    </span>
  );
}

/** Row link into the next level of the hierarchy. */
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

export type Crumb = { label: string; href?: string };

/**
 * The trail back up the hierarchy.
 *
 * A drill-down screen is reachable only through its parent, so without this an
 * admin four levels deep has the browser's Back button and nothing else.
 */
export function Breadcrumbs({ items }: { items: Crumb[] }) {
  return (
    <nav aria-label="Breadcrumb">
      <ol className="flex flex-wrap items-center gap-1.5 text-sm text-gray-500">
        {items.map((item, index) => (
          <li key={`${item.label}-${index}`} className="flex items-center gap-1.5">
            {index > 0 ? <span aria-hidden="true">/</span> : null}
            {item.href ? (
              <Link
                href={item.href}
                className="underline-offset-2 hover:text-gray-900 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-lavender-50"
              >
                {item.label}
              </Link>
            ) : (
              <span aria-current="page" className="text-gray-900">
                {item.label}
              </span>
            )}
          </li>
        ))}
      </ol>
    </nav>
  );
}

/** Short, human date for `created_at` columns. */
export function formatDate(iso: string): string {
  const parsed = new Date(iso);
  if (Number.isNaN(parsed.getTime())) return iso;
  return parsed.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

// --------------------------------------------------------------- validation

/**
 * Client-side length check mirroring the gateway's `validator` rules
 * (`admin-api-contract.md` §6). It is a courtesy that saves a round trip — the
 * gateway re-validates everything and remains the only enforcement.
 */
export function lengthRule(
  value: string,
  min: number,
  max: number,
  label: string,
): string | undefined {
  const trimmed = value.trim();
  if (trimmed.length < min || trimmed.length > max) {
    return `${label} must be ${min}–${max} characters.`;
  }
  return undefined;
}

export function rangeRule(
  value: string,
  min: number,
  max: number,
  label: string,
): string | undefined {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < min || parsed > max) {
    return `${label} must be a whole number between ${min} and ${max}.`;
  }
  return undefined;
}

/** Drop the key entirely when a value is blank — the wire type is optional. */
export function optionalText(value: string): string | undefined {
  const trimmed = value.trim();
  return trimmed === "" ? undefined : trimmed;
}
