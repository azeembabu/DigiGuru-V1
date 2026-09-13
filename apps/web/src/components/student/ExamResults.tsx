import Link from "next/link";

import {
  Pill,
  StudentEmpty,
  WeakTopicBadge,
  cardClass,
  formatDateTime,
  formatPercent,
  formatScore,
} from "@/components/student/parts";
import { OPTION_LETTERS, type ExamResult, type ReviewedQuestion } from "@/lib/student";

// The results screen. This is the ONLY place `correct_option_index` and
// `explanation` exist in the client at all: they arrive with the submit
// response and nowhere else (contract deviation 5), so an in-progress paper
// cannot leak the key even by accident — the paper type simply has no field
// for it.

export function ExamResults({ result }: { result: ExamResult }) {
  const pct = result.score_percentage;
  const tone = pct >= 70 ? "good" : pct >= 40 ? "warn" : "bad";

  return (
    <div className="space-y-6">
      <section className={`${cardClass} sm:!p-8`}>
        <p className="text-[12px] font-semibold uppercase tracking-[0.04em] text-gray-300/70">
          Submitted {formatDateTime(result.submitted_at)}
        </p>
        <h1 className="mt-2 text-balance font-display text-[28px] font-bold leading-[1.2] text-white sm:text-[34px]">
          {result.exam_title}
        </h1>

        <div className="mt-6 flex flex-wrap items-end gap-x-10 gap-y-4">
          <div>
            <p className="text-[13px] text-gray-300/70">Score</p>
            <p className="mt-1 font-display text-[34px] font-bold leading-none tabular-nums text-white">
              {formatScore(result.score, result.max_score)}
            </p>
          </div>
          <div>
            <p className="text-[13px] text-gray-300/70">Percentage</p>
            <p className="mt-1 font-display text-[34px] font-bold leading-none tabular-nums text-white">
              {formatPercent(pct)}
            </p>
          </div>
          <div>
            <p className="text-[13px] text-gray-300/70">Correct</p>
            <p className="mt-1 font-display text-[34px] font-bold leading-none tabular-nums text-white">
              {result.correct_answers}
              <span className="text-[20px] text-gray-300/70">/{result.total_questions}</span>
            </p>
          </div>
          <Pill tone={tone}>
            {pct >= 70 ? "Strong pass" : pct >= 40 ? "Needs revision" : "Needs a re-read"}
          </Pill>
        </div>

        {result.weak_topics.length > 0 ? (
          <div className="mt-6 border-t border-art-mid/60 pt-5">
            <p className="text-[14px] font-semibold text-white">Topics to go back over</p>
            <p className="mt-1 text-[14px] leading-[1.55] text-gray-300/70">
              Taken from the questions you missed. Ask your tutor about any of these by name.
            </p>
            <div className="mt-3 flex flex-wrap gap-2">
              {result.weak_topics.map((topic) => (
                <WeakTopicBadge key={topic} topic={topic} />
              ))}
            </div>
          </div>
        ) : null}

        <div className="mt-7 flex flex-wrap gap-3">
          <Link
            href="/exams"
            className="inline-flex items-center rounded-full bg-lime-400 px-5 py-2.5 text-[15px] font-semibold text-ink-950 transition-colors hover:bg-lime-300 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base"
          >
            Back to exams
          </Link>
          <Link
            href="/dashboard"
            className="inline-flex items-center rounded-full border-[1.5px] border-art-edge px-5 py-2 text-[15px] font-semibold text-white transition-colors hover:bg-art-mid/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base"
          >
            Study this again
          </Link>
        </div>
      </section>

      <section>
        <h2 className="font-display text-[22px] font-bold leading-[1.3] text-white">
          Every question, answered
        </h2>
        <p className="mt-1 text-[15px] leading-[1.6] text-gray-300/70">
          Your answer, the correct one, and why.
        </p>

        <div className="mt-5 space-y-4">
          {result.review.length === 0 ? (
            <div className={cardClass}>
              <StudentEmpty
                title="No per-question breakdown was returned"
                hint="Your score above is the graded result. Ask your coordinator if the question-by-question review stays missing."
              />
            </div>
          ) : (
            result.review.map((row) => <ReviewRow key={row.question_seq} row={row} />)
          )}
        </div>
      </section>
    </div>
  );
}

function ReviewRow({ row }: { row: ReviewedQuestion }) {
  return (
    <article
      className={`${cardClass} ${row.is_correct ? "!border-lime-400/30" : "!border-danger/40"}`}
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-[13px] font-semibold text-gray-300/70">Question {row.question_seq}</p>
        <div className="flex flex-wrap items-center gap-2">
          <Pill>{row.topic}</Pill>
          <Pill tone={row.is_correct ? "good" : "bad"}>
            {row.is_correct ? "Correct" : row.selected_option_index === null ? "Skipped" : "Wrong"}
          </Pill>
        </div>
      </div>

      <p className="mt-3 text-[17px] font-semibold leading-[1.5] text-white">{row.question_text}</p>

      <ul className="mt-4 space-y-2">
        {row.options.map((option, index) => {
          const chosen = row.selected_option_index === index;
          const correct = row.correct_option_index === index;

          // Shape and words carry the meaning, not just the border colour
          // (DESIGN.md §8) — each marked row names itself in text.
          const style = correct
            ? "border-lime-400/60 bg-lime-400/10 text-white"
            : chosen
              ? "border-danger/60 bg-danger/10 text-white"
              : "border-art-mid/60 text-gray-300";

          return (
            <li
              key={index}
              className={`flex items-start gap-3 rounded-[10px] border px-3 py-2.5 text-[15px] leading-[1.5] ${style}`}
            >
              <span className="mt-px font-mono text-[13px] font-semibold text-gray-300/70">
                {OPTION_LETTERS[index] ?? index + 1}
              </span>
              <span className="min-w-0 flex-1">{option}</span>
              {correct ? (
                <span className="shrink-0 text-[12px] font-semibold uppercase tracking-wide text-lime-300">
                  Correct
                </span>
              ) : chosen ? (
                <span className="shrink-0 text-[12px] font-semibold uppercase tracking-wide text-red-200">
                  Your answer
                </span>
              ) : null}
            </li>
          );
        })}
      </ul>

      <div className="mt-4 rounded-[10px] border border-art-mid/60 bg-art-base/40 px-3 py-3">
        <p className="text-[12px] font-semibold uppercase tracking-[0.04em] text-gray-300/70">
          Why
        </p>
        <p className="mt-1 text-[15px] leading-[1.6] text-gray-300">
          {/* AMENDMENT A1.1 made `explanation` NOT NULL, so this fallback should
              be unreachable. It stays because a silent blank box would be a
              worse answer than saying the rationale is missing. */}
          {row.explanation ?? "No rationale was recorded for this question."}
        </p>
      </div>
    </article>
  );
}
