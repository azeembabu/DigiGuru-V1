import {
  CHROME,
  EMPTY_TEXT,
  emptyStateClass,
  formatCompact,
  formatCount,
  seriesColor,
  ACCENT,
  type SeriesPoint,
} from "./palette";

type LineChartProps = {
  data: SeriesPoint[];
  height?: number;
  label?: string;
  valueFormat?: (n: number) => string;
  accent?: string;
};

// The viewBox is a fixed drawing space scaled uniformly to the parent's width,
// so `height` is an intrinsic height at 640px wide rather than a pixel lock —
// the chart shrinks whole (text included) instead of overflowing at 400px.
// Font sizes are set for that worst case, and the x-axis labels are thinned to
// at most three so they cannot collide once scaled down.
const VB_W = 640;
const PAD = { top: 12, right: 14, bottom: 28, left: 52 } as const;

/**
 * A time series over a gap-filled window (30 points is the typical case).
 *
 * `secondary` draws a second, dashed line against the *same* scale — never a
 * second y-axis. Two measures whose magnitudes are not comparable belong in
 * two charts.
 */
export function LineChart({
  data,
  height = 180,
  label,
  valueFormat = formatCount,
  accent = ACCENT,
}: LineChartProps) {
  const points = data.filter((d) => Number.isFinite(d.value));
  const values = points.map((d) => d.value);
  const secondaries = points
    .map((d) => d.secondary)
    .filter((v): v is number => typeof v === "number" && Number.isFinite(v));
  const hasSecondary = secondaries.length === points.length && points.length > 0;

  const max = Math.max(0, ...values, ...secondaries);

  // An empty array and an all-zero window are the same story for a reader:
  // there is nothing to see yet. Drawing a flat line pinned to the axis would
  // imply a measured zero, which is a stronger claim than we can make.
  if (points.length === 0 || max <= 0) {
    return (
      <div className={emptyStateClass} style={{ minHeight: height }} role="img" aria-label={`${label ?? "Trend"}: ${EMPTY_TEXT}`}>
        {EMPTY_TEXT}
      </div>
    );
  }

  const plotW = VB_W - PAD.left - PAD.right;
  const plotH = height - PAD.top - PAD.bottom;
  const single = points.length === 1;

  // A one-point series has no interval to spread across, so it is centred
  // rather than pinned to the left edge where it would read as a truncated line.
  const xAt = (i: number) => (single ? PAD.left + plotW / 2 : PAD.left + (i / (points.length - 1)) * plotW);
  const yAt = (v: number) => PAD.top + plotH - (v / max) * plotH;

  const line = (get: (p: SeriesPoint) => number) =>
    points.map((p, i) => `${i === 0 ? "M" : "L"}${xAt(i).toFixed(2)} ${yAt(get(p)).toFixed(2)}`).join(" ");

  const path = line((p) => p.value);
  // The area closes back along the baseline, so the fill sits under the line
  // rather than around it.
  const area = `${path} L${xAt(points.length - 1).toFixed(2)} ${PAD.top + plotH} L${xAt(0).toFixed(2)} ${PAD.top + plotH} Z`;

  const ticks = [0, max / 2, max];
  const secondColor = seriesColor(1);

  const last = points[points.length - 1];
  const summary =
    `${label ?? "Trend"}: ${points.length} point${points.length === 1 ? "" : "s"} from ` +
    `${points[0].label} to ${last.label}, latest ${valueFormat(last.value)}, peak ${valueFormat(max)}.`;

  return (
    <figure className="m-0">
      {label ? <figcaption className="mb-2 text-sm font-medium text-gray-900">{label}</figcaption> : null}
      <svg
        viewBox={`0 0 ${VB_W} ${height}`}
        preserveAspectRatio="xMidYMid meet"
        style={{ width: "100%", height: "auto" }}
        role="img"
        aria-label={summary}
      >
        {ticks.map((t) => (
          <g key={t}>
            <line
              x1={PAD.left}
              x2={VB_W - PAD.right}
              y1={yAt(t)}
              y2={yAt(t)}
              stroke={t === 0 ? CHROME.axis : CHROME.grid}
              strokeWidth={1}
              vectorEffect="non-scaling-stroke"
            />
            <text x={PAD.left - 8} y={yAt(t) + 5} textAnchor="end" fontSize={14} fill={CHROME.label}>
              {formatCompact(t)}
            </text>
          </g>
        ))}

        <path d={area} fill={accent} fillOpacity={0.1} />
        <path
          d={path}
          fill="none"
          stroke={accent}
          strokeWidth={2}
          strokeLinejoin="round"
          strokeLinecap="round"
          vectorEffect="non-scaling-stroke"
        />
        {hasSecondary ? (
          <path
            d={line((p) => p.secondary ?? 0)}
            fill="none"
            stroke={secondColor}
            strokeWidth={2}
            strokeDasharray="5 4"
            strokeLinecap="round"
            vectorEffect="non-scaling-stroke"
          />
        ) : null}

        {/* A lone point has no line to trace, so the marker is the whole chart. */}
        {single ? <circle cx={xAt(0)} cy={yAt(points[0].value)} r={4} fill={accent} /> : null}

        {xLabelIndices(points.length).map((i) => (
          <text
            key={i}
            x={xAt(i)}
            y={height - 6}
            textAnchor={i === 0 ? "start" : i === points.length - 1 ? "end" : "middle"}
            fontSize={14}
            fill={CHROME.label}
          >
            {points[i].label}
          </text>
        ))}
      </svg>
      {hasSecondary ? (
        <div className="mt-2 flex flex-wrap gap-4 text-xs text-gray-500">
          <LegendKey color={accent} dashed={false}>
            {label ?? "Primary"}
          </LegendKey>
          <LegendKey color={secondColor} dashed>
            Comparison
          </LegendKey>
        </div>
      ) : null}
    </figure>
  );
}

/** First, middle, last — enough to orient without colliding at 400px wide. */
function xLabelIndices(n: number): number[] {
  if (n <= 1) return [0];
  if (n === 2) return [0, 1];
  return [0, Math.floor((n - 1) / 2), n - 1];
}

function LegendKey({
  color,
  dashed,
  children,
}: {
  color: string;
  dashed: boolean;
  children: React.ReactNode;
}) {
  return (
    <span className="inline-flex items-center gap-1.5">
      <svg width={14} height={8} aria-hidden="true">
        <line
          x1={0}
          x2={14}
          y1={4}
          y2={4}
          stroke={color}
          strokeWidth={2}
          strokeDasharray={dashed ? "4 3" : undefined}
        />
      </svg>
      {children}
    </span>
  );
}
