"use client";

import { useEffect, useState } from "react";

import { getQuestionPoolCounts } from "@/lib/admin/client";
import type { QuestionPoolCount } from "@/lib/admin/types";

// Per-course pool coverage (A2.5: "show the per-course question count so an
// admin can see which courses are still empty").
//
// One request for the whole program — `GET /admin/programs/{id}/question-pool-counts`
// returns every course including the empty ones, so there is no per-course fan-out
// and no course that is silently missing because it had no rows to count.
//
// A count is never derived from a page of question rows: that would cap at the
// page size and quietly under-report a full pool.

export type CoverageState = {
  rows: QuestionPoolCount[];
  loading: boolean;
  /** True when the request failed. The UI must then say the count is unknown —
   *  never that the pool is empty, which would send an admin to fill a pool
   *  that may already be full. */
  failed: boolean;
};

export function useQuestionPoolCounts(programId: string): CoverageState {
  const [state, setState] = useState<{
    programId: string;
    rows: QuestionPoolCount[] | null;
  } | null>(null);

  useEffect(() => {
    if (!programId) return;

    let cancelled = false;

    getQuestionPoolCounts(programId)
      .then((rows) => {
        if (!cancelled) setState({ programId, rows });
      })
      .catch(() => {
        // `null` rows is the failure marker, distinct from an empty array (a
        // program with no courses at all, which is a real and different answer).
        if (!cancelled) setState({ programId, rows: null });
      });

    return () => {
      cancelled = true;
    };
  }, [programId]);

  // Tagged with the program it answers, so a stale program's counts are never
  // shown against a newly selected one and "loading" is a comparison during
  // render rather than a synchronous setState inside the effect.
  const current = state !== null && state.programId === programId;

  return {
    rows: current && state.rows !== null ? state.rows : [],
    loading: !current && programId.length > 0,
    failed: current && state.rows === null,
  };
}
