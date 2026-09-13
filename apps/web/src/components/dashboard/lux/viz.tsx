// Chart primitives for the quiet-luxury student dashboard.
//
// Separate from `components/charts/*` on purpose: that set is built for the
// admin console's lavender light surface and its palette module is shared by
// every admin screen, so re-tinting it for the cream dashboard would repaint the
// console too. These are the same *method* (hand-written inline SVG, no charting
// dependency) against the `lux-*` surfaces.
//
// Colour here is not a taste decision. The two series hexes were run through the
// `dataviz` skill's `validate_palette.js` on BOTH surfaces and pass all six
// checks — see the note beside the `lux-*` tokens in `globals.css` for the
// measured numbers. Do not substitute a hex without re-running it.
//
// Mark specs, also from that skill and applied uniformly below: bars capped at
// 24px with a 4px rounded data-end and a square baseline, 2px lines, ≥8px
// markers, a 2px surface gap between touching fills, hairline solid gridlines,
// and selective direct labels rather than a number on every mark.

import type { ReactNode } from "react";

// ---------------------------------------------------------------- palette

export const LUX = {
  light: {
    surface: "#faf8f4",
    series: "#0a6b3c",
    emphasis: "#9c7a24",
    // The de-emphasis step for "context, not the subject". Below 3:1, which is
    // why it is never the only thing distinguishing a mark that carries meaning.
    muted: "#b4ada0",
    grid: "#e7e2d8",
    axis: "#d8d2c4",
    track: "#e7e2d8",
  },
  dark: {
    surface: "#23231f",
    series: "#35a873",
    emphasis: "#b8851f",
    muted: "#6b675d",
    grid: "#3b3b34",
    axis: "#4c4c43",
    track: "#3b3b34",
  },
} as const;

export type VizMode = keyof typeof LUX;

/**
 * A column/bar path: rounded at the data end, square at the baseline.
 *
 * A plain `rx` on a `<rect>` rounds all four corners, which detaches the mark
 * from its own baseline and makes short bars read as floating pills. The radius
 * also has to collapse for a bar shorter than the radius, or the path inverts.
 */
export function barPath(x: number, y: number, w: number, h: number, vertical = true): string {
  const r = Math.min(4, w / 2, Math.max(h, 0));
  if (h <= 0) return "";

  if (vertical) {
    // Grows upward from the baseline at y + h.
    return [
      `M${x} ${y + h}`,
      `V${y + r}`,
      `Q${x} ${y} ${x + r} ${y}`,
      `H${x + w - r}`,
      `Q${x + w} ${y} ${x + w} ${y + r}`,
      `V${y + h}`,
      "Z",
    ].join(" ");
  }

  // Horizontal: grows rightward from the baseline at x, rounded at x + w.
  const rh = Math.min(4, h / 2, Math.max(w, 0));
  return [
    `M${x} ${y}`,
    `H${x + w - rh}`,
    `Q${x + w} ${y} ${x + w} ${y + rh}`,
    `V${y + h - rh}`,
    `Q${x + w} ${y + h} ${x + w - rh} ${y + h}`,
    `H${x}`,
    "Z",
  ].join(" ");
}

// ------------------------------------------------------------------- meter

/**
 * A single ratio against a limit — the right form for the speaking-time quota
 * and for course progress, and specifically not a two-slice pie.
 *
 * The unfilled track is a lighter step of the same family, so state reads across
 * the whole bar rather than only where the fill stops. `tone` is passed in rather
 * than derived from the fraction: "80% of your quota" and "80% of your course"
 * mean opposite things, and only the caller knows which.
 */
export function Meter({
  fraction,
  mode,
  tone = "series",
  label,
}: {
  /** 0..1. A caller with nothing to measure renders `MeterEmpty` instead. */
  fraction: number;
  mode: VizMode;
  tone?: "series" | "emphasis" | "muted";
  /** The accessible sentence. The visible number lives beside the meter. */
  label: string;
}) {
  const c = LUX[mode];
  const clamped = Math.min(Math.max(Number.isFinite(fraction) ? fraction : 0, 0), 1);
  const fill = tone === "emphasis" ? c.emphasis : tone === "muted" ? c.muted : c.series;

  return (
    <div
      role="img"
      aria-label={label}
      className="h-2 w-full overflow-hidden rounded-full"
      style={{ backgroundColor: c.track }}
    >
      {/* A zero-width div would vanish; a hairline sliver reads as "started but
          barely", which is a different claim. Zero renders as an empty track. */}
      {clamped > 0 ? (
        <div
          className="h-full rounded-full transition-[width] duration-500"
          style={{ width: `${clamped * 100}%`, backgroundColor: fill }}
        />
      ) : null}
    </div>
  );
}

/** The track alone, for a tile whose ratio is not measured. Never a 0% fill. */
export function MeterEmpty({ mode, label }: { mode: VizMode; label: string }) {
  return (
    <div
      role="img"
      aria-label={label}
      className="h-2 w-full rounded-full"
      style={{ backgroundColor: LUX[mode].track }}
    />
  );
}

// --------------------------------------------------------------- mini bars

/**
 * The KPI tiles' micro chart: a handful of values as thin columns, no axes.
 *
 * It says "these are the shapes of the last few readings" and nothing more
 * precise, which is why it carries no labels — the tile's own value is the
 * number, and the table and tooltips carry the rest.
 */
export function MiniBars({
  values,
  mode,
  emphasiseLast = false,
  label,
  height = 28,
}: {
  values: number[];
  mode: VizMode;
  emphasiseLast?: boolean;
  label: string;
  height?: number;
}) {
  const c = LUX[mode];
  const usable = values.filter((n) => Number.isFinite(n) && n >= 0);

  if (usable.length === 0) {
    return <MeterEmpty mode={mode} label={label} />;
  }

  const max = Math.max(...usable);
  // A series that is all zeroes is a real measurement, so the marks must still
  // be visible as zeroes rather than dividing by a zero range.
  const scale = max > 0 ? max : 1;
  const slot = 100 / usable.length;
  // The 2px surface gap, expressed in the same percentage units as the slot.
  const barW = Math.max(slot - 2.5, 1);

  return (
    <svg
      viewBox={`0 0 100 ${height}`}
      preserveAspectRatio="none"
      style={{ width: "100%", height }}
      role="img"
      aria-label={label}
    >
      {usable.map((v, i) => {
        // A floor of 1.5px so a genuine zero still shows a mark at the baseline
        // instead of disappearing, which would misread as "no data".
        const h = Math.max((v / scale) * (height - 2), 1.5);
        const x = i * slot + (slot - barW) / 2;
        const emphasised = emphasiseLast && i === usable.length - 1;
        return (
          <rect
            key={i}
            x={x}
            y={height - h}
            width={barW}
            height={h}
            rx={0.8}
            fill={emphasised ? c.emphasis : c.muted}
          />
        );
      })}
    </svg>
  );
}

// --------------------------------------------------------------- sparkline

/**
 * A per-course trendline for the table.
 *
 * Renders only when the caller has at least two real readings. It deliberately
 * has no empty fallback of its own: a sparkline drawn through one point, or a
 * flat line drawn through none, asserts a stability that was never measured, so
 * the table renders `TrendUnavailable` instead.
 */
export function LuxSparkline({
  values,
  mode,
  width = 88,
  height = 26,
  label,
}: {
  values: number[];
  mode: VizMode;
  width?: number;
  height?: number;
  label: string;
}) {
  const c = LUX[mode];
  const usable = values.filter((n) => Number.isFinite(n));
  if (usable.length < 2) return <TrendUnavailable />;

  const pad = 3;
  const min = Math.min(...usable);
  const max = Math.max(...usable);
  const range = max - min;

  const x = (i: number) => pad + (i / (usable.length - 1)) * (width - pad * 2);
  // A genuinely flat series is centred rather than pinned to an edge: with a
  // zero range every point is simultaneously the min and the max.
  const y = (v: number) =>
    range <= 0 ? height / 2 : pad + (1 - (v - min) / range) * (height - pad * 2);

  const d = usable.map((v, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(2)} ${y(v).toFixed(2)}`).join(" ");
  const lastIndex = usable.length - 1;

  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      preserveAspectRatio="xMidYMid meet"
      style={{ width, height }}
      role="img"
      aria-label={label}
    >
      <path
        d={d}
        fill="none"
        stroke={c.series}
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      {/* The end-dot carries a 2px surface ring so it stays legible where it
          crosses the line it terminates. */}
      <circle
        cx={x(lastIndex)}
        cy={y(usable[lastIndex])}
        r={4}
        fill={c.series}
        stroke={c.surface}
        strokeWidth={2}
      />
    </svg>
  );
}

/**
 * The honest stand-in for a trend that does not exist yet.
 *
 * A dash, not a flat line: the distinction between "we measured no change" and
 * "we have not measured twice" is exactly the one this dashboard is not allowed
 * to blur. The text is in the DOM for a screen reader, not only as a glyph.
 */
export function TrendUnavailable() {
  return (
    <span className="inline-flex items-center gap-1.5 text-[13px] text-lux-ink-600">
      <span aria-hidden="true" className="inline-block h-px w-6 bg-lux-cream-400" />
      <span className="sr-only">Not enough attempts to show a trend</span>
      <span aria-hidden="true">—</span>
    </span>
  );
}

// -------------------------------------------------------------- empty state

/**
 * The two empty states, kept together so the difference between them is visible
 * in one place.
 *
 * "Nothing yet" and "could not load" are different facts about the world, and
 * showing the first when the second is true tells a student their record is
 * empty when it is merely unread.
 */
export function VizEmpty({
  kind,
  title,
  hint,
  action,
  mode = "light",
}: {
  kind: "empty" | "error";
  title: string;
  hint?: string;
  action?: ReactNode;
  mode?: VizMode;
}) {
  const dark = mode === "dark";
  const border =
    kind === "error"
      ? "border-solid border-[#d03b3b]/45"
      : dark
        ? "border-dashed border-lux-char-700"
        : "border-dashed border-lux-cream-400";

  return (
    <div
      role={kind === "error" ? "alert" : undefined}
      className={`flex flex-col items-center justify-center rounded-[14px] border px-4 py-8 text-center ${border} ${
        dark ? "bg-lux-char-800/40" : "bg-lux-cream-200/50"
      }`}
    >
      {/* The icon-plus-word pairing, so "this failed" never rests on the red
          border alone. */}
      {kind === "error" ? (
        <span
          className="mb-2 inline-flex items-center gap-1.5 text-[12px] font-semibold uppercase tracking-[0.08em]"
          style={{ color: dark ? "#ec835a" : "#a33226" }}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true" className="h-3.5 w-3.5 fill-current">
            <path d="M8 1.5 15 14H1L8 1.5Zm0 4.2a.85.85 0 0 0-.85.85v2.6a.85.85 0 0 0 1.7 0V6.55A.85.85 0 0 0 8 5.7Zm0 5.1a.95.95 0 1 0 0 1.9.95.95 0 0 0 0-1.9Z" />
          </svg>
          Could not load
        </span>
      ) : null}
      <p className={`text-[15px] font-semibold ${dark ? "text-lux-mist-100" : "text-lux-ink-900"}`}>
        {title}
      </p>
      {hint !== undefined ? (
        <p
          className={`mx-auto mt-1.5 max-w-sm text-[13.5px] leading-[1.6] ${
            dark ? "text-lux-mist-400" : "text-lux-ink-600"
          }`}
        >
          {hint}
        </p>
      ) : null}
      {action !== undefined ? <div className="mt-4">{action}</div> : null}
    </div>
  );
}
