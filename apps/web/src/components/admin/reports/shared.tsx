"use client";

import { StatusPill } from "@/components/admin/primitives";
import type { AttemptStatus } from "@/lib/admin/types";

// Formatting shared by the two Student Reports screens.
//
// The rule that shapes all of it: a missing measurement is a DASH, never a
// zero. `score`, `percentage`, `time_spent_seconds` and the averages are all
// `null` until there is something real to report (A2.4), and "0%" is a genuine
// measured score — showing it for an ungraded attempt would report a student as
// having failed an exam nobody has marked yet.

/** Shown wherever a number does not exist yet. */
export const NOT_YET = "—";

export function Dash() {
  return <span className="text-gray-500">{NOT_YET}</span>;
}

/** `6m 52s`, `1h 04m`, `—` while an attempt is still running. */
export function formatDuration(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds) || seconds < 0) return NOT_YET;

  const whole = Math.floor(seconds);
  if (whole < 60) return `${whole}s`;

  const minutes = Math.floor(whole / 60);
  const restSeconds = whole % 60;
  if (minutes < 60) return `${minutes}m ${String(restSeconds).padStart(2, "0")}s`;

  const hours = Math.floor(minutes / 60);
  return `${hours}h ${String(minutes % 60).padStart(2, "0")}m`;
}

/** `80%`, or a dash when there is nothing measured. */
export function formatPercent(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return NOT_YET;
  return `${Math.round(value)}%`;
}

/** `8 / 10`. Scores are numbers on the wire; the slash is this layer's job. */
export function formatScore(score: number | null, maxScore: number): string {
  const max = Number.isFinite(maxScore) ? trim(maxScore) : NOT_YET;
  if (score === null || !Number.isFinite(score)) return `${NOT_YET} / ${max}`;
  return `${trim(score)} / ${max}`;
}

function trim(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(2).replace(/0$/, "");
}

export function formatDateTime(iso: string | null): string {
  if (!iso) return NOT_YET;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return NOT_YET;
  return date.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/**
 * An attempt's status as a pill.
 *
 * `submitted` is neutral rather than green: it means waiting to be marked, and
 * colouring it as a success would read as a result.
 */
export function AttemptStatusPill({ status }: { status: AttemptStatus }) {
  const tone =
    status === "graded"
      ? "success"
      : status === "abandoned"
        ? "danger"
        : "neutral";
  return <StatusPill tone={tone}>{status.replace(/_/g, " ")}</StatusPill>;
}

/**
 * A percentage banded by outcome.
 *
 * Ungraded stays a plain dash with no band at all — a band implies a judgement
 * that has not been made.
 */
export function PercentPill({ value }: { value: number | null }) {
  if (value === null || !Number.isFinite(value)) return <Dash />;
  const tone = value >= 70 ? "success" : value >= 40 ? "neutral" : "danger";
  return <StatusPill tone={tone}>{formatPercent(value)}</StatusPill>;
}
