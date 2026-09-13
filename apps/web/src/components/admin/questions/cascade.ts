"use client";

import { useEffect, useState } from "react";

import { listBlocks, listCourses, listPrograms, listSemesters } from "@/lib/admin/client";
import type { Block, Course, Program, Semester } from "@/lib/admin/types";

// Program > Semester > Course > Block, resolved one level at a time.
//
// A question hangs off a **block**, and a block is reached only through its
// course (`.claude/rules/api-conventions.md`: `block -> course -> program_id`).
// There is no flat block list endpoint and deliberately no flat
// `GET /admin/semesters`, so the course step is a real step rather than a UI
// nicety — it is the only path the API offers from a semester to a block.

export type Cascade = {
  programs: Program[];
  semesters: Semester[];
  courses: Course[];
  blocks: Block[];
};

export type CascadeIds = {
  programId: string;
  semesterId: string;
  courseId: string;
};

/** Module-level so the reference is stable across renders and the effect below
 *  re-runs on the parent id alone. */
const ALL_PROGRAMS = "all";
const loadPrograms = (): Promise<Program[]> =>
  listPrograms({ limit: 200 }).then((page) => page.items);

/**
 * One level of the hierarchy.
 *
 * The rows are stored **tagged with the parent they belong to** and returned
 * only while that tag still matches. That is what clears a stale level when the
 * parent changes: there is no `setRows([])` in the effect body to do it, so
 * nothing cascades a render, and a semester from the previously selected
 * program can never be offered for the current one.
 *
 * A failed lookup resolves to an empty list rather than an error: the screen
 * around it has its own banner for the thing the admin actually asked for, and
 * an empty select is a truer report of a failed lookup than a page-wide
 * failure.
 */
function useLevel<T>(parentId: string, load: (parentId: string) => Promise<T[]>): T[] {
  const [state, setState] = useState<{ key: string; rows: T[] } | null>(null);

  useEffect(() => {
    if (!parentId) return;

    let cancelled = false;

    load(parentId)
      .then((rows) => {
        if (!cancelled) setState({ key: parentId, rows });
      })
      .catch(() => {
        if (!cancelled) setState({ key: parentId, rows: [] });
      });

    return () => {
      cancelled = true;
    };
  }, [parentId, load]);

  return state !== null && state.key === parentId ? state.rows : [];
}

export function useCascade({ programId, semesterId, courseId }: CascadeIds): Cascade {
  return {
    programs: useLevel(ALL_PROGRAMS, loadPrograms),
    semesters: useLevel(programId, listSemesters),
    courses: useLevel(semesterId, listCourses),
    blocks: useLevel(courseId, listBlocks),
  };
}
