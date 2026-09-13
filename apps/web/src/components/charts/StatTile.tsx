import Link from "next/link";

import { Sparkline } from "./Sparkline";
import { TONES, formatCount, type Tone } from "./palette";

type StatTileProps = {
  label: string;
  value: string | number;
  hint?: string;
  tone?: Tone;
  trend?: number[];
  /**
   * Optional drill-down. When present the whole tile becomes a real link to the
   * records behind the number, so open-in-new-tab and keyboard focus work; when
   * absent the tile renders exactly as it always has.
   */
  href?: string;
  /** The link's accessible sentence. Falls back to "View <label>: <value>". */
  linkLabel?: string;
};

/**
 * One KPI on a white admin card.
 *
 * A numeric `value` is formatted here rather than by the caller, so every tile
 * on the dashboard groups its thousands the same way and a stray `NaN` from an
 * empty-database division shows as an em-dash instead of leaking to the screen.
 */
export function StatTile({
  label,
  value,
  hint,
  tone = "default",
  trend,
  href,
  linkLabel,
}: StatTileProps) {
  const { text, fill, word } = TONES[tone];
  const display = typeof value === "number" ? formatCount(value) : value;
  const hasTrend = Array.isArray(trend) && trend.some((n) => Number.isFinite(n));
  // A zero that links is still worth opening — an empty filtered list answers
  // the question — but it should not read as loudly as a real count.
  const isZero = display === "0";

  const body = (
    <>
      <div className="flex items-center gap-2">
        {/* Tone is a dot plus a screen-reader word, never the colour alone. */}
        {tone !== "default" ? (
          <span aria-hidden="true" className="size-2 shrink-0 rounded-full" style={{ background: fill }} />
        ) : null}
        <p className="min-w-0 truncate text-sm text-gray-500">{label}</p>
      </div>

      <p
        className={`mt-1 font-display text-3xl font-semibold tabular-nums ${
          isZero ? "text-gray-500/70" : ""
        }`}
        style={isZero ? undefined : { color: text }}
      >
        {display}
        {word ? <span className="sr-only"> — {word}</span> : null}
      </p>

      {hint ? <p className="mt-1 text-sm text-gray-500">{hint}</p> : null}

      {hasTrend ? (
        <div className="mt-3">
          <Sparkline data={trend} accent={fill} />
        </div>
      ) : null}
    </>
  );

  const shell =
    "rounded-md border border-lavender-200 bg-white p-5 shadow-[0_1px_2px_rgba(10,12,22,0.05)]";

  if (!href) return <div className={shell}>{body}</div>;

  return (
    <Link
      href={href}
      aria-label={linkLabel ?? `View ${label}: ${display}`}
      className={`${shell} block transition-colors hover:border-indigo-400 hover:bg-lavender-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-lavender-50`}
    >
      {body}
    </Link>
  );
}
