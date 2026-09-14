"use client";

// The student's assessment page: how they have performed on each block of
// their enrolled courses, and their own written remark against each one.
//
// Everything on this page except the remark is DERIVED, not stored — the
// gateway computes it from the same `exam_attempts` and `learning_sessions`
// rows the admin's view of this student reads. That is why there is no
// "refresh my scores" action: there is nothing to recompute into, and the two
// screens cannot fall out of step.
//
// The one thing the page owns is the remark, and the only write on this page
// is the student's own words.

import { useCallback, useEffect, useState } from "react";

import { ApiError } from "@/lib/api";
import {
  LEVEL_TONE,
  SchemaError,
  loadPerformance,
  saveBlockRemark,
  type BlockPerformance,
  type StudentPerformance,
} from "@/lib/student";
import {
  PanelSkeleton,
  SectionHeader,
  StudentEmpty,
  StudentError,
  WeakTopicBadge,
  cardClass,
  formatDate,
  formatDuration,
  formatPercent,
} from "@/components/student/parts";
import { StarRating, TrophyBadge } from "@/components/student/StarRating";
import { UnitAssessments } from "@/components/student/UnitAssessments";

type Outcome =
  | { status: "error"; message: string; retryable: boolean }
  | { status: "ready"; data: StudentPerformance };

export function AssessmentScreen() {
  const [attempt, setAttempt] = useState(0);
  // Tagged with the attempt it answers, so a slow first response cannot
  // overwrite a newer retry's result.
  const [result, setResult] = useState<{ attempt: number; outcome: Outcome } | null>(null);

  useEffect(() => {
    let cancelled = false;
    const setState = (outcome: Outcome) => {
      if (!cancelled) setResult({ attempt, outcome });
    };

    loadPerformance()
      .then((data) => setState({ status: "ready", data }))
      .catch((caught: unknown) => {
        if (caught instanceof ApiError) {
          setState({ status: "error", message: caught.message, retryable: true });
        } else if (caught instanceof SchemaError) {
          // A schema mismatch is not retryable: the same request will return
          // the same unreadable body, and a "try again" button that cannot
          // work is worse than none.
          setState({ status: "error", message: caught.message, retryable: false });
        } else {
          setState({
            status: "error",
            message: "Something went wrong loading your assessment.",
            retryable: true,
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [attempt]);

  const retry = useCallback(() => setAttempt((n) => n + 1), []);

  if (result === null || result.attempt !== attempt) {
    return (
      <div className="space-y-4">
        <PanelSkeleton className="h-32" />
        <PanelSkeleton className="h-64" />
      </div>
    );
  }

  if (result.outcome.status === "error") {
    return (
      <StudentError
        title="Your assessment could not be loaded"
        message={result.outcome.message}
        onRetry={result.outcome.retryable ? retry : undefined}
      />
    );
  }

  const { summary, blocks } = result.outcome.data;

  return (
    <div className="space-y-6">
      <OverallCard summary={summary} />

      {/* The tutor's conversational feedback, above the exam-derived block
          cards: it is what the student came to read after a session, and it is
          the more recent of the two. */}
      <UnitAssessments />

      <section className="space-y-3">
        <SectionHeader
          title="Your blocks"
          hint="Your exam results for each block, and your own notes on it."
        />
        {blocks.length === 0 ? (
          <StudentEmpty
            title="No blocks yet"
            hint="Once your courses have blocks, each one appears here with your results."
          />
        ) : (
          <ul className="space-y-3">
            {blocks.map((block) => (
              <li key={block.block_id}>
                <BlockCard block={block} />
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

function OverallCard({ summary }: { summary: StudentPerformance["summary"] }) {
  return (
    <section className={`${cardClass} p-5`}>
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <p className="text-[13px] font-semibold uppercase tracking-wide text-gray-400">
            Overall
          </p>
          <p className="mt-1 text-[28px] font-bold leading-tight text-white">
            {formatPercent(summary.average_percentage)}
          </p>
          {/* The label comes from the wire so the wording cannot drift from
              the admin's screen. */}
          <span
            className={`mt-2 inline-flex rounded-full px-2.5 py-0.5 text-[12px] font-semibold ring-1 ${LEVEL_TONE[summary.level]}`}
          >
            {summary.level_label}
          </span>
        </div>

        <div className="flex flex-col items-end gap-2">
          <StarRating
            stars={summary.stars}
            label={`Overall rating: ${summary.level_label}`}
          />
          <TrophyBadge
            trophy={summary.trophy}
            attemptsUntil={summary.attempts_until_trophy}
          />
        </div>
      </div>

      <dl className="mt-5 grid grid-cols-2 gap-4 border-t border-art-mid/60 pt-4 sm:grid-cols-4">
        <Stat label="Graded exams" value={String(summary.attempts_graded)} />
        <Stat
          label="Blocks assessed"
          value={`${summary.blocks_assessed} of ${summary.blocks_total}`}
        />
        <Stat label="Best score" value={formatPercent(summary.best_percentage)} />
        <Stat label="Time studied" value={formatDuration(summary.total_active_voice_ms)} />
      </dl>
    </section>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-[12px] font-semibold uppercase tracking-wide text-gray-400">
        {label}
      </dt>
      <dd className="mt-0.5 text-[16px] font-semibold text-white">{value}</dd>
    </div>
  );
}

function BlockCard({ block }: { block: BlockPerformance }) {
  return (
    <article className={`${cardClass} p-4`}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="text-[12px] font-semibold uppercase tracking-wide text-gray-400">
            {block.course_code} · Semester {block.semester_number}
          </p>
          <h3 className="mt-0.5 text-[16px] font-semibold text-white">
            Block {block.block_no}: {block.block_title}
          </h3>
        </div>
        <div className="flex flex-col items-end gap-1.5">
          <StarRating
            stars={block.stars}
            size="sm"
            label={`Block ${block.block_no}: ${block.level_label}`}
          />
          <span
            className={`inline-flex rounded-full px-2.5 py-0.5 text-[12px] font-semibold ring-1 ${LEVEL_TONE[block.level]}`}
          >
            {block.level_label}
          </span>
        </div>
      </div>

      <dl className="mt-4 grid grid-cols-2 gap-3 sm:grid-cols-4">
        <Stat label="Average" value={formatPercent(block.average_percentage)} />
        <Stat label="Best" value={formatPercent(block.best_percentage)} />
        <Stat
          label="Exams sat"
          value={`${block.attempts_graded} of ${block.exams_available}`}
        />
        <Stat label="Last studied" value={formatDate(block.last_studied_at)} />
      </dl>

      {block.weak_topics.length > 0 ? (
        <div className="mt-4">
          <p className="text-[12px] font-semibold uppercase tracking-wide text-gray-400">
            Topics to revise
          </p>
          <div className="mt-2 flex flex-wrap gap-2">
            {block.weak_topics.map((topic) => (
              <WeakTopicBadge key={topic.topic} topic={topic.topic} />
            ))}
          </div>
        </div>
      ) : null}

      <RemarkEditor block={block} />
    </article>
  );
}

/**
 * The student's own note against a block.
 *
 * Saves explicitly rather than on a debounce: this is the student's writing,
 * and a half-typed sentence silently persisting is both surprising and hard to
 * undo. The button reports the outcome in place — a save that failed must not
 * look like one that worked.
 */
function RemarkEditor({ block }: { block: BlockPerformance }) {
  const [text, setText] = useState(block.remark ?? "");
  const [savedAt, setSavedAt] = useState<string | null>(block.remark_updated_at);
  const [state, setState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [error, setError] = useState<string | null>(null);

  // What is currently persisted, tracked so the button can be disabled when
  // there is genuinely nothing to save.
  const [persisted, setPersisted] = useState(block.remark ?? "");
  const dirty = text.trim() !== persisted.trim();

  const save = useCallback(async () => {
    setState("saving");
    setError(null);
    try {
      const saved = await saveBlockRemark(block.block_id, text);
      setPersisted(saved.remark ?? "");
      setSavedAt(saved.remark_updated_at);
      setState("saved");
    } catch (caught: unknown) {
      setError(
        caught instanceof ApiError || caught instanceof SchemaError
          ? caught.message
          : "Your note could not be saved.",
      );
      setState("error");
    }
  }, [block.block_id, text]);

  return (
    <div className="mt-4 border-t border-art-mid/60 pt-4">
      <label
        htmlFor={`remark-${block.block_id}`}
        className="text-[12px] font-semibold uppercase tracking-wide text-gray-400"
      >
        Your notes on this block
      </label>
      <textarea
        id={`remark-${block.block_id}`}
        value={text}
        onChange={(event) => {
          setText(event.target.value);
          // Clearing a stale "Saved" the moment the text changes again: the
          // label must describe what is stored, not what was stored once.
          if (state !== "idle") setState("idle");
        }}
        rows={3}
        maxLength={2000}
        placeholder="What went well? What do you want to come back to?"
        className="mt-2 w-full rounded-[10px] border border-art-mid bg-art-base/60 px-3 py-2 text-[14px] leading-[1.55] text-white placeholder:text-gray-500 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300"
      />
      <div className="mt-2 flex flex-wrap items-center gap-3">
        <button
          type="button"
          onClick={save}
          disabled={!dirty || state === "saving"}
          className="rounded-full border-[1.5px] border-art-edge px-4 py-1.5 text-[14px] font-semibold text-white transition-colors hover:bg-art-mid/50 disabled:cursor-not-allowed disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300"
        >
          {state === "saving" ? "Saving…" : "Save note"}
        </button>

        {/* Announced politely so a save outcome reaches a screen reader
            without stealing focus from the textarea. */}
        <span role="status" aria-live="polite" className="text-[13px]">
          {state === "error" ? (
            <span className="text-red-300">{error}</span>
          ) : state === "saved" ? (
            <span className="text-lime-300">
              {persisted === "" ? "Note cleared" : "Saved"}
            </span>
          ) : savedAt !== null ? (
            <span className="text-gray-400">Last saved {formatDate(savedAt)}</span>
          ) : null}
        </span>
      </div>
    </div>
  );
}
