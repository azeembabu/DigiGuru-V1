"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { Button, buttonClass } from "@/components/ui/Button";
import { IconBoard, IconBook, IconPlay, IconTarget } from "@/components/icons";
import {
  AssessmentTypePill,
  Pill,
  SectionHeader,
  StudentEmpty,
  StudentError,
  WeakTopicBadge,
  cardClass,
  formatDate,
  formatDateTime,
  formatPercent,
  formatScore,
} from "@/components/student/parts";
import {
  loadFlashcards,
  loadRevisions,
  reviewFlashcard,
  type ContinueLearning,
  type ExamHistoryEntry,
  type Flashcard,
  type Revision,
  type SavedResources,
} from "@/lib/student";
import { ApiError } from "@/lib/api";

// The five dashboard panels that are not the quota ring. Each one owns its own
// empty state, because "nothing yet" is the normal first-login condition for
// every one of them and a blank panel would read as a broken page.

function messageFor(caught: unknown, fallback: string): string {
  if (caught instanceof ApiError) return caught.message;
  if (caught instanceof Error) return caught.message;
  return fallback;
}

// ---------------------------------------------------------------- 1. resume

/**
 * Continue Learning hero.
 *
 * `continue_learning` is `null` for a student who has never had a session — a
 * first-login student must render, not 500 (contract). So the null branch is
 * the *welcome*, not an error, and it still offers the classroom because a
 * first session is exactly what it should offer.
 */
export function ContinueLearningHero({ resume }: { resume: ContinueLearning | null }) {
  if (resume === null) {
    return (
      <section className={`${cardClass} sm:!p-8`}>
        <p className="text-[12px] font-semibold uppercase tracking-[0.04em] text-gray-300/70">
          Study module
        </p>
        <h2 className="mt-3 text-balance font-display text-[28px] font-bold leading-[1.2] text-white">
          Your first <span className="text-lime-400">session</span>
        </h2>
        <p className="mt-3 max-w-xl text-[16px] leading-[1.6] text-gray-300">
          There is nothing to resume yet. The tutor opens on your current block and introduces
          itself once; from then on this card returns you to the page you stopped on.
        </p>
        <Button href="/classroom" variant="primary" className="mt-7">
          <IconPlay className="h-5 w-5" />
          Begin your first session
        </Button>
      </section>
    );
  }

  const progress = Math.min(100, Math.max(0, resume.progress_percentage));

  return (
    <section className={`${cardClass} sm:!p-8`}>
      <p className="text-[12px] font-semibold uppercase tracking-[0.04em] text-gray-300/70">
        Continue learning
      </p>
      <h2 className="mt-3 text-balance font-display text-[28px] font-bold leading-[1.2] text-white">
        Continue where you <span className="text-lime-400">left off</span>
      </h2>

      <div className="mt-5 grid gap-5 sm:grid-cols-[1fr_auto] sm:items-end">
        <div className="min-w-0">
          <p className="font-display text-[20px] font-bold leading-[1.3] text-white">
            {resume.block_title}
          </p>
          <p className="mt-1 text-[14px] leading-[1.5] text-gray-300/70">
            {resume.course_title} · Block {resume.block_no}
            {resume.chapter_name ? ` · ${resume.chapter_name}` : ""}
            {resume.page_number > 0 ? ` · page ${resume.page_number}` : ""}
          </p>
          {resume.resume_summary ? (
            <p className="mt-3 max-w-xl text-[15px] leading-[1.6] text-gray-300">
              “{resume.resume_summary}”
            </p>
          ) : null}

          {/* Progress is blocks-completed / blocks-in-course, computed by the
              gateway. Shown with the number beside the bar: a bar alone cannot
              be read by a screen reader or at a glance. */}
          <div className="mt-5 max-w-sm">
            <div className="flex items-center justify-between text-[13px] text-gray-300/70">
              <span>Course progress</span>
              <span className="tabular-nums">{formatPercent(progress)}</span>
            </div>
            <div
              role="progressbar"
              aria-valuenow={Math.round(progress)}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-label="Course progress"
              className="mt-1.5 h-2 overflow-hidden rounded-full bg-art-mid/70"
            >
              <div className="h-full rounded-full bg-art-glow" style={{ width: `${progress}%` }} />
            </div>
          </div>

          <p className="mt-4 text-[13px] leading-[1.5] text-gray-300/60">
            Last active {formatDateTime(resume.last_active_at)}
          </p>
        </div>

        <Button href="/classroom" variant="primary" className="shrink-0">
          <IconPlay className="h-5 w-5" />
          Continue session
        </Button>
      </div>
    </section>
  );
}

// ------------------------------------------------------------------ 2. exams

function examTone(percentage: number | null): "good" | "warn" | "bad" | "neutral" {
  if (percentage === null) return "neutral";
  if (percentage >= 70) return "good";
  if (percentage >= 40) return "warn";
  return "bad";
}

/** The last three attempts, with the weak areas the grading produced. */
export function ExamHistoryPanel({ history }: { history: ExamHistoryEntry[] }) {
  return (
    <section className={cardClass}>
      <SectionHeader
        title="Recent assessments"
        hint="Your last three attempts"
        action={
          <Link
            href="/exams"
            className="text-[14px] font-semibold text-lime-400 underline-offset-4 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base"
          >
            All assessments
          </Link>
        }
      />

      <div className="mt-5">
        {history.length === 0 ? (
          <StudentEmpty
            title="No attempts yet"
            hint="Open the Exam module to sit an assignment, mid-term quiz or semester exam. Your score and the topics to revise appear here on submission."
          />
        ) : (
          <ul className="space-y-3">
            {history.map((entry) => (
              <li
                key={entry.attempt_id}
                className="rounded-[12px] border border-art-mid/60 bg-art-base/40 p-4"
              >
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="min-w-0">
                    <p className="truncate font-display text-[16px] font-bold leading-[1.35] text-white">
                      {entry.exam_title}
                    </p>
                    <p className="mt-1 text-[13px] text-gray-300/70">
                      {entry.submitted_at === null
                        ? "In progress"
                        : `Submitted ${formatDateTime(entry.submitted_at)}`}
                    </p>
                  </div>
                  <div className="shrink-0 text-right">
                    <p className="font-display text-[20px] font-bold leading-none tabular-nums text-white">
                      {formatScore(entry.score, entry.max_score)}
                    </p>
                    <p className="mt-1 text-[13px] tabular-nums text-gray-300/70">
                      {formatPercent(entry.percentage)}
                    </p>
                  </div>
                </div>

                <div className="mt-3 flex flex-wrap items-center gap-2">
                  {entry.assessment_type ? <AssessmentTypePill type={entry.assessment_type} /> : null}
                  <Pill tone={examTone(entry.percentage)}>{entry.status.replace(/_/g, " ")}</Pill>
                  {entry.weak_topics.map((topic) => (
                    <WeakTopicBadge key={topic} topic={topic} />
                  ))}
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

// ------------------------------------------------------------------ 3. notes

/**
 * Saved whiteboard notes.
 *
 * Count only, deliberately. The board op-log is exported client-side from the
 * classroom (`.claude/rules/pedagogy.md` — "export is a client-side render of
 * the JSON ops, not a new server artifact"), and the dashboard contract gives
 * `preserved_notes_count` with no list endpoint behind it. Inventing a row list
 * here would mean inventing the rows.
 */
export function NotesLibraryPanel({ resources }: { resources: SavedResources }) {
  const count = resources.preserved_notes_count;

  return (
    <section className={cardClass}>
      <SectionHeader title="Saved notes" hint="Whiteboards kept from your sessions" />
      <div className="mt-5">
        {count === 0 ? (
          <StudentEmpty
            title="No saved notes yet"
            hint="During a session, use Copy or Download on the whiteboard to keep a board. Any board you tag for revision is counted here."
          />
        ) : (
          <div className="flex items-center gap-4 rounded-[12px] border border-art-mid/60 bg-art-base/40 p-4">
            <span className="flex h-12 w-12 shrink-0 items-center justify-center rounded-full bg-art-mid/60 text-art-glow">
              <IconBoard className="h-6 w-6" />
            </span>
            <div className="min-w-0">
              <p className="font-display text-[24px] font-bold leading-none tabular-nums text-white">
                {count}
              </p>
              <p className="mt-1 text-[14px] leading-[1.5] text-gray-300/70">
                whiteboard{count === 1 ? "" : "s"} saved across your sessions
              </p>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

// ------------------------------------------------------------- 4. flashcards

/**
 * The flashcard notebook.
 *
 * The counts come from the one dashboard call; the cards themselves are a
 * second, secondary request to `GET /student/flashcards?due_only=true`. If that
 * request fails the panel still shows the true counts and says it could not
 * load the cards — it does not hide the counts behind the failure.
 */
export function FlashcardPanel({ resources }: { resources: SavedResources }) {
  const [revealed, setRevealed] = useState<string | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  const [nonce, setNonce] = useState(0);
  // Tagged with the request it answers, so the loading state is derived during
  // render instead of being reset synchronously inside the effect.
  const [loaded, setLoaded] = useState<{
    nonce: number;
    cards: Flashcard[] | null;
    error: string | null;
  } | null>(null);
  // Reviewed cards, held separately from the fetched list: the server has moved
  // their `due_on` forward, so they are no longer due and re-fetching would
  // return the same list minus these rows anyway.
  const [reviewed, setReviewed] = useState<ReadonlySet<string>>(() => new Set());
  const [reviewError, setReviewError] = useState<string | null>(null);

  const hasAny = resources.flashcards_total > 0;

  useEffect(() => {
    if (!hasAny) return;

    let cancelled = false;

    loadFlashcards({ due_only: true, limit: 20 })
      .then((page) => {
        if (!cancelled) setLoaded({ nonce, cards: page.items, error: null });
      })
      .catch((caught: unknown) => {
        if (!cancelled) {
          setLoaded({ nonce, cards: null, error: messageFor(caught, "Your flashcards could not be loaded.") });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [hasAny, nonce]);

  const review = useCallback(async (card: Flashcard, recalled: boolean) => {
    setPending(card.id);
    try {
      await reviewFlashcard(card.id, recalled);
      setReviewed((current) => new Set(current).add(card.id));
      setRevealed(null);
      setReviewError(null);
    } catch (caught: unknown) {
      setReviewError(messageFor(caught, "That review could not be recorded."));
    } finally {
      setPending(null);
    }
  }, []);

  const current = loaded !== null && loaded.nonce === nonce;
  const error = (current ? loaded.error : null) ?? reviewError;
  const cards =
    current && loaded.cards !== null ? loaded.cards.filter((row) => !reviewed.has(row.id)) : null;

  const due = resources.flashcards_due_for_review;

  return (
    <section className={cardClass}>
      <SectionHeader
        title="Flashcards"
        hint={
          hasAny
            ? `${resources.flashcards_total} card${resources.flashcards_total === 1 ? "" : "s"} · ${due} due today`
            : undefined
        }
      />

      <div className="mt-5 space-y-3">
        {!hasAny ? (
          <StudentEmpty
            title="No flashcards yet"
            hint="The tutor adds a card whenever you pin something during a session. Cards return on a spaced-repetition schedule, so the ones you recall least often appear most often."
          />
        ) : error !== null ? (
          <StudentError
            message={error}
            onRetry={() => {
              setReviewError(null);
              setNonce((n) => n + 1);
            }}
          />
        ) : cards === null ? (
          <div className="h-24 animate-pulse rounded-[12px] bg-art-mid/50" aria-hidden="true" />
        ) : cards.length === 0 ? (
          <StudentEmpty
            title="Nothing due today"
            hint={`All ${resources.flashcards_total} of your cards are scheduled for a later date.`}
          />
        ) : (
          cards.map((card) => {
            const open = revealed === card.id;
            return (
              <div
                key={card.id}
                className="rounded-[12px] border border-art-mid/60 bg-art-base/40 p-4"
              >
                <div className="flex flex-wrap items-start justify-between gap-2">
                  <p className="min-w-0 flex-1 text-[15px] font-semibold leading-[1.5] text-white">
                    {card.front}
                  </p>
                  {card.topic ? <Pill>{card.topic}</Pill> : null}
                </div>

                {open ? (
                  <>
                    <p className="mt-3 border-t border-art-mid/60 pt-3 text-[15px] leading-[1.6] text-gray-300">
                      {card.back}
                    </p>
                    <div className="mt-4 flex flex-wrap gap-2">
                      <button
                        type="button"
                        disabled={pending === card.id}
                        onClick={() => void review(card, true)}
                        className={buttonClass(
                          "primary",
                          "!px-4 !py-1.5 !text-[14px] disabled:opacity-60",
                        )}
                      >
                        I recalled this
                      </button>
                      <button
                        type="button"
                        disabled={pending === card.id}
                        onClick={() => void review(card, false)}
                        className={buttonClass(
                          "outline",
                          "!px-4 !py-1 !text-[14px] disabled:opacity-60",
                        )}
                      >
                        Review again
                      </button>
                    </div>
                  </>
                ) : (
                  <button
                    type="button"
                    onClick={() => setRevealed(card.id)}
                    className="mt-3 text-[14px] font-semibold text-lime-400 underline-offset-4 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base"
                  >
                    Show answer
                  </button>
                )}
              </div>
            );
          })
        )}
      </div>
    </section>
  );
}

// -------------------------------------------------------------- 5. revisions

/** Up-next revision schedule, from the `note_reminders` ledger. */
export function RevisionPanel({ resources }: { resources: SavedResources }) {
  const [nonce, setNonce] = useState(0);
  // Tagged with the request it answers — see the note in `FlashcardPanel`.
  const [loaded, setLoaded] = useState<{
    nonce: number;
    rows: Revision[] | null;
    error: string | null;
  } | null>(null);

  const hasAny = resources.revisions_due_count > 0;

  useEffect(() => {
    if (!hasAny) return;

    let cancelled = false;

    loadRevisions({ limit: 10 })
      .then((page) => {
        if (!cancelled) setLoaded({ nonce, rows: page.items, error: null });
      })
      .catch((caught: unknown) => {
        if (!cancelled) {
          setLoaded({
            nonce,
            rows: null,
            error: messageFor(caught, "Your revision schedule could not be loaded."),
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [hasAny, nonce]);

  const current = loaded !== null && loaded.nonce === nonce;
  const error = current ? loaded.error : null;
  const rows = current ? loaded.rows : null;

  return (
    <section className={cardClass}>
      <SectionHeader
        title="Scheduled revision"
        hint={hasAny ? `${resources.revisions_due_count} due` : undefined}
      />

      <div className="mt-5">
        {!hasAny ? (
          <StudentEmpty
            title="Nothing scheduled"
            hint="When you export a board you may tag it with a revision date. The reminder appears here on the date it falls due."
          />
        ) : error !== null ? (
          <StudentError message={error} onRetry={() => setNonce((n) => n + 1)} />
        ) : rows === null ? (
          <div className="h-24 animate-pulse rounded-[12px] bg-art-mid/50" aria-hidden="true" />
        ) : rows.length === 0 ? (
          <StudentEmpty
            title="Nothing due right now"
            hint="Your tagged notes are scheduled for later dates."
          />
        ) : (
          <ul className="divide-y divide-art-mid/60">
            {rows.map((row) => (
              <li key={row.id} className="flex items-start gap-3 py-3 first:pt-0 last:pb-0">
                <span className="mt-0.5 shrink-0 text-art-glow">
                  {row.block_no === null ? (
                    <IconTarget className="h-5 w-5" />
                  ) : (
                    <IconBook className="h-5 w-5" />
                  )}
                </span>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-[15px] font-semibold text-white">
                    {row.topic ?? row.block_title ?? "Saved board"}
                  </p>
                  <p className="mt-0.5 text-[13px] text-gray-300/70">
                    {row.block_no === null ? "Revision" : `Block ${row.block_no}`} · due{" "}
                    {formatDate(row.remind_at)}
                  </p>
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
