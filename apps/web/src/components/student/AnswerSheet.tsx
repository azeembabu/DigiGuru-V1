"use client";

/**
 * The student's own marked answer sheet, as a printable document.
 *
 * Download is `window.print()` against a print stylesheet, not a PDF library —
 * the same decision the classroom note export makes, for the same reason: a
 * question, an option or an explanation may be in Malayalam, every PDF library
 * needs the font embedded to write it, and a download that silently drops the
 * script half the syllabus is taught in is worse than no download. Printing
 * uses the fonts already on the page, and "Save as PDF" is a destination in
 * every browser's print dialogue.
 *
 * The page is safe to open repeatedly: the gateway re-reads the stored marks
 * rather than re-grading, so nothing here can change a score.
 */

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";

import { loadAnswerSheet, type AnswerSheet as Sheet } from "@/lib/student";
import { ApiError } from "@/lib/api";

const OPTION_LETTERS = ["A", "B", "C", "D"] as const;

function letter(index: number | null): string {
  if (index === null || index < 0 || index >= OPTION_LETTERS.length) return "—";
  return OPTION_LETTERS[index];
}

function formatDateTime(iso: string): string {
  return new Date(iso).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

export function AnswerSheetView({ attemptId }: { attemptId: string }) {
  const [sheet, setSheet] = useState<Sheet | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    loadAnswerSheet(attemptId)
      .then((data) => {
        if (!cancelled) setSheet(data);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        // 409 is the one failure worth explaining rather than reporting: the
        // attempt exists and belongs to them, it simply has not been marked.
        setError(
          err instanceof ApiError
            ? err.message
            : "This answer sheet could not be loaded.",
        );
      });
    return () => {
      cancelled = true;
    };
  }, [attemptId]);

  const download = useCallback(() => window.print(), []);

  if (error) {
    return (
      <div className="mx-auto max-w-2xl px-4 py-16 text-center">
        <h1 className="font-display text-[22px] font-bold text-white">
          Answer sheet unavailable
        </h1>
        <p className="mt-2 text-[15px] text-gray-300">{error}</p>
        <Link href="/dashboard" className="mt-6 inline-block text-[14px] text-lime-400 underline">
          Back to dashboard
        </Link>
      </div>
    );
  }

  if (!sheet) {
    return (
      <p className="px-4 py-16 text-center text-[15px] text-gray-400">
        Loading your answer sheet…
      </p>
    );
  }

  return (
    <div className="mx-auto max-w-3xl px-4 py-8 sm:px-6">
      {/* Screen-only chrome: nothing here belongs on the printed page. */}
      <div className="dg-sheet-chrome mb-6 flex flex-wrap items-center gap-3">
        <Link href="/dashboard" className="text-[14px] text-gray-400 hover:text-gray-200">
          ← Dashboard
        </Link>
        <button
          type="button"
          onClick={download}
          className="ml-auto rounded-lg bg-indigo-500 px-4 py-2 text-[14px] font-semibold text-white hover:bg-indigo-400"
        >
          Download PDF
        </button>
      </div>

      <article className="dg-answer-sheet rounded-2xl border border-white/10 bg-[#101512] p-6 text-gray-100 sm:p-8">
        <header className="border-b border-white/10 pb-4">
          <h1 className="font-display text-[22px] font-bold leading-tight text-white">
            {sheet.exam_title}
          </h1>
          <p className="mt-1 text-[13px] text-gray-400">
            {sheet.course_code} · {sheet.course_name} · Block {sheet.block_no} —{" "}
            {sheet.block_title}
          </p>
          <dl className="mt-4 grid gap-x-6 gap-y-1 text-[14px] sm:grid-cols-2">
            <Row label="Student" value={sheet.student_name} />
            <Row label="Roll number" value={sheet.roll_number} />
            <Row label="Attempt" value={String(sheet.attempt_no)} />
            <Row label="Submitted" value={formatDateTime(sheet.submitted_at)} />
          </dl>
        </header>

        <section className="flex flex-wrap items-baseline gap-x-6 gap-y-2 border-b border-white/10 py-4">
          <p className="font-display text-[28px] font-bold leading-none tabular-nums text-white">
            {sheet.score}
            <span className="text-[18px] text-gray-400"> / {sheet.max_score}</span>
          </p>
          <p className="text-[15px] tabular-nums text-gray-300">
            {sheet.correct_answers} of {sheet.total_questions} correct ·{" "}
            {sheet.score_percentage.toFixed(0)}%
          </p>
          {sheet.weak_topics.length > 0 ? (
            <p className="text-[13px] text-amber-300/90">
              To revise: {sheet.weak_topics.join(", ")}
            </p>
          ) : null}
        </section>

        <ol className="mt-2 divide-y divide-white/10">
          {sheet.review.map((q) => (
            <li key={q.question_seq} className="dg-sheet-question py-4">
              <p className="text-[15px] font-semibold leading-snug text-white">
                {q.question_seq}. {q.question_text}
              </p>
              <p className="mt-0.5 text-[12px] uppercase tracking-wide text-gray-500">
                {q.topic}
              </p>

              <ul className="mt-2 space-y-1">
                {q.options.map((option, index) => {
                  const chosen = q.selected_option_index === index;
                  const isKey = q.correct_option_index === index;
                  return (
                    <li
                      key={index}
                      className={`text-[14px] leading-snug ${
                        isKey
                          ? "text-emerald-300"
                          : chosen
                            ? "text-rose-300"
                            : "text-gray-400"
                      }`}
                    >
                      <span className="mr-1.5 font-semibold">{OPTION_LETTERS[index]}.</span>
                      {option}
                      {/* Marked in text, not only in colour: this is printed,
                          often in greyscale, and a colour-only mark would be
                          the difference between a record and a puzzle. */}
                      {isKey ? <span className="ml-2 text-[12px]">✓ correct answer</span> : null}
                      {chosen && !isKey ? (
                        <span className="ml-2 text-[12px]">✗ your answer</span>
                      ) : null}
                    </li>
                  );
                })}
              </ul>

              <p className="mt-2 text-[13px] leading-relaxed text-gray-300">
                <b className="text-gray-200">
                  {q.is_correct
                    ? "Correct"
                    : q.selected_option_index === null
                      ? `Not answered — the answer is ${letter(q.correct_option_index)}`
                      : `Incorrect — you chose ${letter(q.selected_option_index)}, the answer is ${letter(q.correct_option_index)}`}
                  .{" "}
                </b>
                {q.explanation}
              </p>
            </li>
          ))}
        </ol>
      </article>
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex gap-2">
      <dt className="text-gray-500">{label}:</dt>
      <dd className="text-gray-200">{value}</dd>
    </div>
  );
}
