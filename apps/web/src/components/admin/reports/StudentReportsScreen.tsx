"use client";

import { useCallback } from "react";

import { DataTable, type Column } from "@/components/admin/DataTable";
import { AdminInput, Pagination, SearchInput } from "@/components/admin/controls";
import { Banner, PageHeader } from "@/components/admin/primitives";
import { useResource } from "@/components/admin/academic/shared";
import {
  ActiveFilters,
  FilterBar,
  FilterSelect,
  PAGE_SIZE,
  RowLink,
  useUrlFilters,
} from "@/components/admin/drilldown/shared";
import { useCascade } from "@/components/admin/questions/cascade";
import {
  AttemptStatusPill,
  PercentPill,
  formatDateTime,
  formatDuration,
  formatScore,
} from "@/components/admin/reports/shared";
import { listStudentReports } from "@/lib/admin/client";
import {
  ASSESSMENT_TYPES,
  ATTEMPT_STATUSES,
  type AssessmentType,
  type AttemptStatus,
  type StudentReportRow,
} from "@/lib/admin/types";

// AMENDMENT A2.4 — every student exam attempt, filterable, each row opening that
// student's full report.
//
// Filters live in the URL like the other drill-downs, so "every ungraded
// semester exam in MAL101 since the 1st" is a link someone can send.
//
// Nothing is re-filtered or re-scoped here: the gateway runs the scoped query
// for a sub-admin and returns an empty array when it has no scopes. An empty
// table is therefore the correct answer, not a reason to retry unscoped.

/** `2026-09-01` -> `2026-09-01T00:00:00Z`; empty stays undefined. */
function dayStart(day: string): string | undefined {
  return day ? `${day}T00:00:00Z` : undefined;
}

/** `2026-09-30` -> `2026-09-30T23:59:59Z`, so the chosen day is included. */
function dayEnd(day: string): string | undefined {
  return day ? `${day}T23:59:59Z` : undefined;
}

export function StudentReportsScreen() {
  const { get, offset, set, setOffset, clearAll } = useUrlFilters();
  const programId = get("program_id");
  const semesterId = get("semester_id");
  const courseId = get("course_id");
  const examId = get("exam_id");
  const studentId = get("student_id");
  const assessmentType = get("assessment_type");
  const status = get("status");
  const from = get("from");
  const to = get("to");
  const q = get("q");

  const cascade = useCascade({ programId, semesterId, courseId });

  const loader = useCallback(
    () =>
      listStudentReports({
        program_id: programId || undefined,
        semester_id: semesterId || undefined,
        course_id: courseId || undefined,
        exam_id: examId || undefined,
        student_id: studentId || undefined,
        assessment_type: (assessmentType || undefined) as AssessmentType | undefined,
        status: (status || undefined) as AttemptStatus | undefined,
        // The pickers hold a bare `YYYY-MM-DD`; the gateway validates `from`/`to`
        // as full RFC3339 and answers 400 for a bare date (verified against the
        // live endpoint), so each is widened to an instant here. `to` becomes
        // end-of-day so that picking a single date includes attempts started
        // during it — a human picking "to the 30th" means through the 30th, not
        // up to midnight at its start. Boundaries are UTC, matching the analytics
        // series.
        from: dayStart(from),
        to: dayEnd(to),
        q: q || undefined,
        limit: PAGE_SIZE,
        offset,
      }),
    [programId, semesterId, courseId, examId, studentId, assessmentType, status, from, to, q, offset],
  );
  const { data, loading, error } = useResource(loader, "Could not load student reports.");

  const active: string[] = [];
  if (programId) active.push("one program");
  if (semesterId) active.push("one semester");
  if (courseId) active.push("one course");
  if (examId) active.push("one exam");
  if (studentId) active.push("one student");
  if (assessmentType) {
    active.push(ASSESSMENT_TYPES.find((t) => t.value === assessmentType)?.label ?? assessmentType);
  }
  if (status) active.push(ATTEMPT_STATUSES.find((s) => s.value === status)?.label ?? status);
  if (from) active.push(`from ${from}`);
  if (to) active.push(`to ${to}`);
  if (q) active.push(`“${q}”`);

  const columns: Column<StudentReportRow>[] = [
    {
      key: "student",
      header: "Student",
      cell: (row) => (
        <div className="min-w-[11rem]">
          {/* The whole point of the list: every row opens that student's report. */}
          <RowLink href={`/admin/student-reports/${row.student_id}`}>{row.student_name}</RowLink>
          <p className="mt-0.5 font-mono text-xs text-gray-500">{row.roll_number}</p>
        </div>
      ),
    },
    {
      key: "placement",
      header: "Program / semester",
      cell: (row) => (
        <div className="min-w-[10rem]">
          <p>{row.program_name}</p>
          <p className="mt-0.5 text-xs text-gray-500">Semester {row.semester_number}</p>
        </div>
      ),
    },
    {
      key: "course",
      header: "Course",
      cell: (row) => (
        <div className="min-w-[10rem]">
          <RowLink href={`/admin/courses/${row.course_id}`}>{row.course_name}</RowLink>
          <p className="mt-0.5 font-mono text-xs text-gray-500">{row.course_code}</p>
        </div>
      ),
    },
    {
      key: "exam",
      header: "Exam",
      cell: (row) => (
        <div className="min-w-[12rem]">
          <p className="font-medium text-gray-900">{row.exam_title}</p>
          <p className="mt-0.5 text-xs text-gray-500">
            {ASSESSMENT_TYPES.find((t) => t.value === row.assessment_type)?.label ??
              row.assessment_type}{" "}
            · attempt {row.attempt_no}
          </p>
        </div>
      ),
    },
    {
      key: "score",
      header: "Score",
      align: "right",
      className: "w-[8rem]",
      cell: (row) => (
        <div className="whitespace-nowrap">
          <p className="tabular-nums">{formatScore(row.score, row.max_score)}</p>
          <p className="mt-0.5 text-xs text-gray-500 tabular-nums">
            {row.correct_answers}/{row.total_questions} correct
          </p>
        </div>
      ),
    },
    {
      key: "percentage",
      header: "Result",
      align: "right",
      className: "w-[7rem]",
      cell: (row) => <PercentPill value={row.percentage} />,
    },
    {
      key: "time",
      header: "Time spent",
      align: "right",
      className: "w-[7.5rem]",
      cell: (row) => (
        <span className="whitespace-nowrap tabular-nums text-gray-500">
          {formatDuration(row.time_spent_seconds)}
        </span>
      ),
    },
    {
      key: "status",
      header: "Status",
      className: "w-[8rem]",
      cell: (row) => <AttemptStatusPill status={row.status} />,
    },
    {
      key: "submitted",
      header: "Submitted",
      align: "right",
      cell: (row) => (
        <span className="whitespace-nowrap text-gray-500">{formatDateTime(row.submitted_at)}</span>
      ),
    },
  ];

  return (
    <div className="space-y-6">
      <PageHeader
        title="Student reports"
        description="Every exam attempt, newest first. A score or percentage is shown only once the attempt has been graded — an ungraded attempt reads as a dash, never as zero. Open a row for that student's full record."
      />

      <FilterBar>
        <SearchInput
          key={q}
          value={q}
          label="Search student reports"
          placeholder="Student, roll number, or exam title…"
          onChange={(next) => set({ q: next })}
        />
        <FilterSelect
          id="sr-program"
          label="Program"
          value={programId}
          anyLabel="Any program"
          options={cascade.programs.map((p) => ({ value: p.id, label: `${p.code} — ${p.name}` }))}
          onChange={(next) => set({ program_id: next, semester_id: "", course_id: "" })}
        />
        <FilterSelect
          id="sr-semester"
          label="Semester"
          value={semesterId}
          anyLabel={programId ? "Any semester" : "Pick a program first"}
          options={cascade.semesters.map((s) => ({
            value: s.id,
            label: `Semester ${s.semester_number}`,
          }))}
          onChange={(next) => set({ semester_id: next, course_id: "" })}
        />
        <FilterSelect
          id="sr-course"
          label="Course"
          value={courseId}
          anyLabel={semesterId ? "Any course" : "Pick a semester first"}
          options={cascade.courses.map((c) => ({ value: c.id, label: `${c.code} — ${c.name}` }))}
          onChange={(next) => set({ course_id: next })}
        />
        <FilterSelect
          id="sr-type"
          label="Assessment type"
          value={assessmentType}
          anyLabel="Any assessment type"
          options={ASSESSMENT_TYPES}
          onChange={(next) => set({ assessment_type: next })}
        />
        <FilterSelect
          id="sr-status"
          label="Status"
          value={status}
          anyLabel="Any status"
          options={ATTEMPT_STATUSES}
          onChange={(next) => set({ status: next })}
        />

        {/* Native date inputs: the range filters `started_at`, and a text box
            would invite formats the gateway validates rather than clamps. */}
        <div className="flex items-end gap-2">
          <div>
            <label htmlFor="sr-from" className="block text-xs font-medium text-gray-500">
              Started from
            </label>
            <AdminInput
              id="sr-from"
              type="date"
              value={from}
              max={to || undefined}
              onChange={(event) => set({ from: event.target.value })}
            />
          </div>
          <div>
            <label htmlFor="sr-to" className="block text-xs font-medium text-gray-500">
              Started to
            </label>
            <AdminInput
              id="sr-to"
              type="date"
              value={to}
              min={from || undefined}
              onChange={(event) => set({ to: event.target.value })}
            />
          </div>
        </div>
      </FilterBar>

      <ActiveFilters labels={active} onClear={clearAll} />

      {error ? (
        <Banner tone="danger">{error}</Banner>
      ) : (
        <>
          <DataTable
            columns={columns}
            rows={data?.items ?? []}
            rowKey={(row) => row.attempt_id}
            loading={loading}
            empty="No attempts match these filters."
            emptyHint={
              active.length > 0
                ? "Try widening the date range or clearing the status filter."
                : "A row appears here as soon as a student starts a published exam."
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
