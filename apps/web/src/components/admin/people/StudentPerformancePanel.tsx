"use client";

// One student's performance record, on the admin's student page.
//
// This reads the SAME derivation the student's own assessment page reads —
// same rows, same bands, same stars — because both call a gateway that
// computes them once from `exam_attempts` and `learning_sessions`. There is no
// admin-side performance store to fall behind, which is the same argument
// `student_reports` makes for having no mirrored gradebook.
//
// The student's remark is shown but never editable. It is their writing; an
// admin who could edit it would turn a reflection into a record.

import { useEffect, useState } from "react";

import { Banner, panelClass } from "@/components/admin/primitives";
import { getStudentPerformance } from "@/lib/admin/client";
import type {
  BlockPerformance,
  PerformanceLevel,
  StudentPerformance,
} from "@/lib/admin/types";
import { ApiError } from "@/lib/api";

/** Light-theme tones — the admin console is light where the student app is
 *  dark, so these do not share the student's `LEVEL_TONE` map. */
const LEVEL_TONE: Record<PerformanceLevel, string> = {
  not_assessed: "bg-gray-100 text-gray-600 ring-gray-200",
  needs_work: "bg-rose-50 text-rose-700 ring-rose-200",
  developing: "bg-amber-50 text-amber-700 ring-amber-200",
  proficient: "bg-sky-50 text-sky-700 ring-sky-200",
  strong: "bg-emerald-50 text-emerald-700 ring-emerald-200",
  excellent: "bg-violet-50 text-violet-700 ring-violet-200",
};

/** `83.0%`, or an em dash when there is nothing measured. Never `0%` for a
 *  missing value — that would be a different and wrong claim. */
function percent(value: number | null): string {
  return value === null ? "—" : `${value.toFixed(1)}%`;
}

function Stars({ stars }: { stars: number | null }) {
  if (stars === null) {
    return <span className="text-xs italic text-gray-400">Not assessed</span>;
  }
  return (
    <span role="img" aria-label={`${stars} out of 5 stars`} className="text-amber-500">
      {"★".repeat(stars)}
      <span className="text-gray-300">{"★".repeat(5 - stars)}</span>
    </span>
  );
}

type Outcome =
  | { status: "error"; message: string }
  | { status: "ready"; data: StudentPerformance };

export function StudentPerformancePanel({ studentId }: { studentId: string }) {
  // One tagged state rather than separate `data`/`error`, and written only
  // from the async callbacks. Two effects' worth of `setState` in the effect
  // body would both cascade a render and leave a window where a previous
  // student's error was shown against a new student's id; tagging the result
  // with the id it answers closes that instead.
  const [result, setResult] = useState<{ studentId: string; outcome: Outcome } | null>(null);

  useEffect(() => {
    let cancelled = false;
    const settle = (outcome: Outcome) => {
      if (!cancelled) setResult({ studentId, outcome });
    };

    getStudentPerformance(studentId)
      .then((data) => settle({ status: "ready", data }))
      .catch((caught: unknown) => {
        settle({
          status: "error",
          message:
            caught instanceof ApiError
              ? caught.message
              : "This student's performance could not be loaded.",
        });
      });

    return () => {
      cancelled = true;
    };
  }, [studentId]);

  // A result for a different student is as good as no result: show the
  // skeleton rather than the previous student's figures.
  if (result === null || result.studentId !== studentId) {
    return (
      <section className={`${panelClass} p-4`}>
        <h2 className="text-sm font-semibold text-gray-900">Performance</h2>
        <div className="mt-3 h-32 animate-pulse rounded-lg bg-gray-100" aria-hidden="true" />
      </section>
    );
  }

  if (result.outcome.status === "error") {
    return (
      <section className={`${panelClass} p-4`}>
        <h2 className="text-sm font-semibold text-gray-900">Performance</h2>
        <div className="mt-3">
          <Banner tone="danger">{result.outcome.message}</Banner>
        </div>
      </section>
    );
  }

  const { summary, blocks } = result.outcome.data;

  return (
    <section className={`${panelClass} p-4`}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold text-gray-900">Performance</h2>
          <p className="mt-0.5 text-xs text-gray-500">
            Derived from graded exams and tutoring sessions — the same figures the
            student sees.
          </p>
        </div>
        <div className="text-right">
          <p className="text-2xl font-bold text-gray-900">
            {percent(summary.average_percentage)}
          </p>
          <span
            className={`mt-1 inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${LEVEL_TONE[summary.level]}`}
          >
            {summary.level_label}
          </span>
        </div>
      </div>

      <dl className="mt-4 grid grid-cols-2 gap-3 border-t border-gray-200 pt-3 sm:grid-cols-4">
        <Stat label="Graded exams" value={String(summary.attempts_graded)} />
        <Stat
          label="Blocks assessed"
          value={`${summary.blocks_assessed} of ${summary.blocks_total}`}
        />
        <Stat label="Best score" value={percent(summary.best_percentage)} />
        <Stat label="Rating" value={<Stars stars={summary.stars} />} />
      </dl>

      {blocks.length === 0 ? (
        <p className="mt-4 text-sm text-gray-500">
          This student has no active enrolments with blocks yet.
        </p>
      ) : (
        <div className="mt-4 overflow-x-auto">
          <table className="w-full min-w-[640px] text-left text-sm">
            <thead>
              <tr className="border-b border-gray-200 text-xs uppercase tracking-wide text-gray-500">
                <th scope="col" className="py-2 pr-3 font-medium">Block</th>
                <th scope="col" className="py-2 pr-3 font-medium">Level</th>
                <th scope="col" className="py-2 pr-3 font-medium">Average</th>
                <th scope="col" className="py-2 pr-3 font-medium">Best</th>
                <th scope="col" className="py-2 pr-3 font-medium">Exams</th>
                <th scope="col" className="py-2 font-medium">Rating</th>
              </tr>
            </thead>
            <tbody>
              {blocks.map((block) => (
                <BlockRow key={block.block_id} block={block} />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

function BlockRow({ block }: { block: BlockPerformance }) {
  return (
    <>
      <tr className="border-b border-gray-100 align-top">
        <td className="py-2 pr-3">
          <span className="block font-medium text-gray-900">
            Block {block.block_no}: {block.block_title}
          </span>
          <span className="block text-xs text-gray-500">{block.course_code}</span>
        </td>
        <td className="py-2 pr-3">
          <span
            className={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ring-1 ${LEVEL_TONE[block.level]}`}
          >
            {block.level_label}
          </span>
        </td>
        <td className="py-2 pr-3 text-gray-900">{percent(block.average_percentage)}</td>
        <td className="py-2 pr-3 text-gray-900">{percent(block.best_percentage)}</td>
        <td className="py-2 pr-3 text-gray-700">
          {block.attempts_graded} / {block.exams_available}
        </td>
        <td className="py-2">
          <Stars stars={block.stars} />
        </td>
      </tr>
      {block.remark !== null ? (
        // The student's own words, in their own row so they are not mistaken
        // for an admin annotation on the block.
        <tr className="border-b border-gray-100">
          <td colSpan={6} className="pb-3 pl-3">
            <p className="text-xs font-medium uppercase tracking-wide text-gray-500">
              Student&apos;s note
            </p>
            <p className="mt-0.5 whitespace-pre-wrap text-sm text-gray-700">
              {block.remark}
            </p>
          </td>
        </tr>
      ) : null}
    </>
  );
}

function Stat({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div>
      <dt className="text-xs font-medium uppercase tracking-wide text-gray-500">{label}</dt>
      <dd className="mt-0.5 text-sm font-semibold text-gray-900">{value}</dd>
    </div>
  );
}
