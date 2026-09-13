"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import { ApiError } from "@/lib/api";
import { SchemaError, loadStudentExams, type StudentExam } from "@/lib/student";
import {
  AssessmentTypePill,
  PanelSkeleton,
  Pill,
  StudentEmpty,
  StudentError,
  cardClass,
  formatPercent,
} from "@/components/student/parts";
import { IconArrowUpRight, IconClock } from "@/components/icons";

// The Exam Module's index: every published exam the student's enrolments reach.
//
// There is no client-side filtering to "only published" here — the gateway's
// `GET /student/exams` already restricts to `status='published'` for the
// caller's enrolled courses, and re-filtering in the browser would imply the
// client is a participant in that decision. It is not.

type Outcome =
  | { status: "error"; message: string; retryable: boolean }
  | { status: "ready"; exams: StudentExam[]; total: number };

export function ExamsScreen() {
  const router = useRouter();
  const [attempt, setAttempt] = useState(0);
  // Tagged with the attempt it answers — see the note in `StudentOverview`.
  const [result, setResult] = useState<{ attempt: number; outcome: Outcome } | null>(null);

  useEffect(() => {
    let cancelled = false;
    const setState = (outcome: Outcome) => setResult({ attempt, outcome });

    loadStudentExams({ limit: 200 })
      .then((page) => {
        if (!cancelled) setState({ status: "ready", exams: page.items, total: page.total });
      })
      .catch((caught: unknown) => {
        if (cancelled) return;

        if (caught instanceof ApiError && caught.status === 401) {
          router.replace("/login");
          return;
        }

        if (caught instanceof ApiError && caught.status === 404) {
          setState({
            status: "error",
            message:
              "This account has no student record, so it has no assessments. Administrator accounts use the console instead.",
            retryable: false,
          });
          return;
        }

        setState({
          status: "error",
          message:
            caught instanceof SchemaError || caught instanceof ApiError
              ? caught.message
              : "Your assessments could not be loaded. Please try again.",
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

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-[28px] font-bold leading-[1.2] text-white sm:text-[36px]">
          Exam module
        </h1>
        <p className="mt-2 max-w-2xl text-[17px] leading-[1.6] text-gray-300">
          Assignments, mid-term quizzes and semester exams for your semester. Each paper is drawn at
          random from a larger question pool, so attempts vary. Your answers and the reasoning
          behind them are shown on submission.
        </p>
      </div>

      {state.status === "loading" ? (
        <div className="grid gap-5 md:grid-cols-2" aria-busy="true" aria-label="Loading your assessments">
          <PanelSkeleton className="h-44" />
          <PanelSkeleton className="h-44" />
        </div>
      ) : state.status === "error" ? (
        <StudentError
          title="Your assessments could not be loaded"
          message={state.message}
          onRetry={state.retryable ? retry : undefined}
        />
      ) : state.exams.length === 0 ? (
        <div className={cardClass}>
          <StudentEmpty
            title="No assessments are open yet"
            hint="An assessment appears here once your course coordinator publishes one for a course you are enrolled in. Nothing is hidden: none has been published for your semester yet."
          />
        </div>
      ) : (
        <div className="grid gap-5 md:grid-cols-2">
          {state.exams.map((exam) => (
            <ExamCard key={exam.id} exam={exam} />
          ))}
        </div>
      )}
    </div>
  );
}

function ExamCard({ exam }: { exam: StudentExam }) {
  const taken = exam.attempts_used > 0;

  return (
    <article className={`${cardClass} flex flex-col`}>
      <div className="flex flex-wrap items-center gap-2">
        <AssessmentTypePill type={exam.assessment_type} />
        {taken ? (
          <Pill tone="good">
            {exam.attempts_used} attempt{exam.attempts_used === 1 ? "" : "s"} ·{" "}
            {formatPercent(exam.best_percentage)} best
          </Pill>
        ) : (
          <Pill>Not attempted</Pill>
        )}
      </div>

      <h2 className="mt-3 font-display text-[20px] font-bold leading-[1.3] text-white">
        {exam.title}
      </h2>
      <p className="mt-1 text-[14px] leading-[1.5] text-gray-300/70">
        {exam.course_code} · {exam.course_name} · Block {exam.block_no}
      </p>
      {exam.description ? (
        <p className="mt-3 text-[15px] leading-[1.6] text-gray-300">{exam.description}</p>
      ) : null}

      <dl className="mt-4 flex flex-wrap gap-x-6 gap-y-2 text-[14px] text-gray-300/70">
        <div className="flex items-center gap-1.5">
          <dt className="sr-only">Number of questions</dt>
          <dd className="tabular-nums">
            {exam.question_count === null ? "—" : exam.question_count} questions
          </dd>
        </div>
        <div className="flex items-center gap-1.5">
          <IconClock className="h-4 w-4" aria-hidden="true" />
          <dt className="sr-only">Time limit</dt>
          <dd className="tabular-nums">
            {exam.duration_minutes === null ? "No time limit" : `${exam.duration_minutes} minutes`}
          </dd>
        </div>
      </dl>

      <div className="mt-6 flex-1" />

      <Link
        href={`/exams/${exam.id}`}
        className="inline-flex items-center gap-1.5 self-start rounded-full bg-lime-400 px-5 py-2.5 text-[15px] font-semibold text-ink-950 transition-colors hover:bg-lime-300 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base"
      >
        {taken ? "Attempt again" : "Begin"}
        <IconArrowUpRight className="h-4 w-4" />
      </Link>
    </article>
  );
}
