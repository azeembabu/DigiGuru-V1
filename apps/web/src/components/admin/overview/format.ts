// Formatting and tone helpers shared by the overview sections.
//
// One rule runs through all of it: a metric the gateway reports as `null` is
// *unmeasured*, and it renders as an em dash. Printing `0` for it would be a
// measurement we never took.

/** The tone vocabulary the chart primitives and the tiles share. */
export type MetricTone = "default" | "good" | "warn" | "bad";

export function n(value: number): string {
  return value.toLocaleString();
}

/** A count, or an em dash — used wherever a value may legitimately be absent. */
export function maybeN(value: number | null): string {
  return value === null ? "—" : n(Math.round(value));
}

/** Milliseconds as tutoring time: `—`, `1m 30s`, `2h 14m`. */
export function duration(ms: number | null): string {
  if (ms === null) return "—";
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ${seconds % 60}s`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}

export function millis(ms: number | null): string {
  return ms === null ? "—" : `${Math.round(ms)} ms`;
}

/** A 0..1 ratio as a percentage. `fraction` of 2 keeps a small rate visible. */
export function ratio(value: number | null, fraction = 1): string {
  return value === null ? "—" : `${(value * 100).toFixed(fraction)}%`;
}

/** `count` as a share of `total`, rendered for a hint line. */
export function shareOf(count: number, total: number): string {
  if (total <= 0) return "—";
  return `${Math.round((count / total) * 100)}% of ${n(total)}`;
}

/** `bad` only once the count is actually non-zero; otherwise calm. */
export function badIfAny(count: number): MetricTone {
  return count > 0 ? "bad" : "default";
}

export function warnIfAny(count: number): MetricTone {
  return count > 0 ? "warn" : "default";
}

/** `good` once there is something to be good about — never on an empty table. */
export function goodIfAny(count: number): MetricTone {
  return count > 0 ? "good" : "default";
}

/** Pull the plain values out of a series for a `Sparkline` / `StatTile` trend. */
export function trendOf<T>(rows: readonly T[], pick: (row: T) => number): number[] {
  return rows.map(pick);
}

/** `2026-09-13` -> `13 Sep`, the axis label form. Falls back to the raw day. */
export function dayLabel(day: string): string {
  const parsed = new Date(`${day}T00:00:00Z`);
  if (Number.isNaN(parsed.getTime())) return day;
  return parsed.toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    timeZone: "UTC",
  });
}

/** `pending_review` -> `Pending review`, for a chart legend. */
export function humanLabel(label: string): string {
  const spaced = label.replace(/_/g, " ");
  return spaced.charAt(0).toUpperCase() + spaced.slice(1);
}

/** Chart accents, taken from the console's light-theme tokens (`globals.css`). */
export const ACCENT = {
  indigo: "#6e74e8",
  success: "#3fcf7f",
  warning: "#f5b84a",
  danger: "#f1503d",
  gray: "#6c6f82",
} as const;
