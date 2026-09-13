"use client";

import { useEffect, useState } from "react";

import { Pill, SectionHeader, cardClass } from "@/components/student/parts";
import type { QuotaStatus } from "@/lib/student";

// NN-3, rendered. Twenty minutes of active voice per student per calendar day,
// and the day boundary is midnight in the STUDENT's own timezone.
//
// Everything on this card is a server fact. `minutes_used`, `ms_remaining`,
// `is_locked` and `resets_at` all come off the Redis ledger via
// `GET /student/dashboard`; this component does no arithmetic on the quota
// beyond turning `ms` into a ring length. In particular it never derives the
// reset boundary from the browser's clock or timezone — a student's device set
// to the wrong zone would otherwise be told the wrong reset hour, and the whole
// point of NN-3 is that the zone is `students.timezone`.
//
// The one clock read is the "resets in N hours" countdown, which is a *display*
// of the server's `resets_at` instant: if the device clock is skewed the label
// is slightly off, but the quota itself is unaffected because the server owns
// it.

const RADIUS = 54;
const STROKE = 10;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

function hoursUntil(iso: string, now: number): number | null {
  const target = new Date(iso).getTime();
  if (Number.isNaN(target)) return null;
  const ms = target - now;
  if (ms <= 0) return 0;
  return Math.ceil(ms / 3_600_000);
}

function resetLabel(iso: string, now: number | null): string {
  if (now === null) return "Resets at midnight";
  const hours = hoursUntil(iso, now);
  if (hours === null) return "Resets at midnight";
  if (hours === 0) return "Resets in under an hour";
  return `Resets in ${hours} hour${hours === 1 ? "" : "s"}`;
}

export function QuotaRing({ quota }: { quota: QuotaStatus }) {
  // Read after mount, never during render: `Date.now()` in the render path is
  // a hydration mismatch waiting to happen, and the label is not worth one.
  const [now, setNow] = useState<number | null>(null);

  useEffect(() => {
    const tick = () => setNow(Date.now());
    // The first read is a timeout rather than a straight call: reading the clock
    // synchronously in the effect body would set state during the commit and
    // cascade a second render for the sake of a label.
    const first = setTimeout(tick, 0);
    // Re-read every minute so a dashboard left open overnight does not keep
    // claiming "resets in 9 hours" into the morning.
    const timer = setInterval(tick, 60_000);
    return () => {
      clearTimeout(first);
      clearInterval(timer);
    };
  }, []);

  const max = quota.minutes_max > 0 ? quota.minutes_max : 20;
  const used = Math.min(Math.max(quota.minutes_used, 0), max);
  const fraction = used / max;
  const remainingMinutes = Math.max(0, Math.ceil(quota.ms_remaining / 60_000));

  const tone = quota.is_locked ? "stroke-danger" : fraction >= 0.75 ? "stroke-amber-400" : "stroke-art-glow";

  return (
    <section className={cardClass} aria-labelledby="quota-heading">
      <div id="quota-heading">
        <SectionHeader
          title="Today’s speaking time"
          hint={`Counted only while you are speaking · ${quota.timezone}`}
        />
      </div>

      <div className="mt-5 flex flex-wrap items-center gap-6">
        <div className="relative shrink-0">
          <svg
            width={(RADIUS + STROKE) * 2}
            height={(RADIUS + STROKE) * 2}
            viewBox={`0 0 ${(RADIUS + STROKE) * 2} ${(RADIUS + STROKE) * 2}`}
            role="img"
            aria-label={`${used} of ${max} minutes used today`}
          >
            <circle
              cx={RADIUS + STROKE}
              cy={RADIUS + STROKE}
              r={RADIUS}
              fill="none"
              strokeWidth={STROKE}
              className="stroke-art-mid/70"
            />
            <circle
              cx={RADIUS + STROKE}
              cy={RADIUS + STROKE}
              r={RADIUS}
              fill="none"
              strokeWidth={STROKE}
              strokeLinecap="round"
              strokeDasharray={CIRCUMFERENCE}
              strokeDashoffset={CIRCUMFERENCE * (1 - fraction)}
              transform={`rotate(-90 ${RADIUS + STROKE} ${RADIUS + STROKE})`}
              className={`${tone} transition-[stroke-dashoffset] duration-500`}
            />
          </svg>
          {/* The number is in the DOM twice — once as the SVG's accessible
              label, once as visible text marked aria-hidden — so a screen
              reader hears it as one phrase rather than as loose digits. */}
          <div
            aria-hidden="true"
            className="absolute inset-0 flex flex-col items-center justify-center"
          >
            <span className="font-display text-[30px] font-bold leading-none tabular-nums text-white">
              {used}
            </span>
            <span className="mt-1 text-[12px] leading-none text-gray-300/70">of {max} min</span>
          </div>
        </div>

        <div className="min-w-0 flex-1 space-y-3">
          {quota.is_locked ? (
            <>
              <Pill tone="bad">Daily limit reached</Pill>
              <p className="text-[15px] leading-[1.6] text-gray-300">
                All {max} minutes of speaking time for today have been used. Your notes, flashcards
                and assessments remain available; only the live voice session is paused.
              </p>
            </>
          ) : (
            <>
              <Pill tone={fraction >= 0.75 ? "warn" : "good"}>
                {remainingMinutes} minute{remainingMinutes === 1 ? "" : "s"} left
              </Pill>
              <p className="text-[15px] leading-[1.6] text-gray-300">
                The clock runs only while you are speaking. Listening to the tutor and reading the
                whiteboard are not counted.
              </p>
            </>
          )}
          <p className="text-[14px] leading-[1.5] text-gray-300/70">
            {resetLabel(quota.resets_at, now)} — at midnight in your own timezone.
          </p>
        </div>
      </div>
    </section>
  );
}
