// Stars and trophies — the presentation half of `crates/core/src/performance.rs`.
//
// These components render a rating; they never decide one. `stars` and
// `trophy` arrive already computed by the gateway, because the thresholds are
// a pedagogical decision rather than a rendering detail: if the browser banded
// the percentages, the student's page and the admin's could quietly disagree
// about the same block.
//
// The one rule they do enforce is the empty state. `stars === null` means the
// block was never assessed, which is NOT zero stars — drawing five grey stars
// against a paper the student never sat reads as a failure they did not earn.

import type { Trophy } from "@/lib/student";
import { TROPHY_LABEL, TROPHY_TONE } from "@/lib/student";

const MAX_STARS = 5;

/**
 * Five stars, `filled` of them lit.
 *
 * `null` renders the not-assessed state instead of an empty row, so the two
 * cases are distinguishable at a glance and to a screen reader.
 */
export function StarRating({
  stars,
  size = "md",
  label,
}: {
  stars: number | null;
  size?: "sm" | "md";
  label?: string;
}) {
  const dimension = size === "sm" ? "h-3.5 w-3.5" : "h-5 w-5";

  if (stars === null) {
    return (
      <span className="text-xs text-slate-500 italic" title="No graded exam yet">
        Not assessed yet
      </span>
    );
  }

  const filled = Math.max(0, Math.min(MAX_STARS, stars));
  return (
    <span
      className="inline-flex items-center gap-0.5"
      // One label for the group, rather than five separate star images each
      // announcing itself — a screen reader should hear the rating, not count.
      role="img"
      aria-label={label ?? `${filled} out of ${MAX_STARS} stars`}
    >
      {Array.from({ length: MAX_STARS }, (_, index) => (
        <svg
          key={index}
          viewBox="0 0 20 20"
          aria-hidden="true"
          className={`${dimension} ${
            index < filled ? "text-amber-300" : "text-slate-700"
          }`}
          fill="currentColor"
        >
          <path d="M10 1.6l2.47 5.35 5.86.72-4.33 4.02 1.14 5.79L10 14.66l-5.14 2.82 1.14-5.79L1.67 7.67l5.86-.72L10 1.6z" />
        </svg>
      ))}
    </span>
  );
}

/**
 * The trophy, or the progress toward it.
 *
 * A missing trophy is explained rather than left blank: `attempts_until_trophy`
 * tells the student how many more graded papers it takes, which is the
 * difference between "you have not earned this" and "this is unreachable".
 */
export function TrophyBadge({
  trophy,
  attemptsUntil,
}: {
  trophy: Trophy | null;
  attemptsUntil: number;
}) {
  if (trophy === null) {
    if (attemptsUntil <= 0) {
      // Enough papers sat, average below the bronze band. Saying so beats an
      // empty space, and there is deliberately no consolation medal — a trophy
      // for 30% is not an honest signal.
      return (
        <span className="text-xs text-slate-400">
          Keep going — a higher average earns a trophy
        </span>
      );
    }
    return (
      <span className="text-xs text-slate-400">
        {attemptsUntil} more graded {attemptsUntil === 1 ? "exam" : "exams"} to earn a trophy
      </span>
    );
  }

  return (
    <span className={`inline-flex items-center gap-1.5 text-sm font-semibold ${TROPHY_TONE[trophy]}`}>
      <svg viewBox="0 0 24 24" aria-hidden="true" className="h-5 w-5" fill="currentColor">
        <path d="M7 4h10v1h3v3a4 4 0 0 1-3.6 3.98A5.01 5.01 0 0 1 13 14.9V17h3v3H8v-3h3v-2.1a5.01 5.01 0 0 1-3.4-2.92A4 4 0 0 1 4 8V5h3V4zm0 3H5.5v1A2.5 2.5 0 0 0 7 9.29V7zm10 0v2.29A2.5 2.5 0 0 0 18.5 8V7H17z" />
      </svg>
      {TROPHY_LABEL[trophy]}
    </span>
  );
}
