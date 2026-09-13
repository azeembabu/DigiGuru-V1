"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useRef, useState } from "react";

import { ApiError } from "@/lib/api";
import {
  ASSESSMENT_TYPE_LABEL,
  OPTION_LETTERS,
  SchemaError,
  findActiveAttemptId,
  loadExamAttempt,
  loadStudentExams,
  saveExamAnswers,
  startExamAttempt,
  submitExamAttempt,
  type ExamPaper,
  type ExamResult,
  type StudentExam,
} from "@/lib/student";
import {
  AssessmentTypePill,
  PanelSkeleton,
  Pill,
  StudentError,
  cardClass,
  formatClock,
} from "@/components/student/parts";
import { ExamResults } from "@/components/student/ExamResults";

// The exam runner: one question at a time, A/B/C/D, save-as-you-go, submit,
// then the rationale screen.
//
// Two rules shape the whole component:
//
// 1. The server owns the paper. Selections are held locally only so the radio
//    responds instantly; every one is PATCHed to
//    `/student/exams/attempts/{id}/answers`, which is idempotent, and a reload
//    re-reads the attempt rather than trusting anything kept here.
// 2. The countdown is DISPLAY ONLY. It is derived from the server's
//    `started_at` plus `time_limit_minutes`, and when it hits zero this
//    component does not submit, lock, or discard anything — the gateway decides
//    when an attempt has expired. A device with a fast clock must not be able
//    to end a student's exam early, and one with a slow clock must not be able
//    to extend it.

type Phase =
  | { kind: "loading" }
  | { kind: "error"; message: string; retryable: boolean }
  | { kind: "intro"; exam: StudentExam | null }
  | { kind: "starting" }
  | { kind: "running"; paper: ExamPaper }
  | { kind: "done"; result: ExamResult };

type SaveState = "idle" | "saving" | "saved" | "failed";

function messageFor(caught: unknown, fallback: string): string {
  if (caught instanceof SchemaError || caught instanceof ApiError) return caught.message;
  return fallback;
}

/** A1.4: `time_limit_minutes` is the countdown source, `duration_minutes` the
 *  pre-amendment name. Either may be absent — an exam need not be timed. */
function limitMinutes(paper: ExamPaper): number | null {
  return paper.time_limit_minutes ?? paper.duration_minutes ?? null;
}

export function ExamRunner({ examId }: { examId: string }) {
  const router = useRouter();
  const [attempt, setAttempt] = useState(0);
  // Tagged with the load attempt it belongs to, so the initial "loading" state
  // is derived during render instead of being set synchronously in the effect.
  const [tagged, setTagged] = useState<{ attempt: number; phase: Phase } | null>(null);
  const phase: Phase =
    tagged !== null && tagged.attempt === attempt ? tagged.phase : { kind: "loading" };
  const setPhase = useCallback((next: Phase) => setTagged({ attempt, phase: next }), [attempt]);

  // Load the exam's own metadata so the student sees what they are about to
  // start before an attempt row exists. There is no `GET /student/exams/{id}`
  // in the contract, so this reads the list they are already entitled to and
  // picks the row out — it never fabricates the numbers.
  useEffect(() => {
    let cancelled = false;

    loadStudentExams({ limit: 200 })
      .then((page) => {
        if (cancelled) return;
        const exam = page.items.find((row) => row.id === examId) ?? null;
        setTagged({ attempt, phase: { kind: "intro", exam } });
      })
      .catch((caught: unknown) => {
        if (cancelled) return;
        if (caught instanceof ApiError && caught.status === 401) {
          router.replace("/login");
          return;
        }
        // The intro can still be offered without the metadata: starting the
        // attempt is a separate call that the gateway authorises on its own.
        setTagged({ attempt, phase: { kind: "intro", exam: null } });
      });

    return () => {
      cancelled = true;
    };
  }, [examId, router, attempt]);

  const begin = useCallback(async () => {
    setPhase({ kind: "starting" });

    try {
      setPhase({ kind: "running", paper: await startExamAttempt(examId) });
    } catch (caught: unknown) {
      // A 409 means an attempt is already running. The error envelope is
      // `{ code, message }` and cannot carry its id, so the live attempt is
      // looked up from the attempts list and resumed rather than reported.
      if (caught instanceof ApiError && caught.status === 409) {
        const activeId = await findActiveAttemptId(examId);
        if (activeId !== null) {
          try {
            setPhase({ kind: "running", paper: await loadExamAttempt(activeId) });
            return;
          } catch (resumeFailed: unknown) {
            setPhase({
              kind: "error",
              message: messageFor(resumeFailed, "Could not reopen your attempt."),
              retryable: true,
            });
            return;
          }
        }
      }

      setPhase({
        kind: "error",
        message: messageFor(caught, "Could not start this exam."),
        // 422 = the question pool is too small for this paper. Retrying will
        // not change that, so do not offer a button that cannot work.
        retryable: !(caught instanceof ApiError && caught.status === 422),
      });
    }
  }, [examId, setPhase]);

  if (phase.kind === "loading") {
    return <PanelSkeleton className="h-72" />;
  }

  if (phase.kind === "error") {
    return (
      <StudentError
        title="This exam could not be opened"
        message={phase.message}
        onRetry={phase.retryable ? () => setAttempt((n) => n + 1) : undefined}
      />
    );
  }

  if (phase.kind === "done") {
    return <ExamResults result={phase.result} />;
  }

  if (phase.kind === "running") {
    return (
      <Paper
        paper={phase.paper}
        onFinished={(result) => setPhase({ kind: "done", result })}
      />
    );
  }

  // `starting` is its own phase rather than a flag on `intro`, so the narrowing
  // above leaves only these two here.
  return (
    <Intro
      exam={phase.kind === "intro" ? phase.exam : null}
      starting={phase.kind === "starting"}
      onBegin={() => void begin()}
    />
  );
}

// ------------------------------------------------------------------- intro

function Intro({
  exam,
  starting,
  onBegin,
}: {
  exam: StudentExam | null;
  starting: boolean;
  onBegin: () => void;
}) {
  return (
    <section className={`${cardClass} sm:!p-8`}>
      <Link
        href="/exams"
        className="text-[14px] font-semibold text-lime-400 underline-offset-4 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base"
      >
        ← All exams
      </Link>

      {exam === null ? (
        <>
          <h1 className="mt-5 font-display text-[28px] font-bold leading-[1.2] text-white">
            Start this exam
          </h1>
          <p className="mt-3 max-w-xl text-[16px] leading-[1.6] text-gray-300">
            We could not read this exam&rsquo;s details, so the question count and time limit are not
            shown. Starting it will still work if the exam is published for one of your courses — if
            it is not, nothing will be created.
          </p>
        </>
      ) : (
        <>
          <div className="mt-5 flex flex-wrap items-center gap-2">
            <AssessmentTypePill type={exam.assessment_type} />
            {exam.attempts_used > 0 ? (
              <Pill tone="good">
                {exam.attempts_used} previous attempt{exam.attempts_used === 1 ? "" : "s"}
              </Pill>
            ) : null}
          </div>
          <h1 className="mt-3 text-balance font-display text-[28px] font-bold leading-[1.2] text-white sm:text-[34px]">
            {exam.title}
          </h1>
          <p className="mt-1 text-[15px] leading-[1.5] text-gray-300/70">
            {exam.course_code} · {exam.course_name} · Block {exam.block_no}
          </p>
          {exam.description ? (
            <p className="mt-4 max-w-xl text-[16px] leading-[1.6] text-gray-300">
              {exam.description}
            </p>
          ) : null}

          <dl className="mt-6 grid max-w-lg grid-cols-2 gap-4 sm:grid-cols-3">
            <Fact label="Questions" value={exam.question_count === null ? "—" : String(exam.question_count)} />
            <Fact
              label="Time limit"
              value={exam.duration_minutes === null ? "None" : `${exam.duration_minutes} min`}
            />
            <Fact label="Marks" value={String(exam.max_score)} />
          </dl>
        </>
      )}

      <ul className="mt-6 max-w-xl space-y-1.5 text-[15px] leading-[1.6] text-gray-300/80">
        <li>· One question at a time, four options. You can go back and change an answer.</li>
        <li>· Every answer is saved as you go, so a dropped connection loses nothing.</li>
        <li>· The clock on screen is a guide — your exam is timed on the server.</li>
        <li>· The correct answers and the reasoning appear as soon as you submit.</li>
      </ul>

      <button
        type="button"
        onClick={onBegin}
        disabled={starting}
        aria-busy={starting}
        className="mt-7 inline-flex items-center rounded-full bg-lime-400 px-6 py-3 text-[15px] font-semibold text-ink-950 transition-colors hover:bg-lime-300 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base disabled:cursor-not-allowed disabled:opacity-60"
      >
        {starting ? "Preparing your paper…" : "Begin exam"}
      </button>
    </section>
  );
}

function Fact({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-[10px] border border-art-mid/60 bg-art-base/40 px-3 py-2.5">
      <dt className="text-[12px] uppercase tracking-[0.04em] text-gray-300/70">{label}</dt>
      <dd className="mt-1 font-display text-[20px] font-bold leading-none tabular-nums text-white">
        {value}
      </dd>
    </div>
  );
}

// -------------------------------------------------------------------- paper

function Paper({
  paper,
  onFinished,
}: {
  paper: ExamPaper;
  onFinished: (result: ExamResult) => void;
}) {
  const [index, setIndex] = useState(0);
  // Seeded from the server's own `selected_option_index`, so a resumed attempt
  // opens with the answers it already has.
  const [answers, setAnswers] = useState<Map<number, number>>(() => {
    const seeded = new Map<number, number>();
    for (const q of paper.questions) {
      if (q.selected_option_index !== null) seeded.set(q.question_seq, q.selected_option_index);
    }
    return seeded;
  });
  const [save, setSave] = useState<SaveState>("idle");
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [remaining, setRemaining] = useState<number | null>(null);

  const minutes = limitMinutes(paper);

  // Display-only countdown. Recomputed from the server's `started_at` on every
  // tick rather than decremented, so a throttled background tab shows the right
  // time when it comes back instead of a clock that fell behind.
  useEffect(() => {
    if (minutes === null) return;
    const startedAt = new Date(paper.started_at).getTime();
    if (Number.isNaN(startedAt)) return;
    const deadline = startedAt + minutes * 60_000;

    const tick = () => setRemaining(deadline - Date.now());
    // Timeout for the first read, not a direct call: see `QuotaRing`.
    const first = setTimeout(tick, 0);
    const timer = setInterval(tick, 1000);
    return () => {
      clearTimeout(first);
      clearInterval(timer);
    };
  }, [minutes, paper.started_at]);

  const question = paper.questions[index];
  const answered = answers.size;
  const total = paper.questions.length;

  const choose = useCallback(
    (seq: number, option: number) => {
      setAnswers((current) => new Map(current).set(seq, option));
      setSave("saving");
      // Sent immediately rather than debounced: one PATCH per click is cheap,
      // the endpoint is idempotent, and a debounce would mean the last answer
      // before a closed tab was the one that did not reach the server.
      saveExamAnswers(paper.attempt_id, [{ question_seq: seq, selected_option_index: option }])
        .then(() => setSave("saved"))
        .catch(() => setSave("failed"));
    },
    [paper.attempt_id],
  );

  /** Re-send every answer held locally. The save endpoint takes a batch and is
   *  idempotent, so this is the honest repair for a failed save. */
  const retrySave = useCallback(() => {
    setSave("saving");
    saveExamAnswers(
      paper.attempt_id,
      [...answers].map(([question_seq, selected_option_index]) => ({
        question_seq,
        selected_option_index,
      })),
    )
      .then(() => setSave("saved"))
      .catch(() => setSave("failed"));
  }, [answers, paper.attempt_id]);

  const submittedRef = useRef(false);

  const submit = useCallback(async () => {
    if (submittedRef.current) return;
    submittedRef.current = true;
    setSubmitting(true);
    setSubmitError(null);

    try {
      onFinished(await submitExamAttempt(paper.attempt_id));
    } catch (caught: unknown) {
      // Let them try again — except on a 409, which means this attempt is
      // already submitted and a re-submit is not a re-grade.
      submittedRef.current = caught instanceof ApiError && caught.status === 409;
      setSubmitError(messageFor(caught, "Could not submit your exam."));
      setSubmitting(false);
    }
  }, [onFinished, paper.attempt_id]);

  if (question === undefined) {
    return (
      <StudentError
        title="This paper came back empty"
        message="The server started an attempt with no questions in it. Tell your coordinator — there may be too few questions in the pool for your semester."
      />
    );
  }

  const overdue = remaining !== null && remaining <= 0;

  return (
    <div className="space-y-5">
      {/* ------------------------------------------------------ status bar */}
      <div className={`${cardClass} !p-4`}>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="min-w-0">
            <p className="truncate font-display text-[18px] font-bold leading-[1.3] text-white">
              {paper.exam_title}
            </p>
            <p className="mt-0.5 text-[13px] text-gray-300/70">
              {ASSESSMENT_TYPE_LABEL[paper.assessment_type]} · {answered} of {total} answered
            </p>
          </div>

          <div className="flex items-center gap-3">
            <SaveIndicator state={save} onRetry={retrySave} />
            {minutes === null ? (
              <Pill>No time limit</Pill>
            ) : (
              <div
                className={`rounded-full border px-3 py-1 font-mono text-[15px] font-semibold tabular-nums ${
                  overdue
                    ? "border-danger/60 bg-danger/15 text-red-200"
                    : remaining !== null && remaining < 120_000
                      ? "border-amber-400/50 bg-amber-400/10 text-amber-200"
                      : "border-art-mid bg-art-mid/40 text-white"
                }`}
                // Announced on the minute, not the second: a per-second live
                // region would make a screen reader unusable during an exam.
                aria-live="off"
              >
                {remaining === null ? "--:--" : formatClock(remaining)}
              </div>
            )}
          </div>
        </div>

        {/* Question dots: the "Question 4 of 10" progress made navigable. */}
        <ol className="mt-4 flex flex-wrap gap-1.5">
          {paper.questions.map((q, i) => {
            const done = answers.has(q.question_seq);
            const here = i === index;
            return (
              <li key={q.question_seq}>
                <button
                  type="button"
                  onClick={() => setIndex(i)}
                  aria-current={here ? "step" : undefined}
                  aria-label={`Question ${i + 1}${done ? ", answered" : ", not answered"}`}
                  className={`h-8 w-8 rounded-[8px] text-[13px] font-semibold tabular-nums transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base ${
                    here
                      ? "bg-lime-400 text-ink-950"
                      : done
                        ? "bg-art-mid text-white"
                        : "border border-art-mid/70 text-gray-300/70 hover:text-white"
                  }`}
                >
                  {i + 1}
                </button>
              </li>
            );
          })}
        </ol>
      </div>

      {overdue ? (
        <div
          role="alert"
          className="rounded-[12px] border border-danger/40 bg-danger/10 px-4 py-3 text-[14px] leading-[1.55] text-gray-200"
        >
          <span className="font-semibold text-white">Your time is up.</span> Submit now. The exam is
          timed on the server, so this clock is a guide — your answers are already saved.
        </div>
      ) : null}

      {/* -------------------------------------------------------- question */}
      <section className={`${cardClass} sm:!p-7`}>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <p className="text-[13px] font-semibold uppercase tracking-[0.04em] text-gray-300/70">
            Question {index + 1} of {total}
          </p>
          <Pill>{question.topic}</Pill>
        </div>

        <h2 className="mt-4 text-[20px] font-semibold leading-[1.45] text-white sm:text-[22px]">
          {question.question_text}
        </h2>

        {/* A real radio group: one tab stop, arrow keys between options, and it
            works without JavaScript-specific key handling. */}
        <fieldset className="mt-6">
          <legend className="sr-only">Choose one answer</legend>
          <div className="space-y-2.5">
            {question.options.map((option, optionIndex) => {
              const selected = answers.get(question.question_seq) === optionIndex;
              return (
                <label
                  key={optionIndex}
                  className={`flex cursor-pointer items-start gap-3 rounded-[12px] border px-4 py-3 text-[16px] leading-[1.5] transition-colors focus-within:ring-2 focus-within:ring-indigo-300 focus-within:ring-offset-2 focus-within:ring-offset-art-base ${
                    selected
                      ? "border-lime-400 bg-lime-400/10 text-white"
                      : "border-art-mid/70 text-gray-200 hover:border-art-edge hover:bg-art-mid/25"
                  }`}
                >
                  <input
                    type="radio"
                    name={`q-${question.question_seq}`}
                    value={optionIndex}
                    checked={selected}
                    onChange={() => choose(question.question_seq, optionIndex)}
                    className="sr-only"
                  />
                  <span
                    aria-hidden="true"
                    className={`mt-px flex h-6 w-6 shrink-0 items-center justify-center rounded-full border text-[13px] font-semibold ${
                      selected
                        ? "border-lime-400 bg-lime-400 text-ink-950"
                        : "border-art-mid text-gray-300"
                    }`}
                  >
                    {OPTION_LETTERS[optionIndex] ?? optionIndex + 1}
                  </span>
                  <span className="min-w-0 flex-1">{option}</span>
                </label>
              );
            })}
          </div>
        </fieldset>

        <div className="mt-7 flex flex-wrap items-center justify-between gap-3">
          <button
            type="button"
            onClick={() => setIndex((i) => Math.max(0, i - 1))}
            disabled={index === 0}
            className="rounded-full border-[1.5px] border-art-mid px-5 py-2 text-[15px] font-semibold text-white transition-colors hover:border-art-edge hover:bg-art-mid/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base disabled:cursor-not-allowed disabled:opacity-40"
          >
            Back
          </button>

          {index < total - 1 ? (
            <button
              type="button"
              onClick={() => setIndex((i) => Math.min(total - 1, i + 1))}
              className="rounded-full bg-lime-400 px-6 py-2.5 text-[15px] font-semibold text-ink-950 transition-colors hover:bg-lime-300 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base"
            >
              Next
            </button>
          ) : (
            <button
              type="button"
              onClick={() => (answered < total ? setConfirming(true) : void submit())}
              disabled={submitting}
              aria-busy={submitting}
              className="rounded-full bg-lime-400 px-6 py-2.5 text-[15px] font-semibold text-ink-950 transition-colors hover:bg-lime-300 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base disabled:cursor-not-allowed disabled:opacity-60"
            >
              {submitting ? "Submitting…" : "Submit exam"}
            </button>
          )}
        </div>
      </section>

      {confirming ? (
        <div className={`${cardClass} !border-amber-400/40`} role="alertdialog" aria-label="Confirm submission">
          <p className="font-display text-[18px] font-bold text-white">
            {total - answered} question{total - answered === 1 ? "" : "s"} still unanswered
          </p>
          <p className="mt-2 text-[15px] leading-[1.6] text-gray-300">
            Unanswered questions score zero and cannot be changed once you submit. Use the numbered
            squares above to go back to them.
          </p>
          <div className="mt-5 flex flex-wrap gap-3">
            <button
              type="button"
              onClick={() => setConfirming(false)}
              className="rounded-full border-[1.5px] border-art-edge px-5 py-2 text-[15px] font-semibold text-white transition-colors hover:bg-art-mid/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base"
            >
              Keep answering
            </button>
            <button
              type="button"
              onClick={() => void submit()}
              disabled={submitting}
              aria-busy={submitting}
              className="rounded-full bg-lime-400 px-5 py-2.5 text-[15px] font-semibold text-ink-950 transition-colors hover:bg-lime-300 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base disabled:cursor-not-allowed disabled:opacity-60"
            >
              Submit anyway
            </button>
          </div>
        </div>
      ) : null}

      {submitError !== null ? <StudentError title="Not submitted" message={submitError} /> : null}
    </div>
  );
}

/**
 * Save-as-you-go feedback.
 *
 * A failure is loud and offers a repair, because the student's mental model is
 * "it saves itself" — silently losing an answer would break exactly the promise
 * the intro screen makes.
 */
function SaveIndicator({ state, onRetry }: { state: SaveState; onRetry: () => void }) {
  if (state === "idle") return null;

  if (state === "failed") {
    return (
      <button
        type="button"
        onClick={onRetry}
        className="rounded-full border border-danger/60 bg-danger/15 px-3 py-1 text-[13px] font-semibold text-red-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base"
      >
        Not saved — retry
      </button>
    );
  }

  return (
    <p aria-live="polite" className="text-[13px] text-gray-300/70">
      {state === "saving" ? "Saving…" : "Saved"}
    </p>
  );
}
