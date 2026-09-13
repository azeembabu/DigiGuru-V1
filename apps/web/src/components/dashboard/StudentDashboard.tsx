"use client";

import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import { ApiError } from "@/lib/api";
import { loadStudentContext, type StudentContext } from "@/lib/student-context";
import { buttonClass } from "@/components/ui/Button";
import { IconBoard, IconBook, IconClock, IconTarget } from "@/components/icons";
import { Card, CardTitle, StatTile } from "@/components/dashboard/Card";
import { ContextPanel } from "@/components/dashboard/ContextPanel";
import { ContinueCard } from "@/components/dashboard/ContinueCard";
import { DashboardHeader } from "@/components/dashboard/DashboardHeader";
import { ExamCards } from "@/components/dashboard/ExamCards";
import { DashboardSidebar } from "@/components/dashboard/DashboardSidebar";
import { DevicesPanel } from "@/components/dashboard/DevicesPanel";
import { SessionRules } from "@/components/dashboard/SessionRules";

// The dashboard is client-rendered against `GET /me/context` rather than
// fetched in a Server Component — see the note at the top of
// `lib/student-context.ts` for why the cookie transport makes that the honest
// choice here. The consequence is this component owns the three states the
// server would otherwise have resolved before painting: loading, unauthorised,
// and loaded.
type State =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; context: StudentContext };

export function StudentDashboard() {
  const router = useRouter();
  const [state, setState] = useState<State>({ status: "loading" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let cancelled = false;

    loadStudentContext()
      .then((context) => {
        if (!cancelled) setState({ status: "ready", context });
      })
      .catch((error: unknown) => {
        if (cancelled) return;

        // An expired or missing access cookie is not an error to report — it
        // is a redirect. The gateway is the only judge of that, so the 401 it
        // returns is what drives the bounce, not any client-side token check.
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

      {/* The rail is fixed, so the content column carries its own left offset
          rather than living in a flex row — that keeps the sticky header inside
          this column aligned with the content instead of spanning the rail. */}
      <div className="lg:pl-60">
        <DashboardHeader />

        <main className="mx-auto w-full max-w-6xl px-5 py-8 sm:px-8">
          {state.status === "loading" ? (
            <DashboardSkeleton />
          ) : state.status === "error" ? (
            <Card>
              <CardTitle>We couldn&rsquo;t load your dashboard</CardTitle>
              <p className="mt-2 text-[16px] leading-[1.6] text-gray-300">{state.message}</p>
              <button type="button" onClick={retry} className={buttonClass("outline", "mt-5")}>
                Try again
              </button>
            </Card>
          ) : (
            <div className="space-y-6">
              <Greeting context={state.context} />
              <StatRow context={state.context} />
              <ContinueCard context={state.context} />
              <ExamCards />
              <div className="grid gap-6 lg:grid-cols-2">
                <ContextPanel context={state.context} />
                <SessionRules />
              </div>
              <DevicesPanel />
            </div>
          )}
        </main>
      </div>
    </div>
  );
}

/**
 * `GET /me/context` carries no name (`crates/core/src/context.rs`), and it
 * should not — student PII stays out of payloads that are also logged and
 * cached (`.claude/rules/security.md`). So the greeting is keyed off
 * `is_first_login`, the one personal-feeling signal that is already there.
 */
function Greeting({ context }: { context: StudentContext }) {
  return (
    <div>
      <h1 className="font-display text-[28px] font-bold leading-[1.2] text-white sm:text-[40px] sm:leading-[1.1]">
        {context.is_first_login ? "Welcome to Digi Guru" : "Welcome back"}
      </h1>
      <p className="mt-2 max-w-2xl text-[18px] leading-[1.6] text-gray-300">
        {context.is_first_login
          ? "Your first session starts with a short introduction — after that, the tutor picks up wherever you stopped."
          : "Your classroom opens straight onto your current block. No picker, no setup."}
      </p>
    </div>
  );
}

/**
 * The reference layout leads with a metric row, so this one does too — but
 * every tile is a fact the gateway returned or the fixed NN-3 cap. There is no
 * progress percentage, streak, or score here because nothing in the API
 * measures one yet; a plausible-looking number a student cannot act on is
 * worse than an absent tile.
 */
function StatRow({ context }: { context: StudentContext }) {
  const block = context.current_block;

  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
      <StatTile
        label="Program"
        value={context.program.code}
        hint={context.program.name}
        icon={<IconBook className="h-5 w-5" />}
      />
      <StatTile
        label="Semester"
        value={String(context.semester.semester_number)}
        hint={context.semester.name.trim() || "Current semester"}
        icon={<IconTarget className="h-5 w-5" />}
      />
      <StatTile
        label="Current block"
        value={block === null ? "—" : String(block.block_no)}
        hint={block === null ? "Not assigned yet" : block.title}
        icon={<IconBoard className="h-5 w-5" />}
      />
      {/* NN-3 changed on 2026-09-13 from a per-session cap to a daily quota, so
          this tile is the day's allowance, not a session budget. It is the
          allowance rather than the remaining balance because the dashboard has
          no quota endpoint to read a balance from — `quota_remaining_ms`
          arrives on the classroom socket's `session_ready`, not here. */}
      <StatTile
        label="Daily voice limit"
        value="20:00"
        hint="Resets at midnight in your timezone"
        icon={<IconClock className="h-5 w-5" />}
      />
    </div>
  );
}

/** Matches the loaded layout's shape so the page does not jump on arrival. */
function DashboardSkeleton() {
  return (
    <div className="space-y-6" aria-busy="true" aria-label="Loading your dashboard">
      <div className="h-10 w-64 animate-pulse rounded-[8px] bg-art-mid/60" />
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {[0, 1, 2, 3].map((i) => (
          <div key={i} className="h-28 animate-pulse rounded-[16px] bg-art-mid/50" />
        ))}
      </div>
      <div className="h-52 animate-pulse rounded-[16px] bg-art-mid/50" />
      <div className="grid gap-6 lg:grid-cols-2">
        <div className="h-56 animate-pulse rounded-[16px] bg-art-mid/50" />
        <div className="h-56 animate-pulse rounded-[16px] bg-art-mid/50" />
      </div>
    </div>
  );
}
