"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import { ApiError } from "@/lib/api";
import {
  attemptDate,
  formatScore,
  loadMyExamAttempt,
  scorePercent,
  STATUS_LABEL,
  type MyExamAttempt,
} from "@/lib/student-exams";
import { buttonClass } from "@/components/ui/Button";
import { Card, CardLabel, CardTitle } from "@/components/dashboard/Card";
import { DashboardHeader } from "@/components/dashboard/DashboardHeader";
import { DashboardSidebar } from "@/components/dashboard/DashboardSidebar";

type State =
  | { status: "loading" }
  | { status: "missing" }
  | { status: "error"; message: string }
  | { status: "ready"; attempt: MyExamAttempt };

/**
 * One exam attempt, reached from a dashboard score card.
 *
 * `404` is rendered as its own state rather than as an error, because the
 * gateway returns it both for an attempt that does not exist and for one
 * belonging to another student — deliberately, since a `403` would confirm the
 * id was real. This screen must not distinguish them either, so the copy says
 * neither "deleted" nor "not yours".
 */
export function ExamReviewScreen({ attemptId }: { attemptId: string }) {
  const router = useRouter();
  const [state, setState] = useState<State>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;

    loadMyExamAttempt(attemptId)
      .then((attempt) => {
        if (!cancelled) setState({ status: "ready", attempt });
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        if (error instanceof ApiError && error.status === 401) {
          router.replace("/login");
          return;
        }
        if (error instanceof ApiError && error.status === 404) {
          setState({ status: "missing" });
          return;
        }
        setState({
          status: "error",
          message:
            error instanceof ApiError
              ? error.message
              : "Something went wrong. Please try again.",
        });
      });

    return () => {
      cancelled = true;
    };
  }, [attemptId, router]);

  return (
    <div className="min-h-dvh bg-art-base text-lavender-50">
      <DashboardSidebar />
      <div className="lg:pl-60">
        <DashboardHeader />
        <main className="mx-auto w-full max-w-3xl px-5 py-8 sm:px-8">
          <Link
            href="/dashboard"
            className="text-[14px] font-semibold text-gray-300 underline decoration-gray-500 underline-offset-4 hover:decoration-art-glow"
          >
            &larr; Back to dashboard
          </Link>

          <div className="mt-5">
            {state.status === "loading" ? (
              <div
                className="h-64 animate-pulse rounded-[16px] bg-art-mid/50"
                aria-busy="true"
                aria-label="Loading your exam result"
              />
            ) : state.status === "missing" ? (
              <Card>
                <CardTitle>We couldn&rsquo;t find that result</CardTitle>
                <p className="mt-2 text-[16px] leading-[1.6] text-gray-300">
                  This exam result isn&rsquo;t available on your account.
                </p>
                <Link href="/dashboard/exams" className={buttonClass("outline", "mt-5")}>
                  See all your results
                </Link>
              </Card>
            ) : state.status === "error" ? (
              <Card>
                <CardTitle>We couldn&rsquo;t load that result</CardTitle>
                <p className="mt-2 text-[16px] leading-[1.6] text-gray-300">{state.message}</p>
              </Card>
            ) : (
              <AttemptDetail attempt={state.attempt} />
            )}
          </div>
        </main>
      </div>
    </div>
  );
}

function AttemptDetail({ attempt }: { attempt: MyExamAttempt }) {
  const score = formatScore(attempt);
  const percent = scorePercent(attempt);

  return (
    <Card>
      <CardLabel>
        {attempt.course_code} &middot; {attempt.course_name}
      </CardLabel>
      <h1 className="mt-2 font-display text-[28px] font-bold leading-[1.2] text-white">
        {attempt.exam_title}
      </h1>
      <p className="mt-1 text-[15px] leading-[1.6] text-gray-300/80">
        Block {attempt.block_no} &middot; {attempt.block_title}
      </p>

      <div className="mt-6 flex flex-wrap items-end gap-x-8 gap-y-4">
        <div>
          <p className="text-[13px] leading-[1.5] text-gray-300/70">Score</p>
          {score === null ? (
            <p className="mt-1 font-display text-[22px] font-bold leading-[1.2] text-white">
              {STATUS_LABEL[attempt.status]}
            </p>
          ) : (
            <p className="mt-1 font-display text-[34px] font-bold leading-[1.1] tabular-nums text-white">
              {score}
            </p>
          )}
        </div>
        <div>
          <p className="text-[13px] leading-[1.5] text-gray-300/70">
            {attempt.submitted_at === null ? "Started" : "Submitted"}
          </p>
          <p className="mt-1 text-[17px] leading-[1.4] text-white">{attemptDate(attempt)}</p>
        </div>
        <div>
          <p className="text-[13px] leading-[1.5] text-gray-300/70">Attempt</p>
          <p className="mt-1 text-[17px] leading-[1.4] tabular-nums text-white">
            {attempt.attempt_no}
          </p>
        </div>
      </div>

      {percent !== null ? (
        <div
          className="mt-6 h-2 overflow-hidden rounded-full bg-art-mid/60"
          role="img"
          aria-label={`Score ${score}`}
        >
          <div className="h-full rounded-full bg-art-glow" style={{ width: `${percent}%` }} />
        </div>
      ) : null}

      {/* The per-question breakdown the spec calls "weak areas" is not rendered
          here: `MyExamAttemptResponse` carries totals only, and nothing in the
          API scores a topic. Inventing weak areas from a single total would be
          a guess presented to a student as a diagnosis. */}
      <p className="mt-6 text-[14px] leading-[1.6] text-gray-300/70">
        Your tutor can go back over this block in the classroom whenever you want.
      </p>
      <Link href="/classroom" className={buttonClass("primary", "mt-4")}>
        Open the classroom
      </Link>
    </Card>
  );
}
