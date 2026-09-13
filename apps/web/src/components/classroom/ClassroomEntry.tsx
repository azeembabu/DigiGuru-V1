"use client";

/**
 * Resolves which block the session is for, then hands off to the live surface.
 *
 * Separate from `ClassroomSession` because the session cannot start without a
 * `block_id`, and "this student has not been placed in a block yet" is a real,
 * expected state (`current_block` is nullable in `/me/context` precisely because
 * an admin may not have placed them). Handling it here keeps that branch out of
 * the socket component, which would otherwise have to render a classroom it
 * cannot open.
 */

import { useEffect, useState } from "react";
import Link from "next/link";
import { useSearchParams } from "next/navigation";

import { loadStudentContext } from "@/lib/student-context";
import type { StudentContext } from "@/lib/student-context";
import { BoardErrorBoundary } from "@/components/classroom/BoardErrorBoundary";
import { ClassroomSession } from "@/components/classroom/ClassroomSession";
import { Logo } from "@/components/ui/Logo";

type Load =
  | { status: "loading" }
  | { status: "ready"; context: StudentContext }
  | { status: "error"; message: string };

export function ClassroomEntry() {
  const params = useSearchParams();
  // `?block=` is what `/classroom` puts here after the student picks a unit.
  // `current_block_id` remains the fallback so an older "resume" link, or a
  // direct visit to this URL, still opens something rather than dead-ending.
  const chosenBlock = params.get("block");
  const chosenUnit = params.get("unit");
  const [load, setLoad] = useState<Load>({ status: "loading" });

  useEffect(() => {
    let active = true;
    loadStudentContext()
      .then((context) => {
        if (active) setLoad({ status: "ready", context });
      })
      .catch((error: unknown) => {
        if (!active) return;
        // A 404 here means the signed-in account has no student row — an admin
        // reaching this URL. Said plainly rather than dressed up as a failure.
        setLoad({
          status: "error",
          message:
            error instanceof Error
              ? error.message
              : "Your academic context could not be loaded.",
        });
      });
    return () => {
      active = false;
    };
  }, []);

  if (load.status === "loading") {
    return <Shell>Opening the classroom…</Shell>;
  }

  if (load.status === "error") {
    return (
      <Shell>
        <p className="max-w-md text-[15px] leading-relaxed text-gray-300">{load.message}</p>
        <BackLink />
      </Shell>
    );
  }

  const blockId = chosenBlock ?? load.context.current_block?.id ?? null;
  if (blockId === null) {
    return (
      <Shell>
        <h1 className="font-display text-[24px] font-bold text-white">Nothing open yet</h1>
        <p className="max-w-md text-[15px] leading-relaxed text-gray-300">
          Your programme is set to {load.context.program.name}, semester{" "}
          {load.context.semester.semester_number}. The tutor teaches one unit at a time, so pick
          one to begin.
        </p>
        <Link
          href="/classroom"
          className="rounded-lg bg-indigo-500 px-4 py-2 text-[14px] font-semibold text-white hover:bg-indigo-400"
        >
          Choose what to study
        </Link>
        <BackLink />
      </Shell>
    );
  }

  return (
    <BoardErrorBoundary>
      <ClassroomSession blockId={blockId} documentId={chosenUnit} />
    </BoardErrorBoundary>
  );
}

function Shell({ children }: { children: React.ReactNode }) {
  return (
    <main className="flex min-h-dvh flex-col items-center justify-center gap-4 bg-ink-950 px-5 py-16 text-center text-lavender-50">
      <Logo size="md" />
      {children}
    </main>
  );
}

function BackLink() {
  return (
    <Link
      href="/dashboard"
      className="rounded-lg bg-indigo-500 px-4 py-2 text-[14px] font-semibold text-white hover:bg-indigo-400"
    >
      Back to dashboard
    </Link>
  );
}
