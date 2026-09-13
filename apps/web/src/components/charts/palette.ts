/**
 * The single colour and number-formatting source for every chart in the admin
 * console. Nothing in `components/charts/*` hard-codes a hex — if a colour is
 * needed that is not here, it belongs here first.
 *
 * The categorical order is not cosmetic: it is the colour-blind-safety
 * mechanism. It was validated (OKLab ΔE, adjacent pairs, protan/deutan/tritan)
 * against a white card surface — worst adjacent CVD ΔE 9.1, worst adjacent
 * normal-vision ΔE 19.6, both clear of the 8 / 15 floors. Slot 1 is the
 * console's own indigo accent so a single-series chart matches the surrounding
 * UI. Re-validate before reordering or substituting a slot; three slots
 * (aqua, yellow, magenta) sit under 3:1 against white, which is why every
 * chart that uses more than one slot also carries a visible text label or a
 * legend rather than relying on the swatch alone.
 */
export const CATEGORICAL = [
  "#5a5fd1", // indigo — console accent
  "#eb6834", // orange
  "#1baf7a", // aqua
  "#eda100", // yellow
  "#e87ba4", // magenta
  "#008300", // green
  "#4a3aa7", // violet
  "#e34948", // red
] as const;

/** Default accent for single-series charts. */
export const ACCENT = CATEGORICAL[0];

/**
 * Slot for series `index`. Deliberately clamped rather than cycled: a ninth
 * series repeating slot 1 would read as "the same thing twice". A caller with
 * more than eight categories folds the tail into an "Other" row.
 */
export function seriesColor(index: number): string {
  return CATEGORICAL[Math.min(Math.max(index, 0), CATEGORICAL.length - 1)];
}

export type Tone = "default" | "good" | "warn" | "bad";

/**
 * Semantic tones. `fill` is for a mark on white; `text` is the darker step that
 * holds WCAG AA as body text — the two are different colours on purpose, since
 * the fill steps for warn/good are too light to read as small text.
 * `word` exists so tone is never conveyed by colour alone.
 */
export const TONES: Record<Tone, { fill: string; text: string; track: string; word: string }> = {
  default: { fill: ACCENT, text: "#15161f", track: "#e4e5f8", word: "" },
  good: { fill: "#1baf7a", text: "#177245", track: "#d8f1e6", word: "good" },
  warn: { fill: "#eda100", text: "#8a5d05", track: "#faeccc", word: "needs attention" },
  bad: { fill: "#e34948", text: "#a3281a", track: "#fadcd9", word: "critical" },
};

/** Chart chrome. Grid and axis are deliberately recessive against the card. */
export const CHROME = {
  grid: "#e4e5f8",
  axis: "#d2d3f0",
  label: "#6c6f82",
  muted: "#b6b9c9",
  track: "#e4e5f8",
} as const;

/** Shared "No data yet" block, so every chart's empty state looks identical. */
export const emptyStateClass =
  "flex items-center justify-center rounded-sm border border-dashed border-lavender-200 " +
  "bg-lavender-50/60 px-3 py-6 text-center text-sm text-gray-500";

export const EMPTY_TEXT = "No data yet";

// ------------------------------------------------------------- number format

/** True when a value can actually be plotted. Guards NaN/Infinity at the edge. */
export function isPlottable(n: number): boolean {
  return Number.isFinite(n);
}

/** Non-finite input is an em-dash, never `NaN` on screen. */
export function formatCount(n: number): string {
  if (!Number.isFinite(n)) return "—";
  return Math.round(n).toLocaleString("en-US");
}

/** Compact form for axis ticks, where horizontal room is scarce. */
export function formatCompact(n: number): string {
  if (!Number.isFinite(n)) return "—";
  const abs = Math.abs(n);
  if (abs >= 1_000_000) return `${trim(n / 1_000_000)}M`;
  if (abs >= 1_000) return `${trim(n / 1_000)}k`;
  return `${Math.round(n)}`;
}

/** `ratio` is 0..1, not 0..100. */
export function formatPercent(ratio: number, digits = 0): string {
  if (!Number.isFinite(ratio)) return "—";
  return `${(clamp01(ratio) * 100).toFixed(digits)}%`;
}

export function clamp01(n: number): number {
  if (!Number.isFinite(n)) return 0;
  return Math.min(Math.max(n, 0), 1);
}

function trim(n: number): string {
  // One decimal, but "1.0k" reads worse than "1k".
  const s = n.toFixed(1);
  return s.endsWith(".0") ? s.slice(0, -2) : s;
}

// -------------------------------------------------------------- shared types

/**
 * The chart data contract, kept here rather than in any one chart file so that
 * `BarChart` and `DonutChart` do not have to import from each other.
 */
export type Point = { label: string; value: number };

export type SeriesPoint = { label: string; value: number; secondary?: number };
