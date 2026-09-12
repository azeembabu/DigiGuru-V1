// Shared building blocks for the admin console.
//
// The console is the one **light-mode** surface in the product (DESIGN.md §7,
// §9.1: "App light mode (dashboards, admin)"). The marketing site and the
// classroom are dark and always dark, so none of the dark-surface classes from
// `components/ui/*` carry over — `lime-500` is the primary action here rather
// than `lime-400`, because `lime-400` does not hold AA on a `lavender-50`
// ground, and every focus ring offsets against the light surface.
//
// Kept deliberately generic: no screen-specific copy or fields live here.

"use client";

import type { ReactNode } from "react";

/** Card/panel surface used by every admin screen. */
export const panelClass =
  "rounded-md border border-lavender-200 bg-white shadow-[0_1px_2px_rgba(10,12,22,0.05)]";

/** Light-surface form control — the counterpart of `ui/Field.controlClass`. */
export const adminControlClass =
  "w-full rounded-sm border bg-white px-3 py-2 text-sm text-gray-900 " +
  "placeholder:text-gray-500 transition-colors focus-visible:outline-none " +
  "focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 " +
  "focus-visible:ring-offset-lavender-50";

export function adminControlBorder(hasError: boolean): string {
  return hasError ? "border-danger" : "border-lavender-200 hover:border-gray-500/50";
}

// ------------------------------------------------------------------- status

type Tone = "success" | "neutral" | "danger" | "warning" | "info";

const toneClass: Record<Tone, string> = {
  success: "bg-success/12 text-[#177245] ring-success/30",
  neutral: "bg-gray-500/10 text-gray-500 ring-gray-500/25",
  danger: "bg-danger/12 text-[#a3281a] ring-danger/30",
  warning: "bg-warning/15 text-[#8a5d05] ring-warning/40",
  info: "bg-indigo-400/12 text-indigo-500 ring-indigo-400/30",
};

/**
 * Status is never colour alone — the label is always spelled out beside it,
 * per DESIGN.md §8. `ring` rather than `border` so the pill does not shift
 * layout when tones change.
 */
export function StatusPill({ tone, children }: { tone: Tone; children: ReactNode }) {
  return (
    <span
      className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ring-1 ring-inset ${toneClass[tone]}`}
    >
      {children}
    </span>
  );
}

/** The `active | inactive | suspended` vocabulary shared by most tables. */
export function statusTone(status: string): Tone {
  switch (status) {
    case "active":
    case "embedded":
      return "success";
    case "suspended":
    case "failed":
      return "danger";
    case "pending":
    case "pending_review":
      return "warning";
    default:
      return "neutral";
  }
}

// ------------------------------------------------------------------ banners

/**
 * Inline result banner. Deliberately inline rather than a toast: an admin who
 * just saved a row needs the outcome next to the row, and a toast that has
 * already faded is useless when the save failed with a field error.
 */
export function Banner({
  tone,
  children,
  onDismiss,
}: {
  tone: "success" | "danger" | "info";
  children: ReactNode;
  onDismiss?: () => void;
}) {
  const style =
    tone === "success"
      ? "border-success/40 bg-success/10 text-[#177245]"
      : tone === "danger"
        ? "border-danger/40 bg-danger/10 text-[#a3281a]"
        : "border-indigo-400/40 bg-indigo-400/10 text-indigo-500";

  return (
    <div
      role={tone === "danger" ? "alert" : "status"}
      className={`flex items-start gap-2 rounded-sm border px-3 py-2 text-sm ${style}`}
    >
      <span aria-hidden="true">{tone === "danger" ? "⚠" : "✓"}</span>
      <span className="flex-1">{children}</span>
      {onDismiss ? (
        <button
          type="button"
          onClick={onDismiss}
          aria-label="Dismiss"
          className="rounded-sm px-1 text-current/70 hover:text-current focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400"
        >
          &times;
        </button>
      ) : null}
    </div>
  );
}

// ------------------------------------------------------------------- layout

export function PageHeader({
  title,
  description,
  action,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-start justify-between gap-4">
      <div>
        <h1 className="font-display text-2xl font-semibold text-gray-900">{title}</h1>
        {description ? <p className="mt-1 text-sm text-gray-500">{description}</p> : null}
      </div>
      {action}
    </div>
  );
}

export function EmptyState({ title, hint }: { title: string; hint?: string }) {
  return (
    <div className="px-4 py-12 text-center">
      <p className="text-sm font-medium text-gray-900">{title}</p>
      {hint ? <p className="mt-1 text-sm text-gray-500">{hint}</p> : null}
    </div>
  );
}

/** Row-shaped shimmer, so a loading table does not collapse then jump. */
export function TableSkeleton({ rows = 5, columns = 4 }: { rows?: number; columns?: number }) {
  return (
    <div className="divide-y divide-lavender-200" aria-hidden="true">
      {Array.from({ length: rows }).map((_, row) => (
        <div key={row} className="flex gap-4 px-4 py-3.5">
          {Array.from({ length: columns }).map((_, column) => (
            <div
              key={column}
              className="h-4 flex-1 animate-pulse rounded-full bg-lavender-100"
            />
          ))}
        </div>
      ))}
    </div>
  );
}
