"use client";

import { useId, useMemo, useState } from "react";

import {
  TIMEFRAMES,
  formatDateTime,
  formatPct,
  formatShortDate,
  withinTimeframe,
  type PerformancePoint,
  type PerformanceSeries,
  type TimeframeId,
} from "@/lib/dashboard-metrics";
import { Badge, Eyebrow, SectionHeading, SubText, lightCard, luxButton } from "./shell";
import { LUX, VizEmpty, barPath } from "./viz";

// The main graph: one column per graded assessment attempt, over `submitted_at`.
//
// Form choice: columns, not a line. The readings are discrete events a few days
// apart, not a continuous quantity sampled at a regular interval — a line drawn
// between two attempts would imply the student's score passed through every
// value in between, which nobody measured.
//
// One series, so there is no legend: the heading says what is plotted. The best
// attempt is picked out in gold (emphasis, per the dataviz skill's "one series is
// the point, the rest are context" rule) and also carries a direct label and a
// "Best" badge in the table view, so the emphasis never rests on hue alone.
//
// The y-axis is pinned to 0..100 rather than fitted to the data. A fitted axis
// makes 20% and 30% look like a dramatic climb; percentages have a real domain
// and using it is the honest framing.

const VB_W = 640;
const VB_H = 240;
const PAD = { top: 26, right: 14, bottom: 36, left: 38 };
const PLOT_W = VB_W - PAD.left - PAD.right;
const PLOT_H = VB_H - PAD.top - PAD.bottom;

const TICKS = [0, 25, 50, 75, 100];

type Hover = { index: number; x: number; y: number } | null;

export function PerformanceChart({
  series,
  /** Injected rather than read here, so the filter is pure and testable. */
  nowMs,
  courseLabel,
}: {
  series: PerformanceSeries;
  nowMs: number;
  courseLabel: string | null;
}) {
  const [timeframe, setTimeframe] = useState<TimeframeId>("all");
  const [showTable, setShowTable] = useState(false);
  const [hover, setHover] = useState<Hover>(null);
  const tableId = useId();

  const visible = useMemo(
    () => withinTimeframe(series.points, timeframe, nowMs),
    [series.points, timeframe, nowMs],
  );

  const best = useMemo(
    () =>
      visible.reduce<PerformancePoint | null>(
        (acc, p) => (acc === null || p.percentage > acc.percentage ? p : acc),
        null,
      ),
    [visible],
  );

  const c = LUX.light;

  return (
    <section aria-labelledby="performance-heading" id="performance" className={`${lightCard} p-5 sm:p-6`}>
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0">
          <Eyebrow>Assessment performance</Eyebrow>
          <SectionHeading id="performance-heading">Scores over time</SectionHeading>
          <div className="mt-1">
            <SubText>
              Every marked attempt as a percentage of its total
              {courseLabel === null ? "" : `, for ${courseLabel}`}.
            </SubText>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <TimeframeFilter value={timeframe} onChange={setTimeframe} />
          <button
            type="button"
            onClick={() => setShowTable((open) => !open)}
            aria-expanded={showTable}
            aria-controls={tableId}
            className={luxButton("quiet")}
          >
            {showTable ? "Hide values" : "Show values"}
          </button>
        </div>
      </div>

      {/* Three distinct outcomes, and they must not be collapsed into one: no
          attempts at all, no attempts *in this window*, and attempts that exist
          but are not marked. Each tells the student something different to do. */}
      {series.points.length === 0 ? (
        <div className="mt-5">
          <VizEmpty
            kind="empty"
            title={
              series.ungradedCount > 0
                ? "No marked attempts yet"
                : "No assessment attempts yet"
            }
            hint={
              series.ungradedCount > 0
                ? `You have ${series.ungradedCount} attempt${series.ungradedCount === 1 ? "" : "s"} waiting to be marked. Scores appear here once they are graded.`
                : "Your scores appear here after you submit your first assessment."
            }
          />
        </div>
      ) : visible.length === 0 ? (
        <div className="mt-5">
          <VizEmpty
            kind="empty"
            title="No attempts in this period"
            hint="You have marked attempts, just none inside the selected timeframe. Widen it to see them."
            action={
              <button type="button" onClick={() => setTimeframe("all")} className={luxButton("quiet")}>
                Show all time
              </button>
            }
          />
        </div>
      ) : (
        <>
          <div className="relative mt-5">
            <svg
              viewBox={`0 0 ${VB_W} ${VB_H}`}
              preserveAspectRatio="xMidYMid meet"
              role="img"
              aria-label={chartSummary(visible, best)}
              className="w-full"
              style={{ height: "auto" }}
              onMouseLeave={() => setHover(null)}
            >
              {/* Hairline, solid, recessive — never dashed. */}
              {TICKS.map((t) => {
                const y = PAD.top + PLOT_H - (t / 100) * PLOT_H;
                return (
                  <g key={t}>
                    <line
                      x1={PAD.left}
                      x2={PAD.left + PLOT_W}
                      y1={y}
                      y2={y}
                      stroke={t === 0 ? c.axis : c.grid}
                      strokeWidth={1}
                    />
                    <text
                      x={PAD.left - 8}
                      y={y + 4}
                      textAnchor="end"
                      fontSize={11}
                      fill="#56534c"
                      style={{ fontVariantNumeric: "tabular-nums" }}
                    >
                      {t}%
                    </text>
                  </g>
                );
              })}

              {visible.map((point, i) => {
                const slot = PLOT_W / visible.length;
                // Capped at 24px and never allowed to fill its slot: the
                // leftover band is the 2px-plus surface gap that separates
                // neighbours without drawing a stroke around them.
                const barW = Math.max(6, Math.min(slot - 14, 24));
                const x = PAD.left + i * slot + (slot - barW) / 2;
                const h = (point.percentage / 100) * PLOT_H;
                const y = PAD.top + PLOT_H - h;
                const isBest = best !== null && point.attemptId === best.attemptId;
                const isHovered = hover?.index === i;

                return (
                  <g key={point.attemptId}>
                    {/* A genuine 0% has no bar to draw, so it gets a 2px stub at
                        the baseline — visibly "measured and zero", which is a
                        different statement from an absent column. */}
                    <path
                      d={barPath(x, point.percentage <= 0 ? PAD.top + PLOT_H - 2 : y, barW, point.percentage <= 0 ? 2 : h)}
                      fill={isBest ? c.emphasis : c.series}
                      opacity={isHovered || hover === null ? 1 : 0.55}
                    />

                    {/* Only the best attempt is directly labelled. A number on
                        every column is chaos and goes unread; the axis, the
                        tooltip and the table view carry the rest. */}
                    {isBest ? (
                      <text
                        x={x + barW / 2}
                        y={Math.max(y - 8, 12)}
                        textAnchor="middle"
                        fontSize={12}
                        fontWeight={700}
                        fill="#3d3a34"
                      >
                        {formatPct(point.percentage)}
                      </text>
                    ) : null}

                    <text
                      x={x + barW / 2}
                      y={PAD.top + PLOT_H + 16}
                      textAnchor="middle"
                      fontSize={11}
                      fill="#56534c"
                    >
                      {formatShortDate(point.submittedAt)}
                    </text>
                    {/* #6b675d, not a lighter grey: at 10px this is small text
                        and has to clear 4.5:1 on the white card (measured
                        5.57:1). The earlier #8a857b measured 3.67:1 and failed. */}
                    <text
                      x={x + barW / 2}
                      y={PAD.top + PLOT_H + 29}
                      textAnchor="middle"
                      fontSize={10}
                      fill="#6b675d"
                    >
                      #{point.attemptNo}
                    </text>

                    {/* The hit target is the whole slot, not the 24px mark —
                        a thin bar is close to unhoverable otherwise. Focusable
                        so the tooltip is reachable by keyboard, with the value
                        in the accessible name rather than the visual tooltip
                        alone. */}
                    <rect
                      x={PAD.left + i * slot}
                      y={PAD.top}
                      width={slot}
                      height={PLOT_H}
                      fill="transparent"
                      tabIndex={0}
                      role="button"
                      aria-label={pointSummary(point, isBest)}
                      onMouseEnter={() => setHover({ index: i, x: x + barW / 2, y })}
                      onFocus={() => setHover({ index: i, x: x + barW / 2, y })}
                      onBlur={() => setHover(null)}
                      className="cursor-default focus-visible:outline-none"
                    />
                    {isHovered ? (
                      <rect
                        x={PAD.left + i * slot + 1}
                        y={PAD.top}
                        width={slot - 2}
                        height={PLOT_H}
                        fill="none"
                        stroke={c.axis}
                        strokeWidth={1}
                        rx={6}
                      />
                    ) : null}
                  </g>
                );
              })}
            </svg>

            {hover !== null && visible[hover.index] !== undefined ? (
              <Tooltip point={visible[hover.index]} xRatio={hover.x / VB_W} />
            ) : null}
          </div>

          <div className="mt-4 flex flex-wrap items-center gap-x-4 gap-y-2">
            <SubText>
              Average of shown attempts:{" "}
              <span className="font-semibold text-lux-ink-900">
                {formatPct(averageOf(visible), 1)}
              </span>
            </SubText>
            {/* Excluded rows are reported, never plotted at zero. */}
            {series.ungradedCount > 0 ? (
              <Badge tone="neutral">
                {series.ungradedCount} attempt{series.ungradedCount === 1 ? "" : "s"} not marked —
                not plotted
              </Badge>
            ) : null}
          </div>

          <div id={tableId} hidden={!showTable} className="mt-4 overflow-x-auto">
            <table className="w-full min-w-[420px] border-collapse text-left text-[13.5px]">
              <caption className="sr-only">
                Every marked attempt in the selected timeframe, with its score.
              </caption>
              <thead>
                <tr className="border-b border-lux-cream-300 text-[12px] uppercase tracking-[0.06em] text-lux-ink-600">
                  <th scope="col" className="py-2 pr-3 font-semibold">Assessment</th>
                  <th scope="col" className="py-2 pr-3 font-semibold">Attempt</th>
                  <th scope="col" className="py-2 pr-3 font-semibold">Submitted</th>
                  <th scope="col" className="py-2 text-right font-semibold">Score</th>
                </tr>
              </thead>
              <tbody>
                {visible.map((point) => (
                  <tr key={point.attemptId} className="border-b border-lux-cream-200 last:border-0">
                    <td className="py-2 pr-3 text-lux-ink-900">{point.examTitle}</td>
                    <td className="py-2 pr-3 tabular-nums text-lux-ink-700">#{point.attemptNo}</td>
                    <td className="py-2 pr-3 text-lux-ink-700">{formatDateTime(point.submittedAt)}</td>
                    <td className="py-2 text-right font-semibold tabular-nums text-lux-ink-900">
                      {formatPct(point.percentage)}
                      {best !== null && point.attemptId === best.attemptId ? (
                        <span className="ml-2 align-middle">
                          <Badge tone="gold">Best</Badge>
                        </span>
                      ) : null}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </section>
  );
}

// --------------------------------------------------------------- sub-pieces

function TimeframeFilter({
  value,
  onChange,
}: {
  value: TimeframeId;
  onChange: (next: TimeframeId) => void;
}) {
  // A radiogroup rather than a select: four options, and the current one should
  // be visible without opening anything. Each option filters real rows — there
  // is no option here that is cosmetic.
  return (
    <div
      role="radiogroup"
      aria-label="Timeframe"
      className="inline-flex rounded-full border border-lux-cream-300 bg-lux-cream-200/70 p-0.5"
    >
      {TIMEFRAMES.map((frame) => {
        const selected = frame.id === value;
        return (
          <button
            key={frame.id}
            type="button"
            role="radio"
            aria-checked={selected}
            onClick={() => onChange(frame.id)}
            className={`rounded-full px-3 py-1.5 text-[13px] font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#9c7a24] focus-visible:ring-offset-1 ${
              selected
                ? "bg-white text-lux-ink-900 shadow-[0_1px_2px_rgba(28,27,24,0.12)]"
                : "text-lux-ink-600 hover:text-lux-ink-900"
            }`}
          >
            {frame.label}
          </button>
        );
      })}
    </div>
  );
}

/**
 * The hover/focus tooltip.
 *
 * Positioned in percentage units over the SVG, so it tracks the column after the
 * chart has been scaled to its container. Clamped away from both edges — a
 * tooltip on the last column would otherwise hang outside the card.
 */
function Tooltip({ point, xRatio }: { point: PerformancePoint; xRatio: number }) {
  const clamped = Math.min(Math.max(xRatio, 0.08), 0.92);

  return (
    <div
      role="status"
      className="pointer-events-none absolute top-0 z-10 w-max max-w-[230px] -translate-x-1/2 rounded-[11px] border border-lux-char-700 bg-lux-char-900 px-3 py-2 text-left shadow-lg"
      style={{ left: `${clamped * 100}%` }}
    >
      <p className="text-[13px] font-semibold leading-[1.35] text-lux-mist-100">{point.examTitle}</p>
      <p className="mt-0.5 text-[12px] leading-[1.45] text-lux-mist-400">
        {point.courseCode} · attempt #{point.attemptNo}
      </p>
      <p className="mt-1 font-display text-[18px] font-bold leading-none text-[#ddb463]">
        {formatPct(point.percentage)}
      </p>
      <p className="mt-1 text-[12px] leading-[1.45] text-lux-mist-400">
        {formatDateTime(point.submittedAt)}
      </p>
    </div>
  );
}

function averageOf(points: PerformancePoint[]): number | null {
  if (points.length === 0) return null;
  return points.reduce((sum, p) => sum + p.percentage, 0) / points.length;
}

function pointSummary(point: PerformancePoint, isBest: boolean): string {
  return `${point.examTitle}, attempt ${point.attemptNo}, ${formatPct(point.percentage)}, submitted ${formatDateTime(point.submittedAt)}${isBest ? ". Best score shown." : ""}`;
}

/** The whole chart as one sentence, for a reader that does not see the marks. */
function chartSummary(points: PerformancePoint[], best: PerformancePoint | null): string {
  const head = `Column chart of ${points.length} marked assessment attempt${points.length === 1 ? "" : "s"} as percentages.`;
  const body = points
    .map((p) => `${formatShortDate(p.submittedAt)} attempt ${p.attemptNo} ${formatPct(p.percentage)}`)
    .join("; ");
  const tail = best === null ? "" : ` Best: ${formatPct(best.percentage)}.`;
  return `${head} ${body}.${tail}`;
}
