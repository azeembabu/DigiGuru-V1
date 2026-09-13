"use client";

import { useCallback } from "react";

import { DataTable, type Column } from "@/components/admin/DataTable";
import { Banner, EmptyState, PageHeader, panelClass } from "@/components/admin/primitives";
import { Breadcrumbs, useResource } from "@/components/admin/academic/shared";
import { RowLink } from "@/components/admin/drilldown/shared";
import {
  Dash,
  NOT_YET,
  PercentPill,
  formatDuration,
  formatPercent,
} from "@/components/admin/reports/shared";
import { getStudentReport } from "@/lib/admin/client";
import type { StudentReportCourse, StudentReportWeakTopic } from "@/lib/admin/types";

// One student's academic record (A2.4): profile, per-course rollup, and the
// weak topics aggregated across every attempt, ordered by how often each was
// missed.
//
// The ordering is the gateway's and is not re-sorted here — it is the answer to
// "what should this student revise first", and re-deriving it in the browser
// from a page of rows would silently disagree with the server on ties.

export function StudentReportDetail({ studentId }: { studentId: string }) {
  const loader = useCallback(() => getStudentReport(studentId), [studentId]);
  const { data, loading, error } = useResource(loader, "Could not load this student's report.");

  const courseColumns: Column<StudentReportCourse>[] = [
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
      key: "attempts",
      header: "Attempts",
      align: "right",
      className: "w-[7rem]",
      cell: (row) => <span className="tabular-nums">{row.attempts}</span>,
    },
    {
      key: "average",
      header: "Average",
      align: "right",
      className: "w-[8rem]",
      // `null` means nothing graded yet in this course. A dash, not 0%.
      cell: (row) => <PercentPill value={row.average_percentage} />,
    },
    {
      key: "attemptsLink",
      header: "",
      align: "right",
      className: "w-[9rem]",
      cell: (row) => (
        <RowLink href={`/admin/student-reports?student_id=${studentId}&course_id=${row.course_id}`}>
          View attempts
        </RowLink>
      ),
    },
  ];

  const topicColumns: Column<StudentReportWeakTopic>[] = [
    { key: "topic", header: "Topic", cell: (row) => row.topic },
    {
      key: "missed",
      header: "Questions missed",
      align: "right",
      className: "w-[11rem]",
      cell: (row) => <span className="tabular-nums">{row.missed_count}</span>,
    },
  ];

  return (
    <div className="space-y-6">
      <Breadcrumbs
        items={[
          { label: "Student reports", href: "/admin/student-reports" },
          { label: data?.student_name ?? "Report" },
        ]}
      />

      {error ? (
        <Banner tone="danger">{error}</Banner>
      ) : (
        <>
          <PageHeader
            title={data?.student_name ?? "Student report"}
            description={
              data === null
                ? undefined
                : `${data.roll_number} · ${data.program_name} · Semester ${data.semester_number}${
                    data.lsc_code ? ` · LSC ${data.lsc_code}` : ""
                  }`
            }
            action={
              data === null ? undefined : (
                <RowLink href={`/admin/students/${data.student_id}`}>Open student record</RowLink>
              )
            }
          />

          <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-5">
            <Metric label="Attempts" value={data === null ? null : String(data.attempts_total)} />
            <Metric label="Graded" value={data === null ? null : String(data.attempts_graded)} />
            <Metric
              label="Average"
              value={data === null ? null : formatPercent(data.average_percentage)}
              hint={
                data !== null && data.average_percentage === null
                  ? "Nothing graded yet"
                  : undefined
              }
            />
            <Metric
              label="Best"
              value={data === null ? null : formatPercent(data.best_percentage)}
              hint={
                data !== null && data.best_percentage === null ? "Nothing graded yet" : undefined
              }
            />
            <Metric
              label="Time in exams"
              value={data === null ? null : formatDuration(data.total_time_spent_seconds)}
            />
          </div>

          <section className="space-y-3">
            <h2 className="font-display text-lg font-semibold text-gray-900">By course</h2>
            <DataTable
              columns={courseColumns}
              rows={data?.by_course ?? []}
              rowKey={(row) => row.course_id}
              loading={loading}
              empty="No attempts in any course yet."
              emptyHint="This student is enrolled but has not started a published exam."
            />
          </section>

          <section className="space-y-3">
            <h2 className="font-display text-lg font-semibold text-gray-900">
              Weak topics, most missed first
            </h2>
            {loading || (data !== null && data.weak_topics.length > 0) ? (
              <DataTable
                columns={topicColumns}
                rows={data?.weak_topics ?? []}
                rowKey={(row) => row.topic}
                loading={loading}
                empty="No weak topics recorded."
              />
            ) : (
              <div className={panelClass}>
                <EmptyState
                  title="No weak topics recorded"
                  hint={
                    data !== null && data.attempts_graded === 0
                      ? "Weak topics are derived from incorrect answers on graded attempts. Nothing has been graded yet."
                      : "Every question this student answered was correct."
                  }
                />
              </div>
            )}
          </section>
        </>
      )}
    </div>
  );
}

/** One headline figure. `null` while loading; a dash when unmeasured. */
function Metric({
  label,
  value,
  hint,
}: {
  label: string;
  value: string | null;
  hint?: string;
}) {
  return (
    <div className={`${panelClass} p-4`}>
      <p className="text-xs uppercase tracking-wide text-gray-500">{label}</p>
      <p className="mt-1 font-display text-2xl font-semibold tabular-nums text-gray-900">
        {value === null ? (
          <span className="inline-block h-7 w-16 animate-pulse rounded-sm bg-lavender-100" />
        ) : value === NOT_YET ? (
          <Dash />
        ) : (
          value
        )}
      </p>
      {hint ? <p className="mt-1 text-xs text-gray-500">{hint}</p> : null}
    </div>
  );
}
