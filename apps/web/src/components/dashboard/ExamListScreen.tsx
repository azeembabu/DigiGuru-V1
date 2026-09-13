"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import { ApiError } from "@/lib/api";
import {
  attemptDate,
  formatScore,
  loadMyExamAttempts,
  STATUS_LABEL,
  type MyExamAttempt,
} from "@/lib/student-exams";
import { buttonClass } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/dashboard/Card";
import { DashboardHeader } from "@/components/dashboard/DashboardHeader";
import { DashboardSidebar } from "@/components/dashboard/DashboardSidebar";

/** One page. Well inside the gateway's 1–200 ceiling. */
const PAGE_SIZE = 50;

type State =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; attempts: MyExamAttempt[]; total: number };

/**
 * Every exam attempt on this account, newest first.
 *
 * Deliberately not paginated with controls yet: `loadMyExamAttempts` takes a
 * limit but this screen requests one page of 50, and shows a plain count when
 * there are more. Paging UI can follow the first student who has more than 50
 * attempts — building it now would be building against no one.
 */
export function ExamListScreen() {
  const router = useRouter();
  const [state, setState] = useState<State>({ status: "loading" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let cancelled = false;

    loadMyExamAttempts(PAGE_SIZE)
      .then(({ items, total }) => {
        if (!cancelled) setState({ status: "ready", attempts: items, total });
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        if (error instanceof ApiError && error.status === 401) {
          router.replace("/login");
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
  }, [router, attempt]);

  const retry = useCallback(() => {
    setState({ status: "loading" });
    setAttempt((n) => n + 1);
  }, []);

  return (
    <div className="min-h-dvh bg-art-base text-lavender-50">
      <DashboardSidebar />
      <div className="lg:pl-60">
        <DashboardHeader />
        <main className="mx-auto w-full max-w-4xl px-5 py-8 sm:px-8">
          <Link
            href="/dashboard"
            className="text-[14px] font-semibold text-gray-300 underline decoration-gray-500 underline-offset-4 hover:decoration-art-glow"
          >
            &larr; Back to dashboard
          </Link>

          <h1 className="mt-5 font-display text-[28px] font-bold leading-[1.2] text-white sm:text-[34px]">
            Your exam results
          </h1>

          <div className="mt-6">
            {state.status === "loading" ? (
              <div className="space-y-3" aria-busy="true" aria-label="Loading your exam results">
                {[0, 1, 2].map((i) => (
                  <div key={i} className="h-16 animate-pulse rounded-[12px] bg-art-mid/50" />
                ))}
              </div>
            ) : state.status === "error" ? (
              <Card>
                <CardTitle>We couldn&rsquo;t load your results</CardTitle>
                <p className="mt-2 text-[16px] leading-[1.6] text-gray-300">{state.message}</p>
                <button type="button" onClick={retry} className={buttonClass("outline", "mt-5")}>
                  Try again
                </button>
              </Card>
            ) : state.attempts.length === 0 ? (
              <Card>
                <CardTitle>No results yet</CardTitle>
                <p className="mt-2 text-[16px] leading-[1.6] text-gray-300">
                  Once you sit an exam and it is marked, it will appear here.
                </p>
              </Card>
            ) : (
              <>
                <ul className="space-y-3">
                  {state.attempts.map((row) => (
                    <li key={row.id}>
                      <ListRow attempt={row} />
                    </li>
                  ))}
                </ul>
                {state.total > state.attempts.length ? (
                  <p className="mt-4 text-[14px] leading-[1.6] text-gray-300/70">
                    Showing {state.attempts.length} of {state.total}.
                  </p>
                ) : null}
              </>
            )}
          </div>
        </main>
      </div>
    </div>
  );
}

function ListRow({ attempt }: { attempt: MyExamAttempt }) {
  const score = formatScore(attempt);

  return (
    <Link
      href={`/dashboard/exams/${attempt.id}`}
      className="flex flex-wrap items-center justify-between gap-x-4 gap-y-1 rounded-[12px] border border-art-mid/50 bg-art-mid/20 p-4 transition-colors hover:border-art-glow/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-art-glow"
    >
      <div className="min-w-0">
        <p className="truncate text-[15px] font-semibold leading-[1.4] text-white">
          {attempt.exam_title}
        </p>
        <p className="mt-0.5 truncate text-[13px] leading-[1.5] text-gray-300/70">
          {attempt.course_code} &middot; Block {attempt.block_no} &middot; {attemptDate(attempt)}
          {attempt.attempt_no > 1 ? ` · Attempt ${attempt.attempt_no}` : ""}
        </p>
      </div>
      {score === null ? (
        <span className="shrink-0 rounded-full border border-art-mid/70 px-2.5 py-1 text-[12px] font-semibold text-gray-300/80">
          {STATUS_LABEL[attempt.status]}
        </span>
      ) : (
        <p className="shrink-0 font-display text-[18px] font-bold leading-[1.2] tabular-nums text-white">
          {score}
        </p>
      )}
    </Link>
  );
}
