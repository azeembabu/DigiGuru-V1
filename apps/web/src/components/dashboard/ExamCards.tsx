"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { ApiError } from "@/lib/api";
import {
  attemptDate,
  formatScore,
  loadMyExamAttempts,
  scorePercent,
  STATUS_LABEL,
  type MyExamAttempt,
} from "@/lib/student-exams";
import { Card, CardTitle } from "@/components/dashboard/Card";

/** How many cards the dashboard shows before deferring to the full list. */
const PREVIEW_COUNT = 4;

type State =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; attempts: MyExamAttempt[]; total: number };

/**
 * Recent exam score cards.
 *
 * Loaded separately from `/me/context` rather than folded into it: a student
 * with no attempts is the common case early in a term, and a failure here must
 * not take the rest of the dashboard down with it. So this owns its own
 * loading and error states and degrades to a single line of text.
 *
 * A 401 is deliberately *not* handled here. `apiFetch` already redirects to
 * /login when the refresh rotation fails, and `StudentDashboard` owns the
 * bounce for the context request — duplicating it would race two navigations.
 */
export function ExamCards() {
  const [state, setState] = useState<State>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;

    loadMyExamAttempts(PREVIEW_COUNT)
      .then(({ items, total }) => {
        if (!cancelled) setState({ status: "ready", attempts: items, total });
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        setState({
          status: "error",
          message:
            error instanceof ApiError
              ? error.message
              : "Your exam results could not be loaded.",
        });
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <Card>
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <CardTitle>Recent exams</CardTitle>
        {state.status === "ready" && state.total > state.attempts.length ? (
          <Link
            href="/dashboard/exams"
            className="text-[14px] font-semibold text-art-glow underline decoration-art-glow/40 underline-offset-4 hover:decoration-art-glow"
          >
            View all {state.total}
          </Link>
        ) : null}
      </div>

      {state.status === "loading" ? (
        <div className="mt-5 space-y-3" aria-busy="true" aria-label="Loading your exam results">
          {[0, 1].map((i) => (
            <div key={i} className="h-16 animate-pulse rounded-[12px] bg-art-mid/50" />
          ))}
        </div>
      ) : state.status === "error" ? (
        <p className="mt-3 text-[15px] leading-[1.6] text-gray-300">{state.message}</p>
      ) : state.attempts.length === 0 ? (
        <p className="mt-3 text-[15px] leading-[1.6] text-gray-300">
          You haven&rsquo;t sat an exam yet. Results appear here once your first attempt is marked.
        </p>
      ) : (
        <ul className="mt-5 space-y-3">
          {state.attempts.map((attempt) => (
            <li key={attempt.id}>
              <AttemptRow attempt={attempt} />
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}

function AttemptRow({ attempt }: { attempt: MyExamAttempt }) {
  const score = formatScore(attempt);
  const percent = scorePercent(attempt);

  return (
    <Link
      href={`/dashboard/exams/${attempt.id}`}
      className="block rounded-[12px] border border-art-mid/50 bg-art-mid/20 p-4 transition-colors hover:border-art-glow/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-art-glow"
    >
      <div className="flex flex-wrap items-start justify-between gap-x-4 gap-y-1">
        <div className="min-w-0">
          <p className="truncate text-[15px] font-semibold leading-[1.4] text-white">
            {attempt.exam_title}
          </p>
          <p className="mt-0.5 truncate text-[13px] leading-[1.5] text-gray-300/70">
            {attempt.course_code} &middot; Block {attempt.block_no}
            {/* Attempt number only when it is not the first sitting — "attempt 1"
                on every row is noise, but two rows for one exam are otherwise
                indistinguishable. */}
            {attempt.attempt_no > 1 ? ` · Attempt ${attempt.attempt_no}` : ""}
          </p>
        </div>

        <div className="shrink-0 text-right">
          {score === null ? (
            <span className="inline-block rounded-full border border-art-mid/70 px-2.5 py-1 text-[12px] font-semibold text-gray-300/80">
              {STATUS_LABEL[attempt.status]}
            </span>
          ) : (
            <p className="font-display text-[20px] font-bold leading-[1.2] tabular-nums text-white">
              {score}
            </p>
          )}
          <p className="mt-0.5 text-[12px] leading-[1.5] text-gray-300/60">
            {attemptDate(attempt)}
          </p>
        </div>
      </div>

      {/* The meter is drawn only for a graded attempt. There is no pass mark in
          the schema, so it is deliberately one neutral colour — tinting it
          red/green would assert a threshold the institution never set. */}
      {percent !== null ? (
        <div
          className="mt-3 h-1.5 overflow-hidden rounded-full bg-art-mid/60"
          role="img"
          aria-label={`Score ${score}`}
        >
          <div className="h-full rounded-full bg-art-glow" style={{ width: `${percent}%` }} />
        </div>
      ) : null}
    </Link>
  );
}
