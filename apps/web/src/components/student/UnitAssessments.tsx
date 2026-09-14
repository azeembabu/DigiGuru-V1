"use client";

// The tutor's verdicts on the student's conversational performance, unit by unit.
//
// These are NOT exam results. Exam marks come from a stored answer key and live
// on the block cards above; these are the tutor's judgement of how the student
// engaged in a session. The two are visually distinct and labelled, because a
// student who mistakes a conversation rating for a graded paper has been
// misled by the UI, not by the tutor.
//
// Three things every card must keep straight:
//
//   * A unit with no verdict shows "not assessed yet", never zero stars.
//   * A unit never opened shows as such, so the student can see what is left.
//   * The EVIDENCE is shown next to the rating — turns, questions, minutes —
//     because a rating a student cannot interrogate is one they cannot trust.

import { useCallback, useEffect, useState } from "react";

import { ApiError } from "@/lib/api";
import {
  SchemaError,
  loadUnitAssessments,
  type UnitAssessment,
  type UnitCard,
} from "@/lib/student";
import {
  PanelSkeleton,
  SectionHeader,
  StudentEmpty,
  StudentError,
  cardClass,
  formatDateTime,
  formatDuration,
} from "@/components/student/parts";
import { StarRating, TrophyBadge } from "@/components/student/StarRating";

type Outcome =
  | { status: "error"; message: string; retryable: boolean }
  | { status: "ready"; units: UnitCard[] };

export function UnitAssessments() {
  const [attempt, setAttempt] = useState(0);
  const [result, setResult] = useState<{ attempt: number; outcome: Outcome } | null>(null);

  useEffect(() => {
    let cancelled = false;
    const settle = (outcome: Outcome) => {
      if (!cancelled) setResult({ attempt, outcome });
    };

    loadUnitAssessments()
      .then((units) => settle({ status: "ready", units }))
      .catch((caught: unknown) => {
        if (caught instanceof ApiError) {
          settle({ status: "error", message: caught.message, retryable: true });
        } else if (caught instanceof SchemaError) {
          settle({ status: "error", message: caught.message, retryable: false });
        } else {
          settle({
            status: "error",
            message: "Your unit feedback could not be loaded.",
            retryable: true,
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [attempt]);

  const retry = useCallback(() => setAttempt((n) => n + 1), []);

  if (result === null || result.attempt !== attempt) {
    return <PanelSkeleton className="h-64" />;
  }

  if (result.outcome.status === "error") {
    return (
      <StudentError
        title="Unit feedback could not be loaded"
        message={result.outcome.message}
        onRetry={result.outcome.retryable ? retry : undefined}
      />
    );
  }

  const { units } = result.outcome;
  // Studied units first: a student opens this page to read feedback, and a
  // long list of never-opened units above it buries the thing they came for.
  const studied = units.filter((unit) => unit.sessions_total > 0);
  const untouched = units.filter((unit) => unit.sessions_total === 0);

  return (
    <section className="space-y-3">
      <SectionHeader
        title="Unit feedback from your tutor"
        hint="How you took part in each conversation. This is separate from your exam marks."
      />

      {units.length === 0 ? (
        <StudentEmpty
          title="No units yet"
          hint="Units appear here once your courses have material uploaded."
        />
      ) : (
        <>
          {studied.length === 0 ? (
            <StudentEmpty
              title="You have not studied a unit yet"
              hint="Open a unit in the classroom and talk with your tutor — your feedback appears here afterwards."
            />
          ) : (
            <ul className="space-y-3">
              {studied.map((unit) => (
                <li key={unit.document_id}>
                  <UnitAssessmentCard unit={unit} />
                </li>
              ))}
            </ul>
          )}

          {untouched.length > 0 ? (
            <details className={`${cardClass} p-4`}>
              <summary className="cursor-pointer text-[14px] font-semibold text-white">
                Units you have not opened yet ({untouched.length})
              </summary>
              <ul className="mt-3 space-y-1.5">
                {untouched.map((unit) => (
                  <li key={unit.document_id} className="text-[14px] text-gray-300">
                    {unit.unit_title}
                    <span className="text-gray-500"> · {unit.course_code}</span>
                    {/* A unit with no vectors cannot be taught. Saying so beats
                        letting the student open it and find the tutor abstains
                        on every question. */}
                    {!unit.is_ready ? (
                      <span className="ml-2 text-[12px] text-amber-300">not ready yet</span>
                    ) : null}
                  </li>
                ))}
              </ul>
            </details>
          ) : null}
        </>
      )}
    </section>
  );
}

function UnitAssessmentCard({ unit }: { unit: UnitCard }) {
  return (
    <article className={`${cardClass} p-4`}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="text-[12px] font-semibold uppercase tracking-wide text-gray-400">
            {unit.course_code} · Block {unit.block_no}
          </p>
          <h3 className="mt-0.5 text-[16px] font-semibold text-white">{unit.unit_title}</h3>
          {/* The usage line: when they last studied it and for how long. */}
          <p className="mt-1 text-[13px] text-gray-400">
            Last studied {formatDateTime(unit.last_studied_at)} ·{" "}
            {unit.sessions_total} {unit.sessions_total === 1 ? "session" : "sessions"} ·{" "}
            {formatDuration(unit.active_voice_ms)}
          </p>
        </div>

        <div className="flex flex-col items-end gap-1.5">
          <StarRating
            stars={unit.latest?.stars ?? null}
            size="sm"
            label={
              unit.latest
                ? `${unit.unit_title}: ${unit.latest.stars} of 5 stars`
                : `${unit.unit_title}: not assessed`
            }
          />
          {unit.latest ? (
            <>
              <span className="text-[15px] font-bold text-white">
                {unit.latest.mark.toFixed(0)}
                <span className="text-[12px] font-normal text-gray-400">/100</span>
              </span>
              {unit.latest.trophy ? (
                <TrophyBadge trophy={unit.latest.trophy} attemptsUntil={0} />
              ) : null}
            </>
          ) : null}
        </div>
      </div>

      {unit.latest ? (
        <Verdict latest={unit.latest} bestMark={unit.best_mark} count={unit.assessments_count} />
      ) : (
        // Studied but not yet assessed. Distinct from "never opened", and
        // explained rather than left blank — the usual cause is a session too
        // short to judge, which the student can act on.
        <p className="mt-3 border-t border-art-mid/60 pt-3 text-[14px] leading-[1.55] text-gray-300/80">
          No feedback yet. Your tutor writes this after a session where you have
          talked with it for a while — a very short session is not rated.
        </p>
      )}
    </article>
  );
}

function Verdict({
  latest,
  bestMark,
  count,
}: {
  latest: UnitAssessment;
  bestMark: number | null;
  count: number;
}) {
  return (
    <div className="mt-3 border-t border-art-mid/60 pt-3">
      <p className="text-[14px] leading-[1.6] text-gray-200">{latest.summary}</p>

      {latest.strengths.length > 0 || latest.improvements.length > 0 ? (
        <div className="mt-3 grid gap-3 sm:grid-cols-2">
          {latest.strengths.length > 0 ? (
            <Bullets title="What went well" tone="text-lime-300" items={latest.strengths} />
          ) : null}
          {latest.improvements.length > 0 ? (
            <Bullets
              title="To work on"
              tone="text-amber-300"
              items={latest.improvements}
            />
          ) : null}
        </div>
      ) : null}

      {/* The evidence behind the rating. Shown, not hidden behind a tooltip: a
          student who asks "why this mark?" deserves the counted facts, and a
          rating nobody can interrogate is one nobody should trust. */}
      <p className="mt-3 text-[12px] leading-[1.6] text-gray-500">
        Based on {latest.evidence.student_turns}{" "}
        {latest.evidence.student_turns === 1 ? "time" : "times"} you spoke,{" "}
        {latest.evidence.questions_asked}{" "}
        {latest.evidence.questions_asked === 1 ? "question" : "questions"} asked, and{" "}
        {formatDuration(latest.evidence.active_voice_ms)} of talking.
        {latest.evidence.comprehension_failed > 0
          ? ` You asked for ${latest.evidence.comprehension_failed} re-explanation${
              latest.evidence.comprehension_failed === 1 ? "" : "s"
            }.`
          : ""}
      </p>

      <p className="mt-2 text-[12px] text-gray-500">
        Assessed {formatDateTime(latest.assessed_at)}
        {/* Best-ever only when it beats the latest, so a student who had a bad
            session still sees that their best stands. */}
        {count > 1 && bestMark !== null && bestMark > latest.mark
          ? ` · your best on this unit is ${bestMark.toFixed(0)}/100`
          : ""}
      </p>
    </div>
  );
}

function Bullets({
  title,
  tone,
  items,
}: {
  title: string;
  tone: string;
  items: string[];
}) {
  return (
    <div>
      <p className={`text-[12px] font-semibold uppercase tracking-wide ${tone}`}>{title}</p>
      <ul className="mt-1 space-y-1">
        {items.map((item) => (
          <li key={item} className="text-[13px] leading-[1.5] text-gray-300">
            • {item}
          </li>
        ))}
      </ul>
    </div>
  );
}
