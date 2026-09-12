// Id -> name resolution for the people screens.
//
// Student rows carry bare `program_id` / `lsc_id` UUIDs, and the gateway has
// no joined variant of either (semester and account status it does resolve
// server-side, so those are read off the row directly). Resolving per row
// would be one request per row, so both catalogues are fetched once per screen
// and held as maps. They are small and unpaginated on the gateway side
// (`admin/programs.rs`, `admin/lscs.rs` take no query params at all), which is
// what makes a single fetch honest rather than a hidden first page.

"use client";

import { useEffect, useState } from "react";

import { listLscs, listPrograms } from "@/lib/admin/client";
import type { Lsc, Program } from "@/lib/admin/types";

export type Catalogue = {
  programs: Program[];
  programNames: Map<string, string>;
  lscNames: Map<string, string>;
  /** True until both lists have resolved; a failure also ends the wait. */
  loading: boolean;
};

/** Shown wherever an id could not be resolved, so a UUID never reaches a cell. */
export const UNKNOWN_LABEL = "—";

function byId<T extends { id: string; name: string }>(rows: T[]): Map<string, string> {
  return new Map(rows.map((row) => [row.id, row.name]));
}

/**
 * Fetch the program and LSC catalogues once.
 *
 * A failure here is deliberately not surfaced as a page-level error: the list
 * itself still renders, names just fall back to `UNKNOWN_LABEL`. Losing a
 * label is not worth hiding the student rows behind an error state.
 */
export function useCatalogue(): Catalogue {
  const [programs, setPrograms] = useState<Program[]>([]);
  const [lscs, setLscs] = useState<Lsc[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    Promise.allSettled([listPrograms({ limit: 200 }), listLscs({ limit: 200 })])
      .then(([programResult, lscResult]) => {
        if (cancelled) return;
        if (programResult.status === "fulfilled") setPrograms(programResult.value.items);
        if (lscResult.status === "fulfilled") setLscs(lscResult.value.items);
        setLoading(false);
      })
      .catch(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return {
    programs,
    programNames: byId(programs),
    lscNames: byId(lscs),
    loading,
  };
}

/** Resolve an id against a name map without ever falling back to the raw id. */
export function nameOf(map: Map<string, string>, id: string | null | undefined): string {
  if (!id) return UNKNOWN_LABEL;
  return map.get(id) ?? UNKNOWN_LABEL;
}

/** `2026-09-13` — dates only, no locale surprises in a data table. */
export function formatDate(iso: string | null): string {
  if (!iso) return UNKNOWN_LABEL;
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? UNKNOWN_LABEL : date.toISOString().slice(0, 10);
}
