// Layout shells for the overview: a titled section, a card that wraps a chart,
// and the compact metric row used for the long tails of numbers that do not
// each deserve a headline tile.
//
// Every one of these degrades to the all-zero case deliberately. An empty
// platform is the *normal* first render of this console, so a section with no
// data says what would fill it rather than showing a broken-looking chart.

"use client";

import Link from "next/link";
import type { ReactNode } from "react";

import { panelClass } from "@/components/admin/primitives";

import type { MetricTone } from "./format";

const valueToneClass: Record<MetricTone, string> = {
  default: "text-gray-900",
  good: "text-[#177245]",
  warn: "text-[#8a5d05]",
  bad: "text-[#a3281a]",
};

export function Section({
  id,
  title,
  description,
  children,
}: {
  id: string;
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section aria-labelledby={id} className="space-y-4">
      <div>
        <h2 id={id} className="font-display text-lg font-semibold text-gray-900">
          {title}
        </h2>
        {description ? <p className="mt-1 text-sm text-gray-500">{description}</p> : null}
      </div>
      {children}
    </section>
  );
}

/** A responsive tile grid: one column on a phone, widening with the viewport. */
export function TileGrid({ children }: { children: ReactNode }) {
  return <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">{children}</div>;
}

export function ChartCard({
  title,
  hint,
  empty,
  emptyHint,
  children,
}: {
  title: string;
  hint?: string;
  /** True when the series carries no signal at all — renders the hint instead. */
  empty?: boolean;
  emptyHint?: string;
  children: ReactNode;
}) {
  return (
    <div className={`${panelClass} min-w-0 p-5`}>
      <h3 className="font-display text-sm font-semibold text-gray-900">{title}</h3>
      {hint ? <p className="mt-1 text-xs text-gray-500">{hint}</p> : null}
      <div className="mt-4">
        {empty ? (
          <p className="py-8 text-center text-sm text-gray-500">{emptyHint ?? "Nothing yet."}</p>
        ) : (
          children
        )}
      </div>
    </div>
  );
}

export type MetricRow = {
  label: string;
  value: string;
  hint?: string;
  tone?: MetricTone;
  /** Optional drill-down to the records behind the number. */
  href?: string;
  /** The link's accessible sentence, e.g. "View 3 dropped enrolments". */
  linkLabel?: string;
};

/**
 * A definition list of secondary metrics. Kept as text rather than tiles so a
 * section can carry a dozen numbers without burying the two that matter.
 */
export function MetricList({ title, hint, rows }: { title: string; hint?: string; rows: MetricRow[] }) {
  return (
    <div className={`${panelClass} min-w-0 p-5`}>
      <h3 className="font-display text-sm font-semibold text-gray-900">{title}</h3>
      {hint ? <p className="mt-1 text-xs text-gray-500">{hint}</p> : null}
      <dl className="mt-3 divide-y divide-lavender-100">
        {rows.map((row) => (
          <div
            key={row.label}
            className={`relative flex items-baseline justify-between gap-4 py-2 ${
              row.href
                ? "group -mx-2 rounded-sm px-2 transition-colors hover:bg-lavender-50 focus-within:bg-lavender-50 focus-within:ring-2 focus-within:ring-indigo-400"
                : ""
            }`}
          >
            <div className="min-w-0">
              <dt className="text-sm text-gray-500">
                {row.href ? (
                  // A stretched overlay rather than an <a> wrapping dt/dd: it
                  // keeps the definition list valid while making the whole row
                  // one keyboard-reachable click target.
                  <Link
                    href={row.href}
                    aria-label={row.linkLabel ?? `View ${row.label}: ${row.value}`}
                    className="after:absolute after:inset-0 group-hover:text-gray-900 focus-visible:outline-none"
                  >
                    {row.label}
                  </Link>
                ) : (
                  row.label
                )}
              </dt>
              {row.hint ? <p className="text-xs text-gray-300">{row.hint}</p> : null}
            </div>
            <dd
              className={`shrink-0 font-display text-sm font-semibold tabular-nums ${
                row.href && row.value === "0"
                  ? "text-gray-500/70"
                  : valueToneClass[row.tone ?? "default"]
              }`}
            >
              {row.value}
            </dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

export type ChartLink = { label: string; value: string; href: string; linkLabel: string };

/**
 * The clickable counterpart of a chart's categories.
 *
 * The chart primitives (`DonutChart`, `BarChart`) are shared, presentation-only
 * SVG and stay that way — so a donut legend or a latency bucket is opened from
 * this row of chips directly beneath the chart instead. Same categories, same
 * order, but each one is a real `<Link>`: focusable, right-clickable, and
 * readable as a sentence by a screen reader.
 */
export function ChartLinks({ title, links }: { title: string; links: ChartLink[] }) {
  if (links.length === 0) return null;

  return (
    <nav aria-label={title} className="mt-4 border-t border-lavender-100 pt-3">
      <p className="text-xs text-gray-500">{title}</p>
      <ul className="mt-2 flex list-none flex-wrap gap-2 p-0">
        {links.map((link) => (
          <li key={link.label}>
            <Link
              href={link.href}
              aria-label={link.linkLabel}
              className="inline-flex items-center gap-1.5 rounded-full border border-lavender-200 px-2.5 py-1 text-xs text-gray-900 transition-colors hover:border-indigo-400 hover:bg-lavender-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-lavender-50"
            >
              <span className="truncate">{link.label}</span>
              <span
                className={`tabular-nums ${link.value === "0" ? "text-gray-500/70" : "text-gray-500"}`}
              >
                {link.value}
              </span>
            </Link>
          </li>
        ))}
      </ul>
    </nav>
  );
}

/** Two-up on desktop, stacked on a phone — the shape most sections want. */
export function SplitGrid({ children }: { children: ReactNode }) {
  return <div className="grid gap-4 lg:grid-cols-2">{children}</div>;
}

/** Skeleton stand-in while the first fetch is in flight. */
export function OverviewSkeleton() {
  return (
    <div className="space-y-4" aria-hidden="true">
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {Array.from({ length: 8 }).map((_, index) => (
          <div key={index} className={`${panelClass} p-5`}>
            <div className="h-3 w-24 animate-pulse rounded-full bg-lavender-100" />
            <div className="mt-3 h-8 w-16 animate-pulse rounded-sm bg-lavender-100" />
          </div>
        ))}
      </div>
      <div className="grid gap-4 lg:grid-cols-2">
        {Array.from({ length: 2 }).map((_, index) => (
          <div key={index} className={`${panelClass} p-5`}>
            <div className="h-3 w-32 animate-pulse rounded-full bg-lavender-100" />
            <div className="mt-4 h-40 animate-pulse rounded-sm bg-lavender-100" />
          </div>
        ))}
      </div>
    </div>
  );
}
