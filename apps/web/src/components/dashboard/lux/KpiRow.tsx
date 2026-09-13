"use client";

import { useEffect, useState } from "react";

import {
  formatPct,
  type AssessmentProgress,
  type Percentage,
  type RevisionBacklog,
  type SpeakingTime,
} from "@/lib/dashboard-metrics";
import { Badge, BigValue, Eyebrow, SubText, darkCard, lightCard } from "./shell";
import { Meter, MeterEmpty, MiniBars } from "./viz";

// The four KPI tiles. One dark contrast card, three light.
//
// Two of the four are substitutions, and the reason is the same in both cases:
// the metric the brief named does not exist in this product, and inventing a
// plausible number for it would be the single most damaging thing this screen
// could do.
//
//   "Overall attendance rate" -> Today's speaking time. There is no attendance
//   concept anywhere in Digi Guru; students are not marked present. The
//   server-authoritative 20-minute daily voice quota (NN-3) is the product's
//   signature constraint and the one figure that moves every session, which
//   earns it the dark card.
//
//   "Study hours logged" -> Revision due. A per-student cumulative session time
//   is not exposed on any `/student/*` endpoint — it exists only inside the
//   admin analytics roll-up — so there is no honest student-side source for it.
//
// Every tile renders an em-dash and a sentence when its figure is unmeasured.
// None of them renders `0` for "we do not know", because 0% is a real score a
// student can actually earn and showing it falsely misreports them to themselves.

function Tile({
  dark = false,
  children,
}: {
  dark?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div className={`${dark ? darkCard : lightCard} flex flex-col gap-3 p-5`}>{children}</div>
  );
}

// ------------------------------------------------------- 1. speaking time

function hoursUntil(iso: string, nowMs: number): number | null {
  const target = Date.parse(iso);
  if (!Number.isFinite(target)) return null;
  const ms = target - nowMs;
  if (ms <= 0) return 0;
  return Math.ceil(ms / 3_600_000);
}

/**
 * NN-3 on the dark contrast card.
 *
 * Every figure is a server fact off the Redis ledger. The component does no
 * quota arithmetic beyond turning minutes into a meter width, and it never
 * derives the reset boundary from the browser — `resets_at` is already midnight
 * in the student's own timezone, resolved server-side, and the countdown is a
 * *display* of that instant. A skewed device clock makes the "resets in N hours"
 * label slightly wrong and the quota itself not at all, which is the right way
 * round.
 */
function SpeakingTile({
  speaking,
  resetsAt,
  timezone,
}: {
  speaking: SpeakingTime;
  resetsAt: string;
  timezone: string;
}) {
  // Read after mount only: `Date.now()` during render is a hydration mismatch,
  // and this label is not worth one.
  const [nowMs, setNowMs] = useState<number | null>(null);

  useEffect(() => {
    const tick = () => setNowMs(Date.now());
    const first = setTimeout(tick, 0);
    // Re-read each minute so a dashboard left open overnight stops insisting it
    // resets in nine hours.
    const timer = setInterval(tick, 60_000);
    return () => {
      clearTimeout(first);
      clearInterval(timer);
    };
  }, []);

  const hours = nowMs === null ? null : hoursUntil(resetsAt, nowMs);
  const resetLabel =
    hours === null
      ? "Resets at midnight in your timezone"
      : hours === 0
        ? "Resets within the hour"
        : `Resets in ${hours} hour${hours === 1 ? "" : "s"}`;

  return (
    <Tile dark>
      <div className="flex items-start justify-between gap-3">
        <Eyebrow dark>Today’s speaking time</Eyebrow>
        {speaking.isLocked ? (
          <Badge dark tone="warn">
            Limit reached
          </Badge>
        ) : (
          <Badge dark tone="good">
            {speaking.remainingMinutes} min left
          </Badge>
        )}
      </div>

      <div>
        <BigValue dark>
          {speaking.usedMinutes}
          <span className="ml-1 font-sans text-[15px] font-semibold text-lux-mist-400">
            / {speaking.maxMinutes} min
          </span>
        </BigValue>
      </div>

      <Meter
        fraction={speaking.fraction}
        mode="dark"
        tone={speaking.isLocked ? "emphasis" : "series"}
        label={`${speaking.usedMinutes} of ${speaking.maxMinutes} minutes of speaking time used today`}
      />

      <SubText dark>
        Counted only while you are speaking. {resetLabel} — {timezone}.
      </SubText>
    </Tile>
  );
}

// --------------------------------------------------------- 2. revision due

function RevisionTile({ backlog }: { backlog: RevisionBacklog }) {
  // The two zeroes are different facts and get different copy: "you have saved
  // nothing yet" explains where flashcards come from, while "you are clear"
  // congratulates a cleared queue. A bare "0" would say neither.
  return (
    <Tile>
      <div className="flex items-start justify-between gap-3">
        <Eyebrow>Revision due</Eyebrow>
        {backlog.total > 0 ? <Badge tone="gold">Action needed</Badge> : null}
      </div>

      {backlog.nothingSavedYet ? (
        <>
          <BigValue>—</BigValue>
          <MeterEmpty mode="light" label="Nothing saved for revision yet" />
          <SubText>
            Nothing saved yet. Your tutor builds flashcards and revision reminders as you work
            through a block.
          </SubText>
        </>
      ) : (
        <>
          <BigValue>{backlog.total}</BigValue>
          <MiniBars
            values={[backlog.flashcardsDue, backlog.revisionsDue]}
            mode="light"
            label={`${backlog.flashcardsDue} flashcards and ${backlog.revisionsDue} saved notes due`}
          />
          <SubText>
            {backlog.flashcardsDue} flashcard{backlog.flashcardsDue === 1 ? "" : "s"} ·{" "}
            {backlog.revisionsDue} saved note{backlog.revisionsDue === 1 ? "" : "s"}
            {backlog.total === 0 ? " — your queue is clear." : " due for review."}
          </SubText>
        </>
      )}
    </Tile>
  );
}

// --------------------------------------------------- 3. assessments attempted

function AssessmentTile({ progress }: { progress: AssessmentProgress }) {
  // "Attempted", not "completed": `attempts_used > 0` is all the payload
  // supports, and a student who scored 0% on every assessment has still
  // attempted them all. Calling that "completed" would be a claim the data
  // does not make.
  return (
    <Tile>
      <Eyebrow>Assessments attempted</Eyebrow>

      {progress.total === 0 ? (
        <>
          <BigValue>—</BigValue>
          <MeterEmpty mode="light" label="No assessments published yet" />
          <SubText>
            No assessments have been published for your semester yet. This fills in when your
            centre releases one.
          </SubText>
        </>
      ) : (
        <>
          <BigValue>
            {progress.attempted}
            <span className="ml-1 font-sans text-[15px] font-semibold text-lux-ink-600">
              / {progress.total}
            </span>
          </BigValue>
          {progress.fraction === null ? (
            <MeterEmpty mode="light" label="No assessments to measure" />
          ) : (
            <Meter
              fraction={progress.fraction}
              mode="light"
              label={`${progress.attempted} of ${progress.total} published assessments attempted`}
            />
          )}
          <SubText>
            Counts an assessment once you have started it at least once — not a mark or a pass.
          </SubText>
        </>
      )}
    </Tile>
  );
}

// ------------------------------------------------------- 4. course progress

function ProgressTile({
  progress,
  courseTitle,
}: {
  progress: Percentage;
  courseTitle: string | null;
}) {
  // The brief's "course completion index" under its honest name. It is exactly
  // `continue_learning.progress_percentage` — blocks with a completed session
  // over blocks in the course — and is not a composite of anything.
  return (
    <Tile>
      <div className="flex items-start justify-between gap-3">
        <Eyebrow>Course progress</Eyebrow>
        {progress !== null && progress >= 100 ? <Badge tone="good">Blocks complete</Badge> : null}
      </div>

      {progress === null ? (
        <>
          <BigValue>—</BigValue>
          <MeterEmpty mode="light" label="No session started yet" />
          <SubText>
            Not started. Your progress appears after your first classroom session.
          </SubText>
        </>
      ) : (
        <>
          <BigValue>{formatPct(progress)}</BigValue>
          <Meter
            fraction={progress / 100}
            mode="light"
            label={`${formatPct(progress)} of the blocks in this course completed`}
          />
          <SubText>
            Blocks completed in {courseTitle ?? "your current course"}.
          </SubText>
        </>
      )}
    </Tile>
  );
}

// ------------------------------------------------------------------- the row

export function KpiRow({
  speaking,
  resetsAt,
  timezone,
  backlog,
  progress,
  courseProgressValue,
  courseTitle,
}: {
  speaking: SpeakingTime;
  resetsAt: string;
  timezone: string;
  backlog: RevisionBacklog;
  progress: AssessmentProgress;
  courseProgressValue: Percentage;
  courseTitle: string | null;
}) {
  return (
    // Stacks to one column under 640px so the 400px target never scrolls
    // sideways, and the dark tile stays first in both orders — it is the one
    // figure that changes every session.
    <section aria-labelledby="kpi-heading" id="revision">
      <h2 id="kpi-heading" className="sr-only">
        Today at a glance
      </h2>
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <SpeakingTile speaking={speaking} resetsAt={resetsAt} timezone={timezone} />
        <RevisionTile backlog={backlog} />
        <AssessmentTile progress={progress} />
        <ProgressTile progress={courseProgressValue} courseTitle={courseTitle} />
      </div>
    </section>
  );
}
