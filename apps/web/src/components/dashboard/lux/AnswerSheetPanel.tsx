"use client";

/**
 * Marked papers, each downloadable as an answer sheet.
 *
 * The dashboard could already tell a student *how* they scored — the chart, the
 * course table, the weak topics — but not *what they got wrong*, and gave them
 * no way back to the marked paper. The review had been visible for one moment
 * on submission and was then unreachable, even though the gateway could still
 * serve it.
 *
 * Only marked attempts are listed. An `in_progress` attempt is still being sat
 * and an `abandoned` one was never graded; the gateway answers `409` for both,
 * so listing them here would be offering a dead link.
 */

import Link from "next/link";

import type { ExamAttemptRow } from "@/lib/student";

import { Eyebrow, SectionHeading, SubText, lightCard } from "./shell";
import { VizEmpty } from "./viz";

/** How many to show. The rest are on the Assessments screen. */
const VISIBLE = 6;

function marked(attempt: ExamAttemptRow): boolean {
  return attempt.status === "submitted" || attempt.status === "graded";
}

function formatDate(iso: string | null): string {
  if (iso === null) return "—";
  return new Date(iso).toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

export function AnswerSheetPanel({ attempts }: { attempts: ExamAttemptRow[] }) {
  const sheets = attempts.filter(marked).slice(0, VISIBLE);

  return (
    <section className={lightCard}>
      <Eyebrow>Your marked papers</Eyebrow>
      <SectionHeading>Answer sheets</SectionHeading>
      <SubText>
        Every question, your answer, the correct one and why — downloadable as a PDF.
      </SubText>

      <div className="mt-5">
        {sheets.length === 0 ? (
          <VizEmpty
            kind="empty"
            title="No marked papers yet"
            hint="Sit an assessment and submit it. The marked paper appears here, and stays available to download."
          />
        ) : (
          <ul className="divide-y divide-black/5">
            {sheets.map((attempt) => (
              <li
                key={attempt.id}
                className="flex flex-wrap items-baseline gap-x-3 gap-y-1 py-3 first:pt-0"
              >
                <div className="min-w-0 flex-1">
                  <p className="truncate text-[15px] font-semibold text-ink-900">
                    {attempt.exam_title}
                  </p>
                  <p className="mt-0.5 text-[13px] text-ink-500">
                    {attempt.course_code} · attempt {attempt.attempt_no} ·{" "}
                    {formatDate(attempt.submitted_at)}
                  </p>
                </div>
                <p className="shrink-0 text-[15px] font-semibold tabular-nums text-ink-900">
                  {attempt.score === null
                    ? "awaiting marks"
                    : `${attempt.score}/${attempt.max_score}`}
                </p>
                <Link
                  href={`/exams/attempts/${attempt.id}/sheet`}
                  className="shrink-0 rounded-full border border-black/10 px-3 py-1.5 text-[13px] font-semibold text-ink-900 transition hover:bg-black/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300"
                >
                  Answer sheet
                </Link>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
