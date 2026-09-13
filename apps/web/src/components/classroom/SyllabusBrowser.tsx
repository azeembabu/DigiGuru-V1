"use client";

/**
 * The way into the classroom: course -> block -> unit -> board.
 *
 * `/classroom` used to open the board immediately on
 * `students.current_block_id`, which meant a student could study exactly one
 * block and had no way to see the rest of their own syllabus, let alone choose
 * from it. This is that missing step.
 *
 * # State lives in the URL
 *
 * `?course=` and `?block=` rather than component state, so the browser's back
 * button walks back up the syllabus one level at a time — which is what a
 * student will press — and so a particular block is a link somebody can keep.
 * Held with `replace` rather than `push` only where the change is a correction
 * rather than a navigation.
 *
 * # Readiness is shown, not hidden
 *
 * A unit is teachable only once its PDF is embedded; until then retrieval finds
 * nothing and the tutor abstains on everything (NN-4 working correctly, but
 * indistinguishable from a broken classroom if you are the student). So an
 * un-ingested unit is listed and visibly marked rather than omitted — "not
 * ready yet" is a truthful answer, a missing unit is not.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";

import {
  loadSyllabusBlocks,
  loadSyllabusCourses,
  loadSyllabusUnits,
  type SyllabusBlock,
  type SyllabusCourse,
  type SyllabusUnit,
} from "@/lib/student";
import { Logo } from "@/components/ui/Logo";
import { IngestProgress, isInFlight } from "@/components/shared/IngestProgress";

type Load<T> =
  | { status: "loading" }
  | { status: "ready"; items: T[] }
  | { status: "error"; message: string };

function message(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}

/**
 * Loads a list once per key, discarding results that arrive after a change.
 *
 * The stored value is **tagged with the key it answers**, and a tag that does
 * not match the current key reads as "loading". That is what makes switching
 * course or block show a skeleton rather than the previous list, without the
 * effect having to set state on the way in — a stale list rendered under a new
 * heading is worse than a spinner, because it looks like real data.
 */
function useList<T>(
  key: string | null,
  load: (key: string) => Promise<T[]>,
  /** Bumping this refetches the same key without flashing a skeleton. */
  refreshToken = 0,
): Load<T> {
  const [tagged, setTagged] = useState<{ key: string; state: Load<T> } | null>(null);

  useEffect(() => {
    if (key === null) return;
    let active = true;
    load(key)
      .then((items) => {
        if (active) setTagged({ key, state: { status: "ready", items } });
      })
      .catch((error: unknown) => {
        if (!active) return;
        setTagged({
          key,
          state: { status: "error", message: message(error, "This could not be loaded.") },
        });
      });
    return () => {
      active = false;
    };
    // `load` is a module-level function; the id is what decides a refetch.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, refreshToken]);

  if (key === null || tagged === null || tagged.key !== key) return { status: "loading" };
  return tagged.state;
}

export function SyllabusBrowser() {
  const router = useRouter();
  const params = useSearchParams();
  const courseId = params.get("course");
  const blockId = params.get("block");

  const courses = useList<SyllabusCourse>("all", loadSyllabusCourses);
  const blocks = useList<SyllabusBlock>(courseId, loadSyllabusBlocks);
  // Declared before the list that consumes it: `tick` is what makes the list
  // refetch, so it cannot be derived from the list.
  const [tick, setTick] = useState(0);
  const units = useList<SyllabusUnit>(blockId, loadSyllabusUnits, tick);

  // Ingestion finishes on its own, minutes after an upload, so a unit list
  // opened mid-processing would otherwise sit at "Queued" until the student
  // thought to reload. Polling stops the moment nothing is in flight, so a
  // settled block costs nothing.
  const unitsInFlight =
    units.status === "ready" && units.items.some((unit) => isInFlight(unit.status));
  useEffect(() => {
    if (!unitsInFlight) return;
    const id = setInterval(() => setTick((n) => n + 1), 5000);
    return () => clearInterval(id);
  }, [unitsInFlight]);

  const go = useCallback(
    (next: { course?: string | null; block?: string | null }) => {
      const search = new URLSearchParams();
      const course = next.course === undefined ? courseId : next.course;
      const block = next.block === undefined ? blockId : next.block;
      if (course) search.set("course", course);
      if (block) search.set("block", block);
      const qs = search.toString();
      router.push(qs === "" ? "/classroom" : `/classroom?${qs}`);
    },
    [router, courseId, blockId],
  );

  const course = useMemo(
    () =>
      courses.status === "ready"
        ? (courses.items.find((c) => c.course_id === courseId) ?? null)
        : null,
    [courses, courseId],
  );
  const block = useMemo(
    () =>
      blocks.status === "ready"
        ? (blocks.items.find((b) => b.block_id === blockId) ?? null)
        : null,
    [blocks, blockId],
  );

  return (
    <main className="min-h-dvh bg-ink-950 text-lavender-50">
      <header className="flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-white/10 px-4 py-3 sm:px-6">
        <Logo size="sm" />
        <nav aria-label="Breadcrumb" className="flex flex-wrap items-center gap-2 text-[13px]">
          <Crumb onClick={() => go({ course: null, block: null })} active={courseId === null}>
            Your courses
          </Crumb>
          {courseId ? (
            <>
              <span className="text-gray-600">/</span>
              <Crumb onClick={() => go({ block: null })} active={blockId === null}>
                {course?.code ?? "Course"}
              </Crumb>
            </>
          ) : null}
          {blockId ? (
            <>
              <span className="text-gray-600">/</span>
              <Crumb active>Block {block?.block_no ?? ""}</Crumb>
            </>
          ) : null}
        </nav>
        <Link
          href="/dashboard"
          className="ml-auto text-[13px] text-gray-400 hover:text-gray-200"
        >
          Dashboard
        </Link>
      </header>

      <div className="mx-auto max-w-3xl px-4 py-8 sm:px-6">
        {blockId !== null ? (
          <Step
            eyebrow={block ? `Block ${block.block_no}` : "Block"}
            title={block?.title ?? "Units"}
            hint="Pick a unit to study. The tutor teaches from the material in it, and writes on the board before it speaks."
          >
            <ListState state={units} empty="No units have been uploaded to this block yet.">
              {(items) =>
                items.map((unit) => (
                  <UnitRow key={unit.document_id} unit={unit} blockId={blockId} />
                ))
              }
            </ListState>
          </Step>
        ) : courseId !== null ? (
          <Step
            eyebrow={course?.code ?? "Course"}
            title={course?.name ?? "Blocks"}
            hint="Each block is a part of the course. Open one to see its units."
          >
            <ListState state={blocks} empty="No blocks have been added to this course yet.">
              {(items) =>
                items.map((b) => (
                  <button
                    key={b.block_id}
                    type="button"
                    onClick={() => go({ block: b.block_id })}
                    className="w-full rounded-xl border border-white/10 bg-white/[0.03] p-4 text-left transition hover:border-white/25 hover:bg-white/[0.06] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400"
                  >
                    <p className="text-[12px] font-semibold uppercase tracking-[0.12em] text-gray-500">
                      Block {b.block_no}
                    </p>
                    <p className="mt-1 text-[16px] font-semibold leading-snug text-white">
                      {b.title}
                    </p>
                    <p className="mt-1.5 text-[13px] text-gray-400">
                      {b.unit_count === 0
                        ? "No units uploaded yet"
                        : b.ready_unit_count === 0
                          ? `${b.unit_count} unit${b.unit_count === 1 ? "" : "s"} · still being prepared`
                          : `${b.ready_unit_count} of ${b.unit_count} unit${b.unit_count === 1 ? "" : "s"} ready`}
                    </p>
                  </button>
                ))
              }
            </ListState>
          </Step>
        ) : (
          <Step
            eyebrow="Classroom"
            title="Choose a course"
            hint="These are the courses you are enrolled in this semester."
          >
            <ListState
              state={courses}
              empty="You are not enrolled in any courses yet. An administrator can add you to one."
            >
              {(items) =>
                items.map((c) => (
                  <button
                    key={c.course_id}
                    type="button"
                    onClick={() => go({ course: c.course_id, block: null })}
                    className="w-full rounded-xl border border-white/10 bg-white/[0.03] p-4 text-left transition hover:border-white/25 hover:bg-white/[0.06] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400"
                  >
                    <p className="text-[12px] font-semibold uppercase tracking-[0.12em] text-gray-500">
                      {c.code} · {c.semester_name}
                    </p>
                    <p className="mt-1 text-[16px] font-semibold leading-snug text-white">
                      {c.name}
                    </p>
                    <p className="mt-1.5 text-[13px] text-gray-400">
                      {c.block_count === 0
                        ? "No blocks yet"
                        : `${c.block_count} block${c.block_count === 1 ? "" : "s"}${
                            c.teachable_block_count < c.block_count
                              ? ` · ${c.teachable_block_count} ready to study`
                              : ""
                          }`}
                    </p>
                  </button>
                ))
              }
            </ListState>
          </Step>
        )}
      </div>
    </main>
  );
}

function UnitRow({ unit, blockId }: { unit: SyllabusUnit; blockId: string }) {
  const href = `/classroom/session?block=${encodeURIComponent(blockId)}&unit=${encodeURIComponent(unit.document_id)}`;

  return (
    <div className="flex flex-wrap items-center gap-3 rounded-xl border border-white/10 bg-white/[0.03] p-4">
      <div className="min-w-0 flex-1">
        <p className="truncate text-[16px] font-semibold leading-snug text-white">{unit.title}</p>
        {/*
          The progress bar replaces the old "0 pages · still being prepared"
          line, which could not distinguish a queue that was moving from one
          that had stalled — or either from a PDF that failed hours ago.
        */}
        <IngestProgress
          className="mt-2 max-w-sm"
          status={unit.status}
          pageCount={unit.page_count}
        />
      </div>
      {unit.is_ready ? (
        <Link
          href={href}
          className="shrink-0 rounded-lg bg-indigo-500 px-4 py-2 text-[14px] font-semibold text-white hover:bg-indigo-400 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300"
        >
          Open classroom
        </Link>
      ) : (
        /*
          Deliberately not a disabled-looking link to nowhere: until the PDF is
          embedded there are no vectors, so the tutor would abstain on every
          question. Saying that is more use than a dead button.
        */
        <span
          title="This unit's material has not finished processing, so the tutor has nothing to teach from yet."
          className="shrink-0 rounded-lg border border-white/10 px-4 py-2 text-[13px] text-gray-500"
        >
          Not ready
        </span>
      )}
    </div>
  );
}

function Step({
  eyebrow,
  title,
  hint,
  children,
}: {
  eyebrow: string;
  title: string;
  hint: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <p className="text-[12px] font-semibold uppercase tracking-[0.14em] text-gray-500">
        {eyebrow}
      </p>
      <h1 className="mt-1 text-balance font-display text-[26px] font-bold leading-tight text-white">
        {title}
      </h1>
      <p className="mt-2 max-w-xl text-[15px] leading-relaxed text-gray-400">{hint}</p>
      <div className="mt-6 space-y-3">{children}</div>
    </section>
  );
}

function ListState<T>({
  state,
  empty,
  children,
}: {
  state: Load<T>;
  empty: string;
  children: (items: T[]) => React.ReactNode;
}) {
  if (state.status === "loading") {
    return (
      <div className="space-y-3" aria-busy>
        {[0, 1, 2].map((n) => (
          <div key={n} className="h-[86px] animate-pulse rounded-xl bg-white/[0.04]" />
        ))}
      </div>
    );
  }
  if (state.status === "error") {
    return <p className="text-[15px] text-rose-300">{state.message}</p>;
  }
  if (state.items.length === 0) {
    return <p className="text-[15px] text-gray-400">{empty}</p>;
  }
  return <>{children(state.items)}</>;
}

function Crumb({
  children,
  onClick,
  active = false,
}: {
  children: React.ReactNode;
  onClick?: () => void;
  active?: boolean;
}) {
  const className = active ? "font-semibold text-white" : "text-gray-400 hover:text-gray-200";
  if (!onClick || active) return <span className={className}>{children}</span>;
  return (
    <button type="button" onClick={onClick} className={className}>
      {children}
    </button>
  );
}
