import { ACCENT, CHROME, formatCount } from "./palette";

type SparklineProps = {
  data: number[];
  width?: number;
  height?: number;
  accent?: string;
};

/**
 * A tiny inline trend — no axes, no labels, no legend. It sits beside a number
 * and says "rising" or "flat", nothing more precise than that.
 */
export function Sparkline({ data, width = 96, height = 24, accent = ACCENT }: SparklineProps) {
  const values = data.filter((n) => Number.isFinite(n));
  const pad = 2;
  const w = Math.max(width, 8);
  const h = Math.max(height, 8);

  // Nothing to trend, a single reading, or a perfectly flat series all get the
  // same calm baseline. A zero range would otherwise divide by zero and a
  // single point would draw an invisible zero-length path.
  const min = values.length > 0 ? Math.min(...values) : 0;
  const max = values.length > 0 ? Math.max(...values) : 0;
  const range = max - min;
  const flat = values.length < 2 || range <= 0;

  const label = flat
    ? values.length === 0
      ? "Trend: no data yet"
      : `Trend: flat at ${formatCount(values[0])}`
    : `Trend: ${formatCount(values[0])} to ${formatCount(values[values.length - 1])}`;

  const d = flat
    ? `M${pad} ${h / 2} L${w - pad} ${h / 2}`
    : values
        .map((v, i) => {
          const x = pad + (i / (values.length - 1)) * (w - pad * 2);
          const y = pad + (1 - (v - min) / range) * (h - pad * 2);
          return `${i === 0 ? "M" : "L"}${x.toFixed(2)} ${y.toFixed(2)}`;
        })
        .join(" ");

  return (
    <svg
      viewBox={`0 0 ${w} ${h}`}
      preserveAspectRatio="xMidYMid meet"
      style={{ width: "100%", maxWidth: w, height: "auto" }}
      role="img"
      aria-label={label}
    >
      <path
        d={d}
        fill="none"
        stroke={flat ? CHROME.muted : accent}
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}
