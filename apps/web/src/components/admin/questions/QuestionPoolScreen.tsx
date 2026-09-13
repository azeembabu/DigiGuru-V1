"use client";

import Link from "next/link";
import { useCallback } from "react";

import { DataTable, type Column } from "@/components/admin/DataTable";
import { Pagination, SearchInput } from "@/components/admin/controls";
import { Banner, PageHeader, StatusPill, panelClass } from "@/components/admin/primitives";
import { useResource } from "@/components/admin/academic/shared";
import {
  ActiveFilters,
  FilterBar,
  FilterSelect,
  PAGE_SIZE,
  RowLink,
  formatDateTime,
} from "@/components/admin/drilldown/shared";
import { useUrlFilters } from "@/components/admin/drilldown/shared";
import { useCascade } from "@/components/admin/questions/cascade";
import { useQuestionPoolCounts } from "@/components/admin/questions/counts";
import { listPoolQuestions } from "@/lib/admin/client";
import {
  ASSESSMENT_TYPES,
  DIFFICULTY_LEVELS,
  type AssessmentType,
  type DifficultyLevel,
  type PoolQuestion,
  type QuestionStatus,
} from "@/lib/admin/types";

// The question pool an exam paper is sampled from.
//
// AMENDMENT A2.2 narrowed sampling back to the COURSE: a paper is drawn strictly
// from the pool of the exam's own course, filtered by `assessment_type`. So the
// navigation here is Program -> Semester -> Course, and the unit/module (block)
// is an optional last filter rather than the thing a question belongs to.
//
// Filters live in the URL like the other drill-downs, so a coordinator can paste
// "every advanced semester-exam question in MAL101" into a chat.

const STATUSES: { value: QuestionStatus; label: string }[] = [
  { value: "active", label: "Active" },
  { value: "retired", label: "Retired" },
];

export function QuestionPoolScreen() {
  const { get, offset, set, setOffset, clearAll } = useUrlFilters();
  const programId = get("program_id");
  const semesterId = get("semester_id");
  const courseId = get("course_id");
  const blockId = get("block_id");
  const assessmentType = get("assessment_type");
  const difficulty = get("difficulty_level");
  const status = get("status");
  const q = get("q");

  const cascade = useCascade({ programId, semesterId, courseId });

  const loader = useCallback(
    () =>
      listPoolQuestions({
        program_id: programId || undefined,
        semester_id: semesterId || undefined,
        course_id: courseId || undefined,
        block_id: blockId || undefined,
        assessment_type: (assessmentType || undefined) as AssessmentType | undefined,
        difficulty_level: (difficulty || undefined) as DifficultyLevel | undefined,
        status: (status || undefined) as QuestionStatus | undefined,
        q: q || undefined,
        limit: PAGE_SIZE,
        offset,
      }),
    [programId, semesterId, courseId, blockId, assessmentType, difficulty, status, q, offset],
  );
  const { data, loading, error } = useResource(loader, "Could not load the question pool.");

  const active: string[] = [];
  if (programId) active.push("one program");
  if (semesterId) active.push("one semester");
  if (courseId) active.push("one course");
  if (blockId) active.push("one unit");
  if (assessmentType) {
    active.push(ASSESSMENT_TYPES.find((t) => t.value === assessmentType)?.label ?? assessmentType);
  }
  if (difficulty) {
    active.push(DIFFICULTY_LEVELS.find((d) => d.value === difficulty)?.label ?? difficulty);
  }
  if (status) active.push(status);
  if (q) active.push(`“${q}”`);

  const columns: Column<PoolQuestion>[] = [
    {
      key: "question",
      header: "Question",
      cell: (row) => (
        <div className="min-w-[18rem] max-w-[32rem]">
          <p className="font-medium text-gray-900">{row.question_text}</p>
          <p className="mt-0.5 text-xs text-gray-500">
            Correct: {letterFor(row.correct_option_index)} ·{" "}
            {row.options[row.correct_option_index] ?? "—"}
          </p>
        </div>
      ),
    },
    {
      key: "topic",
      header: "Topic",
      className: "w-[11rem]",
      cell: (row) => row.topic,
    },
    {
      key: "placement",
      header: "Course / unit",
      cell: (row) => (
        <div className="min-w-[11rem]">
          <RowLink href={`/admin/courses/${row.course_id}`}>
            {row.course_code}
            {row.course_name ? ` — ${row.course_name}` : ""}
          </RowLink>
          {/* A2.1: the unit/module is optional. A question with none is pooled
              for the whole course, which is the normal case — so the absence is
              stated plainly rather than shown as a missing link. */}
          {row.block_id === null ? (
            <p className="mt-0.5 text-xs text-gray-500">Whole course</p>
          ) : (
            <p className="mt-0.5 text-xs text-gray-500">
              Unit {row.block_no}
              {row.block_title ? ` — ${row.block_title}` : ""}
            </p>
          )}
        </div>
      ),
    },
    {
      key: "type",
      header: "Assessment",
      className: "w-[9rem]",
      cell: (row) => (
        <span className="whitespace-nowrap">
          {ASSESSMENT_TYPES.find((t) => t.value === row.assessment_type)?.label ??
            row.assessment_type}
        </span>
      ),
    },
    {
      key: "difficulty",
      header: "Difficulty",
      className: "w-[7.5rem]",
      cell: (row) => (
        <StatusPill tone={row.difficulty_level === "advanced" ? "danger" : "neutral"}>
          {row.difficulty_level}
        </StatusPill>
      ),
    },
    {
      key: "status",
      header: "Status",
      className: "w-[6.5rem]",
      cell: (row) => (
        <StatusPill tone={row.status === "active" ? "success" : "neutral"}>{row.status}</StatusPill>
      ),
    },
    {
      key: "created",
      header: "Added",
      align: "right",
      cell: (row) => (
        <span className="whitespace-nowrap text-gray-500">{formatDateTime(row.created_at)}</span>
      ),
    },
  ];

  return (
    <div className="space-y-6">
      <PageHeader
        title="Question pool"
        description="Every multiple-choice question an exam paper can be drawn from. A pool belongs to a course: a paper is sampled strictly from its own course's pool, filtered by assessment type. Linking a question to a unit is optional."
        action={
          <div className="flex flex-wrap gap-2">
            <Link
              href="/admin/question-pool/new"
              className="inline-flex items-center rounded-full bg-lime-500 px-5 py-2 text-sm font-semibold text-ink-950 transition-colors hover:bg-lime-400 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-lavender-50"
            >
              Add a question
            </Link>
            <Link
              href="/admin/question-pool/import"
              className="inline-flex items-center rounded-full border-[1.5px] border-lavender-200 px-5 py-1.5 text-sm font-semibold text-gray-900 transition-colors hover:bg-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 focus-visible:ring-offset-2 focus-visible:ring-offset-lavender-50"
            >
              Bulk import
            </Link>
          </div>
        }
      />

      <FilterBar>
        <SearchInput
          key={q}
          value={q}
          label="Search questions"
          placeholder="Question text or topic…"
          onChange={(next) => set({ q: next })}
        />
        <FilterSelect
          id="qp-program"
          label="Program"
          value={programId}
          anyLabel="Any program"
          options={cascade.programs.map((p) => ({ value: p.id, label: `${p.code} — ${p.name}` }))}
          // Each level clears the ones below it: a semester from another
          // program is not a filter, it is a contradiction.
          onChange={(next) => set({ program_id: next, semester_id: "", course_id: "", block_id: "" })}
        />
        <FilterSelect
          id="qp-semester"
          label="Semester"
          value={semesterId}
          anyLabel={programId ? "Any semester" : "Pick a program first"}
          options={cascade.semesters.map((s) => ({
            value: s.id,
            label: `Semester ${s.semester_number}`,
          }))}
          onChange={(next) => set({ semester_id: next, course_id: "", block_id: "" })}
        />
        <FilterSelect
          id="qp-course"
          label="Course"
          value={courseId}
          anyLabel={semesterId ? "Any course" : "Pick a semester first"}
          options={cascade.courses.map((c) => ({ value: c.id, label: `${c.code} — ${c.name}` }))}
          onChange={(next) => set({ course_id: next, block_id: "" })}
        />
        <FilterSelect
          id="qp-block"
          label="Unit or module"
          value={blockId}
          anyLabel={courseId ? "Any unit (optional)" : "Pick a course first"}
          options={cascade.blocks.map((b) => ({
            value: b.id,
            label: `Unit ${b.block_no} — ${b.title}`,
          }))}
          onChange={(next) => set({ block_id: next })}
        />
        <FilterSelect
          id="qp-type"
          label="Assessment type"
          value={assessmentType}
          anyLabel="Any assessment type"
          options={ASSESSMENT_TYPES}
          onChange={(next) => set({ assessment_type: next })}
        />
        <FilterSelect
          id="qp-difficulty"
          label="Difficulty"
          value={difficulty}
          anyLabel="Any difficulty"
          options={DIFFICULTY_LEVELS}
          onChange={(next) => set({ difficulty_level: next })}
        />
        <FilterSelect
          id="qp-status"
          label="Status"
          value={status}
          anyLabel="Any status"
          options={STATUSES}
          onChange={(next) => set({ status: next })}
        />
      </FilterBar>

      <CourseCoverage
        programId={programId}
        semesterId={semesterId}
        selectedCourseId={courseId}
        onPick={(id) => set({ course_id: id, block_id: "" })}
      />

      <ActiveFilters labels={active} onClear={clearAll} />

      {error ? (
        <Banner tone="danger">{error}</Banner>
      ) : (
        <>
          <DataTable
            columns={columns}
            rows={data?.items ?? []}
            rowKey={(row) => row.id}
            loading={loading}
            empty="No questions match these filters."
            emptyHint={
              active.length > 0
                ? "Try clearing a filter — a block with no questions of this assessment type is common."
                : "Add a question manually, or import a file. An exam cannot be sampled until its semester has at least as many active questions as the paper asks for."
            }
          />

          <Pagination
            offset={offset}
            limit={PAGE_SIZE}
            total={data?.total ?? 0}
            onOffsetChange={setOffset}
          />
        </>
      )}
    </div>
  );
}

/** A/B/C/D for an option index. Papers are exactly four options (A1.1). */
function letterFor(index: number): string {
  return ["A", "B", "C", "D"][index] ?? String(index + 1);
}

/**
 * Per-course pool coverage (A2.5).
 *
 * The "which courses are still empty" view: every course in the program — empty
 * ones included — with the number of active questions in its pool, in ONE
 * request. Clicking a course filters the table to it.
 *
 * It appears as soon as a program is chosen, and narrows to the selected
 * semester when there is one. Empty is marked explicitly rather than shown as a
 * quiet zero, and a failed fetch says the counts are unavailable — claiming
 * "empty" there would send an admin to fill a pool that may already be full.
 */
function CourseCoverage({
  programId,
  semesterId,
  selectedCourseId,
  onPick,
}: {
  programId: string;
  semesterId: string;
  selectedCourseId: string;
  onPick: (courseId: string) => void;
}) {
  const { rows, loading, failed } = useQuestionPoolCounts(programId);

  if (!programId) return null;

  // The endpoint is program-wide and already ordered by semester then course
  // code, so narrowing to the chosen semester is a filter, not a re-sort.
  const shown = semesterId ? rows.filter((row) => row.semester_id === semesterId) : rows;

  return (
    <div className={`${panelClass} p-4`}>
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <p className="text-sm font-semibold text-gray-900">Pool coverage by course</p>
        <p className="text-xs text-gray-500">
          Active questions across every assessment type
          {semesterId ? ", in the selected semester" : ", in every semester of this program"}.
        </p>
      </div>

      {failed ? (
        <p className="mt-3 text-xs text-gray-500">
          Counts are unavailable right now, so this program&rsquo;s coverage cannot be shown. The
          question list below is unaffected.
        </p>
      ) : loading ? (
        <div className="mt-3 flex flex-wrap gap-2" aria-hidden="true">
          {[0, 1, 2, 3].map((i) => (
            <div key={i} className="h-7 w-28 animate-pulse rounded-full bg-lavender-100" />
          ))}
        </div>
      ) : shown.length === 0 ? (
        <p className="mt-3 text-xs text-gray-500">
          This {semesterId ? "semester" : "program"} has no courses yet, so there is nothing to pool
          questions against.
        </p>
      ) : (
        <ul className="mt-3 flex flex-wrap gap-2">
          {shown.map((row) => {
            const selected = selectedCourseId === row.course_id;
            const empty = row.active_questions === 0;

            return (
              <li key={row.course_id}>
                <button
                  type="button"
                  onClick={() => onPick(selected ? "" : row.course_id)}
                  aria-pressed={selected}
                  title={`${row.course_name} — Semester ${row.semester_number}`}
                  className={`flex items-center gap-2 rounded-full border px-3 py-1.5 text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-lavender-50 ${
                    selected
                      ? "border-indigo-400 bg-indigo-400/10 text-gray-900"
                      : "border-lavender-200 bg-white text-gray-900 hover:border-indigo-400"
                  }`}
                >
                  <span className="font-mono font-semibold">{row.course_code}</span>
                  {!semesterId ? (
                    <span className="text-gray-500">S{row.semester_number}</span>
                  ) : null}
                  {/* Empty is called out in words and in colour — colour alone
                      would not survive forced-colours mode (DESIGN.md §8). */}
                  <span className={empty ? "font-semibold text-danger" : "text-gray-500"}>
                    {empty
                      ? "empty"
                      : `${row.active_questions} question${row.active_questions === 1 ? "" : "s"}`}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
