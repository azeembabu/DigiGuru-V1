import {
  ACCENT,
  CHROME,
  EMPTY_TEXT,
  emptyStateClass,
  formatCount,
  type Point,
} from "./palette";

type BarChartProps = {
  data: Point[];
  height?: number;
  horizontal?: boolean;
  valueFormat?: (n: number) => string;
  accent?: string;
  highlightLabels?: string[];
};

// Same scaling contract as LineChart: a fixed drawing space scaled uniformly,
// so `height` is the intrinsic height at 640px wide.
const VB_W = 640;
const FONT = 14;

/**
 * Categorical magnitudes. `horizontal` is the right form for a top-N list —
 * long category names get real room instead of being rotated 45°.
 *
 * Every bar carries its value as a direct label, which is also what lets the
 * palette's lighter slots be used at all: identity and magnitude never depend
 * on the fill colour alone.
 */
export function BarChart({
  data,
  height = 200,
  horizontal = false,
  valueFormat = formatCount,
  accent = ACCENT,
  highlightLabels,
}: BarChartProps) {
  const points = data.filter((d) => Number.isFinite(d.value) && d.value >= 0);
  const max = Math.max(0, ...points.map((d) => d.value));

  if (points.length === 0 || max <= 0) {
    return (
      <div className={emptyStateClass} style={{ minHeight: Math.min(height, 96) }} role="img" aria-label={EMPTY_TEXT}>
        {EMPTY_TEXT}
      </div>
    );
  }

  const highlighted = new Set(highlightLabels ?? []);
  // With no highlight list every bar is the accent; with one, the rest recede
  // so the named bars read as the subject rather than as a different category.
  const colorFor = (label: string) =>
    highlighted.size === 0 || highlighted.has(label) ? accent : CHROME.muted;

  const summary = `Bar chart, ${points.length} categories. ${points
    .map((p) => `${p.label} ${valueFormat(p.value)}`)
    .join(", ")}.`;

  return horizontal ? (
    <HorizontalBars
      points={points}
      max={max}
      colorFor={colorFor}
      valueFormat={valueFormat}
      summary={summary}
      highlighted={highlighted}
    />
  ) : (
    <VerticalBars
      points={points}
      max={max}
      height={height}
      colorFor={colorFor}
      valueFormat={valueFormat}
      summary={summary}
      highlighted={highlighted}
    />
  );
}

type BodyProps = {
  points: Point[];
  max: number;
  colorFor: (label: string) => string;
  valueFormat: (n: number) => string;
  summary: string;
  highlighted: Set<string>;
};

function VerticalBars({ points, max, height, colorFor, valueFormat, summary }: BodyProps & { height: number }) {
  const pad = { top: 22, right: 8, bottom: 30, left: 8 };
  const plotH = height - pad.top - pad.bottom;
  const slot = (VB_W - pad.left - pad.right) / points.length;
  // A 2px surface gap between neighbouring fills; never wider than the slot.
  const barW = Math.max(4, Math.min(slot - 8, 64));

  return (
    <svg
      viewBox={`0 0 ${VB_W} ${height}`}
      preserveAspectRatio="xMidYMid meet"
      style={{ width: "100%", height: "auto" }}
      role="img"
      aria-label={summary}
    >
      <line
        x1={pad.left}
        x2={VB_W - pad.right}
        y1={pad.top + plotH}
        y2={pad.top + plotH}
        stroke={CHROME.axis}
        strokeWidth={1}
      />
      {points.map((p, i) => {
        const h = (p.value / max) * plotH;
        const x = pad.left + i * slot + (slot - barW) / 2;
        const y = pad.top + plotH - h;
        return (
          <g key={`${p.label}-${i}`}>
            <path d={roundedTop(x, y, barW, h, 4)} fill={colorFor(p.label)}>
              <title>{`${p.label}: ${valueFormat(p.value)}`}</title>
            </path>
            <text x={x + barW / 2} y={y - 6} textAnchor="middle" fontSize={FONT} fill={CHROME.label}>
              {valueFormat(p.value)}
            </text>
            <text x={x + barW / 2} y={height - 8} textAnchor="middle" fontSize={FONT} fill={CHROME.label}>
              {truncate(p.label, Math.max(4, Math.floor(slot / 8)))}
            </text>
          </g>
        );
      })}
    </svg>
  );
}

/**
 * Horizontal bars are laid out in HTML rather than SVG: the category name is
 * ordinary wrapping text, which is exactly what a top-N list needs and what an
 * SVG `<text>` cannot do.
 */
function HorizontalBars({ points, max, colorFor, valueFormat, summary, highlighted }: BodyProps) {
  return (
    <ul className="m-0 list-none space-y-2 p-0" role="img" aria-label={summary}>
      {points.map((p, i) => (
        <li key={`${p.label}-${i}`} className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-3">
          <span className="truncate text-sm text-gray-900">
            {p.label}
            {highlighted.has(p.label) ? <span className="sr-only"> (highlighted)</span> : null}
          </span>
          <span className="text-sm font-medium tabular-nums text-gray-900">{valueFormat(p.value)}</span>
          <span className="col-span-2 h-2 w-full overflow-hidden rounded-full" style={{ background: CHROME.track }}>
            <span
              className="block h-full rounded-full"
              style={{ width: `${(p.value / max) * 100}%`, background: colorFor(p.label) }}
            />
          </span>
        </li>
      ))}
    </ul>
  );
}

/**
 * A bar path with only its data-end rounded — the baseline end stays square so
 * the bar sits flat on the axis. A plain `rx` would round all four corners and
 * lift the bar off its own baseline.
 */
function roundedTop(x: number, y: number, w: number, h: number, r: number): string {
  const radius = Math.min(r, w / 2, Math.max(h, 0));
  if (h <= 0) return "";
  return [
    `M${x} ${y + h}`,
    `V${y + radius}`,
    `Q${x} ${y} ${x + radius} ${y}`,
    `H${x + w - radius}`,
    `Q${x + w} ${y} ${x + w} ${y + radius}`,
    `V${y + h}`,
    "Z",
  ].join(" ");
}

function truncate(s: string, max: number): string {
  return s.length > max ? `${s.slice(0, Math.max(1, max - 1))}…` : s;
}
