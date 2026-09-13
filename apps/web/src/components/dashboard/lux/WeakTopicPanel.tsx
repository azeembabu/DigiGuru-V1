import Link from "next/link";

import type { WeakTopic } from "@/lib/dashboard-metrics";
import { Eyebrow, SectionHeading, SubText, darkCard, luxButton } from "./shell";
import { LUX, VizEmpty, barPath } from "./viz";

// The dark feature widget, bottom-right.
//
// SUBSTITUTION. The brief asked for an "interactive learning map by region".
// There is no geographic data anywhere in Digi Guru — not on a student, not on an
// LSC in any form this endpoint exposes, not on a session — so a map would have
// to be fabricated wholesale, and a fabricated map is the most convincing kind of
// lie a dashboard can tell.
//
// The honest version of the same idea ("where should I look?") is already in the
// payload: every graded attempt carries `weak_topics`, written by the grader.
// Counted across attempts and ranked most-missed first, that is a genuinely
// actionable panel built entirely from real rows.
//
// A plain server component: it has no state, no hover, and nothing to hydrate.

const BAR_H = 10;
const ROW_H = 34;
const LABEL_W = 0; // labels are HTML beside the bars, not SVG text

export function WeakTopicPanel({
  topics,
  attemptCount,
  filter,
}: {
  topics: WeakTopic[];
  /** Graded attempts the topics were counted from, for the honest subtitle. */
  attemptCount: number;
  filter: string;
}) {
  const c = LUX.dark;
  const shown = topics.slice(0, 6);
  const max = shown.reduce((m, t) => Math.max(m, t.count), 0);

  return (
    <section aria-labelledby="weak-heading" className={`${darkCard} flex min-w-0 flex-col p-5 sm:p-6`}>
      <Eyebrow dark>Focus next</Eyebrow>
      <SectionHeading dark id="weak-heading">
        Topics to revise
      </SectionHeading>
      <div className="mt-1">
        <SubText dark>
          {attemptCount === 0
            ? "Flagged by the grader on your marked attempts."
            : `Flagged across your ${attemptCount} marked attempt${attemptCount === 1 ? "" : "s"}, most-missed first.`}
        </SubText>
      </div>

      {topics.length === 0 ? (
        <div className="mt-5">
          <VizEmpty
            mode="dark"
            kind="empty"
            title={
              attemptCount === 0 ? "No marked attempts yet" : "No weak topics flagged"
            }
            hint={
              attemptCount === 0
                ? "Once an assessment is marked, the topics you missed are listed here."
                : "Nothing was flagged on your marked attempts. Keep going."
            }
          />
        </div>
      ) : shown.length === 0 ? (
        <div className="mt-5">
          <VizEmpty
            mode="dark"
            kind="empty"
            title="Nothing matches your filter"
            hint={`No flagged topic matches “${filter.trim()}”.`}
          />
        </div>
      ) : (
        <>
          {/* Horizontal bars: the right form for a ranked top-N with long
              category names, which get real room instead of a 45° rotation.
              One series, one hue — no value-ramp, which would double-encode the
              bar length as colour and burn the only free channel. */}
          <ul className="mt-5 flex flex-col gap-1">
            {shown.map((topic) => {
              const ratio = max > 0 ? topic.count / max : 0;
              return (
                <li key={topic.topic} className="flex items-center gap-3" style={{ minHeight: ROW_H }}>
                  <span className="min-w-0 flex-1 truncate text-[14px] leading-[1.4] text-lux-mist-100">
                    {topic.topic}
                  </span>

                  <svg
                    viewBox={`0 0 100 ${BAR_H}`}
                    preserveAspectRatio="none"
                    aria-hidden="true"
                    className="h-[10px] w-[38%] shrink-0"
                  >
                    <rect x={LABEL_W} y={0} width={100} height={BAR_H} rx={4} fill={c.track} />
                    <path d={barPath(0, 0, Math.max(ratio * 100, 3), BAR_H, false)} fill={c.series} />
                  </svg>

                  {/* The count as text beside every bar — this is a six-row
                      ranked list, so a direct label per row is legible rather
                      than chaos, and it means the ranking never depends on
                      comparing bar lengths by eye. */}
                  <span className="w-14 shrink-0 text-right text-[13px] font-semibold tabular-nums text-[#ddb463]">
                    {topic.count}×
                    <span className="sr-only">
                      {` flagged on ${topic.count} attempt${topic.count === 1 ? "" : "s"}`}
                    </span>
                  </span>
                </li>
              );
            })}
          </ul>

          {topics.length > shown.length ? (
            <div className="mt-3">
              <SubText dark>
                {topics.length - shown.length} more topic
                {topics.length - shown.length === 1 ? "" : "s"} flagged.
              </SubText>
            </div>
          ) : null}

          <div className="mt-5">
            <Link href="/classroom" className={luxButton("onDark")}>
              Revise in the classroom
            </Link>
          </div>
        </>
      )}
    </section>
  );
}
