/**
 * Hand-written inline-SVG charts for the admin console. No charting library on
 * purpose: the whole set is a few hundred lines, so a dependency (and its
 * bundle cost) would buy nothing these screens need.
 */
export { LineChart } from "./LineChart";
export { BarChart } from "./BarChart";
export { DonutChart } from "./DonutChart";
export { Sparkline } from "./Sparkline";
export { StatTile } from "./StatTile";
export { ProgressBar } from "./ProgressBar";

export {
  CATEGORICAL,
  ACCENT,
  CHROME,
  TONES,
  EMPTY_TEXT,
  emptyStateClass,
  seriesColor,
  clamp01,
  formatCount,
  formatCompact,
  formatPercent,
  isPlottable,
} from "./palette";

export type { Point, SeriesPoint, Tone } from "./palette";
