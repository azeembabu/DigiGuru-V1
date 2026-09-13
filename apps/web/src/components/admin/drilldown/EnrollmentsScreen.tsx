"use client";

import { useCallback } from "react";

import { DataTable, type Column } from "@/components/admin/DataTable";
import { Pagination, SearchInput } from "@/components/admin/controls";
import { Banner, PageHeader, StatusPill } from "@/components/admin/primitives";
import { useResource } from "@/components/admin/academic/shared";
import {
  ActiveFilters,
  FilterBar,
  FilterSelect,
  PAGE_SIZE,
  RowLink,
  formatDateTime,
  useUrlFilters,
} from "@/components/admin/drilldown/shared";
import { listEnrollments } from "@/lib/admin/client";
import type { Enrollment, EnrollmentStatus } from "@/lib/admin/types";

const STATUSES: { value: EnrollmentStatus; label: string }[] = [
  { value: "active", label: "Active" },
  { value: "completed", label: "Completed" },
  { value: "dropped", label: "Dropped" },
];

function headingFor(status: string): { title: string; description: string } {
  switch (status) {
    case "active":
      return {
        title: "Active course enrolments",
        description:
          "Students currently reaching content through this course. A student reaches a block only through an enrolment — no row here means no syllabus.",
      };
    case "completed":
      return {
        title: "Completed course enrolments",
        description: "The student finished this course; the enrolment is kept as a record.",
      };
    case "dropped":
      return {
        title: "Dropped course enrolments",
        description:
          "These students no longer reach this course's blocks. Re-enrol them if that was not intended.",
      };
    default:
      return {
        title: "Course enrolments",
        description:
          "Every student-to-course assignment, newest first. This is the link that grants a student access to a course's blocks.",
      };
  }
}

function statusTone(status: EnrollmentStatus): "success" | "neutral" | "danger" {
  if (status === "active") return "success";
  if (status === "dropped") return "danger";
  return "neutral";
}

/** The `student_courses` drill-down: who can reach which course, and since when. */
export function EnrollmentsScreen() {
  const { get, offset, set, setOffset, clearAll } = useUrlFilters();
  const status = get("status");
  const q = get("q");
  const courseId = get("course_id");
  const studentId = get("student_id");

  const loader = useCallback(
    () =>
      listEnrollments({
        status: (status || undefined) as EnrollmentStatus | undefined,
        q: q || undefined,
        course_id: courseId || undefined,
        student_id: studentId || undefined,
        limit: PAGE_SIZE,
        offset,
      }),
    [status, q, courseId, studentId, offset],
  );
  const { data, loading, error } = useResource(loader, "Could not load enrolments.");

  const heading = headingFor(status);
  const active: string[] = [];
  if (status) active.push(STATUSES.find((item) => item.value === status)?.label ?? status);
  if (q) active.push(`“${q}”`);
  if (courseId) active.push("one course");
  if (studentId) active.push("one student");

  const columns: Column<Enrollment>[] = [
    {
      key: "student",
      header: "Student",
      cell: (row) => (
        <div className="min-w-[10rem]">
          <RowLink href={`/admin/students/${row.student_id}`}>{row.student_name}</RowLink>
          <p className="mt-0.5 font-mono text-xs text-gray-500">{row.roll_number}</p>
        </div>
      ),
    },
    {
      key: "course",
      header: "Course",
      cell: (row) => (
        <div className="min-w-[12rem]">
          <RowLink href={`/admin/courses/${row.course_id}`}>{row.course_name}</RowLink>
          <p className="mt-0.5 font-mono text-xs text-gray-500">{row.course_code}</p>
        </div>
      ),
    },
    {
      key: "semester",
      header: "Semester",
      className: "w-[110px]",
      cell: (row) => <span className="whitespace-nowrap">Sem {row.semester_number}</span>,
    },
    {
      key: "status",
      header: "Status",
      className: "w-[130px]",
      cell: (row) => <StatusPill tone={statusTone(row.status)}>{row.status}</StatusPill>,
    },
    {
      key: "assigned",
      header: "Assigned",
      cell: (row) => (
        <span className="whitespace-nowrap text-gray-500">{formatDateTime(row.assigned_at)}</span>
      ),
    },
    {
      key: "sessions",
      header: "Sessions",
      align: "right",
      className: "w-[130px]",
      cell: (row) => (
        <RowLink href={`/admin/sessions?student_id=${row.student_id}&course_id=${row.course_id}`}>
          View sessions
        </RowLink>
      ),
    },
  ];

  return (
    <div className="space-y-6">
      <PageHeader title={heading.title} description={heading.description} />

      <FilterBar>
        {/* Remounted per committed term so clearing the filters also clears the
            box, rather than cancelling the in-flight debounce. */}
        <SearchInput
          key={q}
          value={q}
          label="Search enrolments"
          placeholder="Student, roll number, or course…"
          onChange={(next) => set({ q: next })}
        />
        <FilterSelect
          id="enrollment-status"
          label="Enrolment status"
          value={status}
          anyLabel="Any status"
          options={STATUSES}
          onChange={(next) => set({ status: next })}
        />
      </FilterBar>

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
            empty="No enrolments match these filters."
            emptyHint={
              active.length > 0
                ? "Try a shorter search term, or clear the status filter."
                : "Enrol a student into a course from the student's own page to create a row here."
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
