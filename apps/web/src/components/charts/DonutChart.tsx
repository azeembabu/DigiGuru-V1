import {
  CHROME,
  EMPTY_TEXT,
  emptyStateClass,
  formatCount,
  formatPercent,
  seriesColor,
  type Point,
} from "./palette";

type DonutChartProps = {
  data: Point[];
  size?: number;
  centerLabel?: string;
  valueFormat?: (n: number) => string;
};

/**
 * Parts of a whole, with the total in the middle and a legend beside it.
 *
 * Arcs are drawn as `stroke-dasharray` on a single circle rather than as `A`
 * path commands: the dash approach has no 180°/360° special case, so a lone
 * segment covering the entire ring renders correctly instead of collapsing —
 * which is exactly the shape an almost-empty database produces.
 */
export function DonutChart({
  data,
  size = 180,
  centerLabel,
  valueFormat = formatCount,
}: DonutChartProps) {
  const points = data.filter((d) => Number.isFinite(d.value) && d.value > 0);
  const total = points.reduce((sum, d) => sum + d.value, 0);

  if (points.length === 0 || total <= 0) {
    return (
      <div className={emptyStateClass} style={{ minHeight: Math.min(size, 120) }} role="img" aria-label={`${centerLabel ?? "Breakdown"}: ${EMPTY_TEXT}`}>
        {EMPTY_TEXT}
      </div>
    );
  }

  const stroke = Math.max(10, Math.round(size * 0.16));
  const r = (size - stroke) / 2;
  const circumference = 2 * Math.PI * r;
  // A 2px surface gap between neighbouring segments. Skipped when a segment is
  // too small to survive it, and when there is only one segment (a gap in a
  // full ring would read as a missing slice).
  const gap = points.length > 1 ? 2 : 0;

  const arcs = buildArcs(points, total, circumference, gap);

  const summary = `${centerLabel ?? "Breakdown"}, total ${valueFormat(total)}. ${points
    .map((p) => `${p.label} ${valueFormat(p.value)} (${formatPercent(p.value / total)})`)
    .join(", ")}.`;

  return (
    <div className="flex flex-wrap items-center gap-x-6 gap-y-4" role="img" aria-label={summary}>
      <div className="relative shrink-0" style={{ width: size, maxWidth: "100%" }}>
        <svg viewBox={`0 0 ${size} ${size}`} preserveAspectRatio="xMidYMid meet" style={{ width: "100%", height: "auto" }} aria-hidden="true">
          {/* -90° so the first segment starts at twelve o'clock, where a reader
              expects a ring to begin. */}
          <g transform={`rotate(-90 ${size / 2} ${size / 2})`}>
            <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke={CHROME.track} strokeWidth={stroke} />
            {arcs.map((a) => (
              <circle
                key={a.key}
                cx={size / 2}
                cy={size / 2}
                r={r}
                fill="none"
                stroke={a.color}
                strokeWidth={stroke}
                strokeDasharray={`${a.drawn} ${circumference - a.drawn}`}
                strokeDashoffset={-a.offset}
              />
            ))}
          </g>
        </svg>
        <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center">
          <span className="font-display text-2xl font-semibold tabular-nums text-gray-900">
            {valueFormat(total)}
          </span>
          {centerLabel ? <span className="mt-0.5 text-xs text-gray-500">{centerLabel}</span> : null}
        </div>
      </div>

      {/* The legend is not decoration: three of the palette's slots sit under
          3:1 against white, so the written label is what carries identity. */}
      <ul className="m-0 min-w-[8rem] flex-1 list-none space-y-1.5 p-0">
        {points.map((p, i) => (
          <li key={`${p.label}-${i}`} className="flex items-center gap-2 text-sm">
            <span
              aria-hidden="true"
              className="size-2.5 shrink-0 rounded-full"
              style={{ background: seriesColor(i) }}
            />
            <span className="min-w-0 flex-1 truncate text-gray-900">{p.label}</span>
            <span className="tabular-nums text-gray-500">{valueFormat(p.value)}</span>
            <span className="w-10 shrink-0 text-right tabular-nums text-gray-500">
              {formatPercent(p.value / total)}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

type Arc = { key: string; color: string; drawn: number; offset: number };

/**
 * Each segment is the same full circle, offset by the arcs already laid down —
 * so the running total is accumulated here, outside the render, rather than in
 * a mutable local inside the component body.
 */
function buildArcs(points: Point[], total: number, circumference: number, gap: number): Arc[] {
  const arcs: Arc[] = [];
  let offset = 0;
  for (const [i, p] of points.entries()) {
    const length = (p.value / total) * circumference;
    arcs.push({
      key: `${p.label}-${i}`,
      color: seriesColor(i),
      // A segment narrower than the gap keeps a 1px sliver, so a tiny-but-real
      // category never disappears entirely.
      drawn: Math.max(length - gap, Math.min(length, 1)),
      offset,
    });
    offset += length;
  }
  return arcs;
}
