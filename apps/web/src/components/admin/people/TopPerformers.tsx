"use client";

// The best-performers board on the admin's students page.
//
// The ranking is the gateway's: `rank`, `level`, `stars` and `trophy` all
// arrive computed, and nothing here re-sorts or re-bands them. That is not
// ceremony — the student's own assessment page reads the same derivation, so a
// second ranking in the browser would be a second answer to the same question.
//
// Two properties of the board worth knowing while reading this file, both
// enforced in SQL rather than here: only GRADED attempts count, and a student
// below the minimum number of graded papers does not appear at all. A
// leaderboard topped by someone who sat one lucky paper is a sampling
// artefact, not a ranking.

import Link from "next/link";
import { useEffect, useState } from "react";

import { Banner, panelClass } from "@/components/admin/primitives";
import { listTopPerformers } from "@/lib/admin/client";
import type { TopPerformer } from "@/lib/admin/types";
import { ApiError } from "@/lib/api";

const BOARD_SIZE = 5;

/** Medal colouring for the top three; the rest render as plain numbers. */
const RANK_TONE: Record<number, string> = {
  1: "bg-amber-400/15 text-amber-300 ring-amber-400/30",
  2: "bg-slate-300/15 text-slate-200 ring-slate-300/30",
  3: "bg-orange-400/15 text-orange-300 ring-orange-400/30",
};

export function TopPerformers({ limit = BOARD_SIZE }: { limit?: number }) {
  const [rows, setRows] = useState<TopPerformer[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    listTopPerformers({ limit })
      .then((data) => {
        if (!cancelled) setRows(data);
      })
      .catch((caught: unknown) => {
        if (cancelled) return;
        setError(
          caught instanceof ApiError
            ? caught.message
            : "The best performers could not be loaded.",
        );
      });
    return () => {
      cancelled = true;
    };
  }, [limit]);

  if (error !== null) {
    return (
      <section className={`${panelClass} p-4`}>
        <Header />
        <Banner tone="danger">{error}</Banner>
      </section>
    );
  }

  if (rows === null) {
    return (
      <section className={`${panelClass} p-4`}>
        <Header />
        <div className="mt-3 h-24 animate-pulse rounded-[12px] bg-art-mid/40" aria-hidden="true" />
      </section>
    );
  }

  return (
    <section className={`${panelClass} p-4`}>
      <Header />
      {rows.length === 0 ? (
        // Deliberately explicit about WHY it is empty. "No data" would read as
        // a fault, when the usual cause is simply that nobody has sat enough
        // graded papers yet.
        <p className="mt-3 text-[14px] leading-[1.55] text-gray-300/70">
          No student has enough graded exams yet to be ranked.
        </p>
      ) : (
        <ol className="mt-3 space-y-2">
          {rows.map((row) => (
            <li key={row.student_id}>
              <Link
                href={`/admin/students/${row.student_id}`}
                className="flex items-center gap-3 rounded-[10px] border border-art-mid/60 px-3 py-2 transition-colors hover:bg-art-mid/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300"
              >
                <span
                  className={`inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-[13px] font-bold ring-1 ${
                    RANK_TONE[row.rank] ?? "bg-art-mid/50 text-gray-300 ring-art-edge"
                  }`}
                >
                  {row.rank}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-[14px] font-semibold text-white">
                    {row.student_name}
                  </span>
                  <span className="block truncate text-[12px] text-gray-400">
                    {row.roll_number} · {row.program_name} · Sem {row.semester_number}
                  </span>
                </span>
                <span className="shrink-0 text-right">
                  <span className="block text-[14px] font-semibold text-white">
                    {row.average_percentage.toFixed(1)}%
                  </span>
                  <span className="block text-[12px] text-gray-400">
                    {row.attempts_graded} graded
                  </span>
                </span>
              </Link>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

function Header() {
  return (
    <div>
      <h2 className="text-[15px] font-semibold text-white">Best performers</h2>
      <p className="mt-0.5 text-[12px] text-gray-400">
        By average across graded exams. Unmarked papers are not counted.
      </p>
    </div>
  );
}
