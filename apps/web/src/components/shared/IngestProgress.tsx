"use client";

/**
 * How far an uploaded PDF has got through ingestion.
 *
 * A unit is not usable the moment it is uploaded: it has to be parsed, chunked,
 * embedded and upserted into Qdrant before the tutor has anything to retrieve.
 * Until then the unit list said "0 pages · still being prepared" with no way to
 * tell a queue that is moving from one that has stalled, and no way to tell
 * either from a PDF that failed hours ago.
 *
 * # The bar is stage-based, and says so
 *
 * There is no byte-level progress to report — `ingestion_jobs` records a stage,
 * not a percentage, and inventing a smooth 0-100 from a four-step pipeline would
 * be a progress bar that lies. So each stage maps to a fixed position and the
 * stage is named in words beside it. A bar that sits at "Reading the PDF" for a
 * minute is telling the truth; one that crawls to 99% and stops is not.
 *
 * `failed` deliberately keeps a full-width bar in red rather than an empty one:
 * the work did happen, it just ended badly, and an empty bar reads as "not
 * started".
 */

export type IngestStatus =
  | "pending"
  | "parsing"
  | "pending_review"
  | "embedded"
  | "failed"
  | (string & {});

type Stage = {
  percent: number;
  label: string;
  /** Tailwind classes for the filled portion. */
  fill: string;
  text: string;
  /** Whether this stage can still change on its own. */
  settled: boolean;
};

const STAGES: Record<string, Stage> = {
  pending: {
    percent: 8,
    label: "Queued for processing",
    fill: "bg-amber-400/70",
    text: "text-amber-300",
    settled: false,
  },
  parsing: {
    percent: 45,
    label: "Reading the PDF",
    fill: "bg-sky-400/80",
    text: "text-sky-300",
    settled: false,
  },
  pending_review: {
    percent: 80,
    label: "Low text quality — awaiting review",
    fill: "bg-amber-400/80",
    text: "text-amber-300",
    settled: true,
  },
  embedded: {
    percent: 100,
    label: "Ready to teach from",
    fill: "bg-emerald-400/80",
    text: "text-emerald-300",
    settled: true,
  },
  failed: {
    percent: 100,
    label: "Processing failed",
    fill: "bg-rose-500/70",
    text: "text-rose-300",
    settled: true,
  },
};

const UNKNOWN: Stage = {
  percent: 8,
  label: "Processing",
  fill: "bg-white/30",
  text: "text-gray-400",
  settled: false,
};

export function stageFor(status: IngestStatus): Stage {
  return STAGES[status] ?? UNKNOWN;
}

/** True while the status can still change without anybody doing anything. */
export function isInFlight(status: IngestStatus): boolean {
  return !stageFor(status).settled;
}

export function IngestProgress({
  status,
  pageCount,
  className = "",
}: {
  status: IngestStatus;
  /** Shown once known; a parsed document reports its real page count. */
  pageCount?: number;
  className?: string;
}) {
  const stage = stageFor(status);
  const inFlight = !stage.settled;

  return (
    <div className={className}>
      <div className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
        <span className={`text-[13px] font-medium ${stage.text}`}>{stage.label}</span>
        {pageCount !== undefined && pageCount > 0 ? (
          <span className="text-[13px] text-gray-400">
            · {pageCount} page{pageCount === 1 ? "" : "s"}
          </span>
        ) : null}
      </div>

      <div
        role="progressbar"
        aria-valuenow={stage.percent}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label={`${stage.label}${pageCount ? `, ${pageCount} pages` : ""}`}
        className="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-white/10"
      >
        <div
          className={`h-full rounded-full transition-[width] duration-500 ${stage.fill} ${
            // Only an unsettled stage pulses. A finished bar that keeps
            // animating reads as still working.
            inFlight ? "animate-pulse" : ""
          }`}
          style={{ width: `${stage.percent}%` }}
        />
      </div>
    </div>
  );
}
