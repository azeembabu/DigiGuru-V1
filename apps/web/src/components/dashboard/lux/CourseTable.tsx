"use client";

import { ASSESSMENT_TYPE_LABEL, type AssessmentType } from "@/lib/student";
import { formatPct, type CourseRow } from "@/lib/dashboard-metrics";
import { Badge, Eyebrow, SectionHeading, SubText, lightCard } from "./shell";
import { LuxSparkline, TrendUnavailable, VizEmpty } from "./viz";

// The per-course breakdown, grouped from `GET /student/exams` and given its
// trendlines by `GET /student/exam-attempts`.
//
// Grouped from the exams list rather than the attempts list on purpose: a course
// with a published assessment the student has never opened still earns a row.
// Grouping from attempts would silently hide exactly the work left to do.

function typeLabel(type: string): string {
  // Narrow rather than cast: the table renders whatever the grouping produced,
  // and an assessment type this build does not know must show as itself rather
  // than crash a lookup or render `undefined`.
  return type in ASSESSMENT_TYPE_LABEL
    ? ASSESSMENT_TYPE_LABEL[type as AssessmentType]
    : type;
}

export function CourseTable({
  rows,
  /** The sidebar's search text. Empty means no filter. */
  filter,
  /** True when a filter or course selection is active but matched nothing. */
  totalRowCount,
}: {
  rows: CourseRow[];
  filter: string;
  totalRowCount: number;
}) {
  return (
    <section aria-labelledby="courses-heading" className={`${lightCard} p-5 sm:p-6`}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <Eyebrow>By course</Eyebrow>
          <SectionHeading id="courses-heading">Module breakdown</SectionHeading>
        </div>
        <SubText>
          {rows.length} of {totalRowCount} course{totalRowCount === 1 ? "" : "s"} shown
        </SubText>
      </div>

      {totalRowCount === 0 ? (
        <div className="mt-5">
          <VizEmpty
            kind="empty"
            title="No courses with assessments yet"
            hint="Courses appear here once your centre publishes an assessment against one of your blocks."
          />
        </div>
      ) : rows.length === 0 ? (
        <div className="mt-5">
          <VizEmpty
            kind="empty"
            title="Nothing matches your filter"
            hint={
              filter.trim() === ""
                ? "No course matches the selected workspace."
                : `No course matches “${filter.trim()}”. Clear the filter in the sidebar to see them all.`
            }
          />
        </div>
      ) : (
        // Its own scroll container, so the page itself never scrolls sideways at
        // 400px — the table is the only thing allowed to overflow.
        <div className="mt-5 -mx-1 overflow-x-auto px-1">
          <table className="w-full min-w-[620px] border-collapse text-left">
            <caption className="sr-only">
              Your courses, with assessment counts, attempts, best score and recent trend.
            </caption>
            <thead>
              <tr className="border-b border-lux-cream-300 text-[11.5px] uppercase tracking-[0.07em] text-lux-ink-600">
                <th scope="col" className="py-2.5 pr-4 font-semibold">Course</th>
                <th scope="col" className="py-2.5 pr-4 font-semibold">Type</th>
                <th scope="col" className="py-2.5 pr-4 text-right font-semibold">Assessments</th>
                <th scope="col" className="py-2.5 pr-4 text-right font-semibold">Attempts</th>
                <th scope="col" className="py-2.5 pr-4 text-right font-semibold">Best</th>
                <th scope="col" className="py-2.5 font-semibold">Trend</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr
                  key={row.courseId}
                  className="border-b border-lux-cream-200 last:border-0 hover:bg-lux-cream-200/40"
                >
                  <th scope="row" className="py-3 pr-4 font-normal">
                    <span className="block text-[14.5px] font-semibold leading-[1.35] text-lux-ink-900">
                      {row.courseCode}
                    </span>
                    <span className="block text-[13px] leading-[1.45] text-lux-ink-600">
                      {row.courseName}
                    </span>
                  </th>

                  <td className="py-3 pr-4">
                    <span className="flex flex-wrap gap-1">
                      {row.assessmentTypes.map((type) => (
                        <Badge key={type} tone="neutral">
                          {typeLabel(type)}
                        </Badge>
                      ))}
                    </span>
                  </td>

                  {/* `tabular-nums` here and not on the KPI values: these are
                      columns of numbers that have to line up vertically, which
                      is the one case the variant is for. */}
                  <td className="py-3 pr-4 text-right text-[14px] tabular-nums text-lux-ink-700">
                    {row.assessmentCount}
                  </td>
                  <td className="py-3 pr-4 text-right text-[14px] tabular-nums text-lux-ink-700">
                    {row.attemptsUsed}
                  </td>
                  <td className="py-3 pr-4 text-right text-[14px] font-semibold tabular-nums text-lux-ink-900">
                    {/* A course with no marked attempt has no best score. An
                        em-dash, never `0%` — the seeded student has a real 0%
                        attempt, so the two must stay distinguishable. */}
                    {formatPct(row.bestPercentage)}
                  </td>

                  <td className="py-3">
                    {row.trend.length >= 2 ? (
                      <LuxSparkline
                        values={row.trend}
                        mode="light"
                        label={`${row.courseCode} trend across ${row.trend.length} marked attempts: ${row.trend
                          .map((v) => formatPct(v))
                          .join(", ")}`}
                      />
                    ) : (
                      <TrendUnavailable />
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
