"use client";

import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import { ApiError } from "@/lib/api";
import { SchemaError, loadStudentDashboard, type StudentDashboard } from "@/lib/student";
import { PanelSkeleton, Pill, StudentError } from "@/components/student/parts";
import { QuotaRing } from "@/components/student/QuotaRing";
import {
  ContinueLearningHero,
  ExamHistoryPanel,
  FlashcardPanel,
  NotesLibraryPanel,
  RevisionPanel,
} from "@/components/student/panels";

// All six dashboard components, hydrated from the ONE `GET /student/dashboard`
// call the contract specifies. Nothing here fans out to fetch a label: the
// payload is already denormalised for exactly that reason.
//
// Two panels (flashcards, revisions) make a second request for their *rows*
// after the counts land — the dashboard payload carries counts only, and those
// list endpoints are the contract's own. Each degrades on its own without
// taking the page with it.

type Outcome =
  | { status: "error"; message: string; retryable: boolean }
  | { status: "ready"; data: StudentDashboard };

export function StudentOverview() {
  const router = useRouter();
  const [attempt, setAttempt] = useState(0);
  // Tagged with the attempt it answers, so "are we still loading" is a
  // comparison during render rather than a synchronous setState inside the
  // effect — the same shape `useResource` uses in the admin console.
  const [result, setResult] = useState<{ attempt: number; outcome: Outcome } | null>(null);

  useEffect(() => {
    let cancelled = false;
    const setState = (outcome: Outcome) => setResult({ attempt, outcome });

    loadStudentDashboard()
      .then((data) => {
        if (!cancelled) setState({ status: "ready", data });
      })
      .catch((caught: unknown) => {
        if (cancelled) return;

        // Only the gateway can judge the cookie, so its 401 is what drives the
        // bounce — there is no client-side token check to make.
        if (caught instanceof ApiError && caught.status === 401) {
          router.replace("/login");
          return;
        }

        // A 404 here is the gateway saying "you are not a student" — the
        // student routes are self-only and resolve the subject from the token,
        // so an admin signed into this page legitimately has no dashboard.
        // That is the documented behaviour, not a failure to retry.
        if (caught instanceof ApiError && caught.status === 404) {
          setState({
            status: "error",
            message:
              "This account has no student record, so there is no study dashboard to show. Admin accounts use the console instead.",
            retryable: false,
          });
          return;
        }

        setState({
          status: "error",
          message:
            caught instanceof SchemaError || caught instanceof ApiError
              ? caught.message
              : "Something went wrong loading your dashboard.",
          retryable: !(caught instanceof SchemaError),
        });
      });

    return () => {
      cancelled = true;
    };
  }, [router, attempt]);

  const retry = useCallback(() => setAttempt((n) => n + 1), []);

  const state: Outcome | { status: "loading" } =
    result !== null && result.attempt === attempt ? result.outcome : { status: "loading" };

  if (state.status === "loading") {
    return (
      <div className="space-y-6" aria-busy="true" aria-label="Loading your study dashboard">
        <PanelSkeleton className="h-56" />
        <div className="grid gap-6 lg:grid-cols-2">
          <PanelSkeleton className="h-64" />
          <PanelSkeleton className="h-64" />
        </div>
        <div className="grid gap-6 lg:grid-cols-3">
          <PanelSkeleton className="h-48" />
          <PanelSkeleton className="h-48" />
          <PanelSkeleton className="h-48" />
        </div>
      </div>
    );
  }

  if (state.status === "error") {
    return (
      <StudentError
        title="We couldn’t load your study dashboard"
        message={state.message}
        onRetry={state.retryable ? retry : undefined}
      />
    );
  }

  const { student_info, continue_learning, quota_status, exam_history, saved_resources } =
    state.data;

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center gap-3">
        <p className="font-display text-[18px] font-bold leading-[1.3] text-white">
          {student_info.name}
        </p>
        <Pill>{student_info.roll_number}</Pill>
        <Pill tone="accent">
          {student_info.program} · Semester {student_info.semester}
        </Pill>
      </div>

      <ContinueLearningHero resume={continue_learning} />

      <div className="grid gap-6 lg:grid-cols-2">
        <QuotaRing quota={quota_status} />
        <ExamHistoryPanel history={exam_history} />
      </div>

      <div className="grid gap-6 lg:grid-cols-3">
        <NotesLibraryPanel resources={saved_resources} />
        <FlashcardPanel resources={saved_resources} />
        <RevisionPanel resources={saved_resources} />
      </div>
    </div>
  );
}
