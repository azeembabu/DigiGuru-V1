"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import { ApiError } from "@/lib/api";
import { isAdmin, loadMe, type Me } from "@/lib/me";
import { loadStudentContext, type StudentContext } from "@/lib/student-context";
import { IconArrowUpRight, IconBoard, IconTarget } from "@/components/icons";
import { Logo } from "@/components/ui/Logo";
import { PanelSkeleton, Pill, StudentError, cardClass } from "@/components/student/parts";

// The post-login router: the two things a signed-in student can actually do.
//
// Identity is *read*, never guessed: `GET /me` says which role this is and
// `GET /me/context` says which program and block. An admin who lands here is
// sent to the console rather than shown a student chooser — the gateway would
// 404 every student route for them anyway, and the honest move is to route
// them, not to render a page that cannot work.
//
// `/me/context` is allowed to fail without blocking the page: it only supplies
// the subtitle line. Losing a label is not worth blocking both doors.

type Outcome =
  | { status: "error"; message: string }
  | { status: "ready"; me: Me; context: StudentContext | null };

export function PortalChooser() {
  const router = useRouter();
  const [attempt, setAttempt] = useState(0);
  // Tagged with the attempt it answers — see the note in `StudentOverview`.
  const [result, setResult] = useState<{ attempt: number; outcome: Outcome } | null>(null);

  useEffect(() => {
    let cancelled = false;
    const setState = (outcome: Outcome) => setResult({ attempt, outcome });

    loadMe()
      .then(async (me) => {
        if (cancelled) return;

        if (isAdmin(me.role)) {
          router.replace("/admin");
          return;
        }

        const context = await loadStudentContext().catch(() => null);
        if (!cancelled) setState({ status: "ready", me, context });
      })
      .catch((caught: unknown) => {
        if (cancelled) return;

        if (caught instanceof ApiError && caught.status === 401) {
          router.replace("/login");
          return;
        }

        setState({
          status: "error",
          message:
            caught instanceof ApiError
              ? caught.message
              : "We could not confirm who you are signed in as.",
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
    <main className="mx-auto w-full max-w-4xl px-5 py-12 sm:px-8 sm:py-16">
      <Logo size="sm" />

      {state.status === "loading" ? (
        <div className="mt-10 space-y-6" aria-busy="true" aria-label="Loading your portal">
          <PanelSkeleton className="h-16" />
          <div className="grid gap-6 sm:grid-cols-2">
            <PanelSkeleton className="h-60" />
            <PanelSkeleton className="h-60" />
          </div>
        </div>
      ) : state.status === "error" ? (
        <div className="mt-10">
          <StudentError message={state.message} onRetry={retry} />
        </div>
      ) : (
        <>
          <h1 className="mt-10 text-balance font-display text-[32px] font-bold leading-[1.15] text-white sm:text-[40px]">
            Study and assessment
          </h1>
          <p className="mt-3 max-w-xl text-[17px] leading-[1.6] text-gray-300">
            Two modules, one record: tutoring on your own syllabus, and the assessments set for your
            semester. Your progress carries between them.
          </p>

          {state.context !== null ? (
            <div className="mt-5 flex flex-wrap items-center gap-2">
              <Pill tone="accent">
                {state.context.program.name} · Semester {state.context.semester.semester_number}
              </Pill>
              {state.context.current_block === null ? (
                <Pill tone="warn">No block assigned yet</Pill>
              ) : (
                <Pill>Block {state.context.current_block.block_no}</Pill>
              )}
            </div>
          ) : null}

          <div className="mt-8 grid gap-6 sm:grid-cols-2">
            <Door
              href="/dashboard"
              eyebrow="Study module"
              title="Live tutoring session"
              body="A spoken session held strictly within your own syllabus, with the whiteboard drawn before the tutor speaks. Up to 20 minutes of speaking time each day."
              icon={<IconBoard className="h-6 w-6" />}
            />
            <Door
              href="/exams"
              eyebrow="Exam module"
              title="Assessments for your semester"
              body="Assignments, mid-term quizzes and semester exams drawn from your semester. Your answers and the reasoning behind them are shown on submission."
              icon={<IconTarget className="h-6 w-6" />}
            />
          </div>
        </>
      )}
    </main>
  );
}

/**
 * One destination.
 *
 * The whole card is the link, not a button inside it — at this size a student
 * aims at the card, and a nested interactive element would give a keyboard user
 * two stops for one choice.
 */
function Door({
  href,
  eyebrow,
  title,
  body,
  icon,
}: {
  href: string;
  eyebrow: string;
  title: string;
  body: string;
  icon: React.ReactNode;
}) {
  return (
    <Link
      href={href}
      className={`${cardClass} group block transition-colors hover:border-art-edge focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base sm:!p-7`}
    >
      <span className="flex h-12 w-12 items-center justify-center rounded-full bg-art-mid/60 text-art-glow">
        {icon}
      </span>
      <p className="mt-5 text-[12px] font-semibold uppercase tracking-[0.04em] text-gray-300/70">
        {eyebrow}
      </p>
      <p className="mt-2 font-display text-[22px] font-bold leading-[1.25] text-white">{title}</p>
      <p className="mt-2 text-[15px] leading-[1.6] text-gray-300">{body}</p>
      <span className="mt-5 inline-flex items-center gap-1.5 text-[15px] font-semibold text-lime-400">
        Open
        <IconArrowUpRight className="h-4 w-4 transition-transform group-hover:translate-x-0.5" />
      </span>
    </Link>
  );
}
