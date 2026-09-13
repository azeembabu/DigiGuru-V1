// Shared dark-surface pieces for the student portal, dashboard, and exam
// module.
//
// These deliberately do not reuse `components/admin/primitives.tsx`: that set
// is the light, data-dense console (DESIGN.md §7), and these sit on the
// green-black `art-*` ground the student already sees on the dashboard. Same
// roles, different surface — see the header comment in the admin file.

import type { ReactNode } from "react";

import { cardClass } from "@/components/dashboard/Card";
import { ASSESSMENT_TYPE_LABEL, type AssessmentType } from "@/lib/student";

export { cardClass };

/** Section heading used by every dashboard panel, so they read as one set. */
export function SectionHeader({
  title,
  hint,
  action,
}: {
  title: string;
  hint?: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-start justify-between gap-3">
      <div className="min-w-0">
        <h2 className="font-display text-[20px] font-bold leading-[1.3] text-white">{title}</h2>
        {hint ? <p className="mt-1 text-[14px] leading-[1.5] text-gray-300/70">{hint}</p> : null}
      </div>
      {action}
    </div>
  );
}

/**
 * The empty state every panel must have.
 *
 * It says what is missing *and* how a row gets there, because a student looking
 * at an empty flashcard notebook has no other way to learn that the tutor fills
 * it during a session. It never renders a zero dressed as data.
 */
export function StudentEmpty({ title, hint }: { title: string; hint?: string }) {
  return (
    <div className="rounded-[12px] border border-dashed border-art-mid/70 px-4 py-8 text-center">
      <p className="text-[15px] font-semibold text-white">{title}</p>
      {hint ? (
        <p className="mx-auto mt-1 max-w-md text-[14px] leading-[1.55] text-gray-300/70">{hint}</p>
      ) : null}
    </div>
  );
}

/**
 * A failed panel.
 *
 * Separate from the empty state on purpose: "you have no exams" and "we could
 * not ask the server about your exams" are different facts, and showing the
 * first when the second is true is a lie the student would act on.
 */
export function StudentError({
  title = "We couldn’t load this",
  message,
  onRetry,
}: {
  title?: string;
  message: string;
  onRetry?: () => void;
}) {
  return (
    <div
      role="alert"
      className="rounded-[12px] border border-danger/40 bg-danger/10 px-4 py-4 text-[14px] leading-[1.55]"
    >
      <p className="font-semibold text-white">{title}</p>
      <p className="mt-1 text-gray-300">{message}</p>
      {onRetry ? (
        <button
          type="button"
          onClick={onRetry}
          className="mt-3 rounded-full border-[1.5px] border-art-edge px-4 py-1.5 text-[14px] font-semibold text-white transition-colors hover:bg-art-mid/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-art-base"
        >
          Try again
        </button>
      ) : null}
    </div>
  );
}

export type PillTone = "neutral" | "good" | "warn" | "bad" | "accent";

const pillTones: Record<PillTone, string> = {
  neutral: "border-art-mid bg-art-mid/40 text-gray-300",
  good: "border-lime-400/40 bg-lime-400/10 text-lime-300",
  warn: "border-amber-400/40 bg-amber-400/10 text-amber-200",
  bad: "border-danger/40 bg-danger/15 text-red-200",
  accent: "border-indigo-300/40 bg-indigo-300/10 text-indigo-200",
};

export function Pill({ tone = "neutral", children }: { tone?: PillTone; children: ReactNode }) {
  return (
    <span
      className={`inline-flex items-center rounded-full border px-2.5 py-0.5 text-[12px] font-semibold leading-[1.5] ${pillTones[tone]}`}
    >
      {children}
    </span>
  );
}

/**
 * A weak area from a graded attempt.
 *
 * Warn, not danger: a weak topic is a thing to revise, not a failure, and the
 * badge carries the word "Revise" so the meaning does not rest on colour alone
 * (DESIGN.md §8).
 */
export function WeakTopicBadge({ topic }: { topic: string }) {
  return <Pill tone="warn">Revise: {topic}</Pill>;
}

export function AssessmentTypePill({ type }: { type: AssessmentType }) {
  return <Pill tone="accent">{ASSESSMENT_TYPE_LABEL[type]}</Pill>;
}

/** A skeleton block matching a panel's height, so arrival does not jump. */
export function PanelSkeleton({ className = "h-40" }: { className?: string }) {
  return <div className={`animate-pulse rounded-[16px] bg-art-mid/50 ${className}`} aria-hidden="true" />;
}

// ------------------------------------------------------------------ formats

/** `83%` — percentages are already 0..100 on the wire. */
export function formatPercent(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return "—";
  return `${Math.round(value)}%`;
}

/** `18 / 20`. Scores are numbers on the wire; the slash is this layer's job. */
export function formatScore(score: number | null, maxScore: number): string {
  if (score === null || !Number.isFinite(score)) return `— / ${trimNumber(maxScore)}`;
  return `${trimNumber(score)} / ${trimNumber(maxScore)}`;
}

/** Drop a pointless `.0` without rounding a real fraction away. */
export function trimNumber(value: number): string {
  if (!Number.isFinite(value)) return "—";
  return Number.isInteger(value) ? String(value) : value.toFixed(2).replace(/0$/, "");
}

export function formatDate(iso: string | null): string {
  if (!iso) return "—";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";
  return date.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

export function formatDateTime(iso: string | null): string {
  if (!iso) return "—";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";
  return date.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** `mm:ss` for the exam countdown. Clamped at zero — never a negative clock. */
export function formatClock(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}
