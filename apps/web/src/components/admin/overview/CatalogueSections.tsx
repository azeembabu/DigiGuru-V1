// Catalogue, Students & enrolment, and Textbook ingestion.
//
// These three describe the platform at rest: what content exists, who can
// reach it, and whether the textbooks behind it are actually live. The
// ingestion numbers are the ones with teeth — a block cannot be taught from a
// document that has not finished embedding (`.claude/rules/rag-pipeline.md`),
// so `pending_review` and `failed` are tasks, not statistics.

"use client";

import { BarChart, DonutChart, LineChart, StatTile } from "@/components/charts";
import type {
  CatalogueMetrics,
  DocumentMetrics,
  StudentMetrics,
  AnalyticsSeries,
} from "@/lib/admin/analytics";

import {
  ACCENT,
  badIfAny,
  dayLabel,
  goodIfAny,
  humanLabel,
  n,
  ratio,
  shareOf,
  trendOf,
  warnIfAny,
} from "./format";
import { countWords, toEnrollments, toLscs, toPrograms, toStudents } from "./links";
import { ChartCard, MetricList, Section, SplitGrid, TileGrid } from "./parts";

export function CatalogueSection({
  catalogue,
  topBlocks,
}: {
  catalogue: CatalogueMetrics;
  topBlocks: AnalyticsSeries["top_blocks"];
}) {
  const taught = topBlocks.some((row) => row.count > 0);

  return (
    <Section
      id="catalogue-heading"
      title="Catalogue"
      description="Program > Semester > Course > Block. A student reaches content only through an enrolment."
    >
      <TileGrid>
        <StatTile
          label="Programs"
          value={n(catalogue.programs)}
          href={toPrograms()}
          linkLabel={`View all ${countWords(catalogue.programs)}programs`}
        />
        <StatTile label="Semesters" value={n(catalogue.semesters)} />
        <StatTile label="Courses" value={n(catalogue.courses)} />
        <StatTile
          label="Blocks"
          value={n(catalogue.blocks)}
          hint={`${n(catalogue.blocks_active)} active`}
        />
      </TileGrid>

      <SplitGrid>
        <MetricList
          title="Gaps in the hierarchy"
          hint="Each of these is a dead end a student can be routed into."
          rows={[
            {
              label: "Blocks without documents",
              value: n(catalogue.blocks_without_documents),
              hint: "Nothing to retrieve — the tutor can only abstain (NN-4).",
              tone: warnIfAny(catalogue.blocks_without_documents),
            },
            {
              label: "Courses without blocks",
              value: n(catalogue.courses_without_blocks),
              tone: warnIfAny(catalogue.courses_without_blocks),
            },
            {
              label: "Semesters without courses",
              value: n(catalogue.semesters_without_courses),
              tone: warnIfAny(catalogue.semesters_without_courses),
            },
            {
              label: "Inactive blocks",
              value: n(catalogue.blocks_inactive),
              hint: "Present but not offered to students.",
            },
            {
              label: "Learner support centres",
              value: n(catalogue.lscs),
              hint: "Platform-wide for every admin role.",
              href: toLscs(),
              linkLabel: `View all ${countWords(catalogue.lscs)}learner support centres`,
            },
          ]}
        />

        <ChartCard
          title="Most-taught blocks"
          hint="By session count, highest first."
          empty={!taught}
          emptyHint="No blocks have been taught yet — this ranks blocks once sessions start."
        >
          <BarChart
            horizontal
            data={topBlocks.map((row) => ({ label: row.label, value: row.count }))}
            accent={ACCENT.indigo}
            valueFormat={n}
          />
        </ChartCard>
      </SplitGrid>
    </Section>
  );
}

export function StudentsSection({
  students,
  daily,
}: {
  students: StudentMetrics;
  daily: AnalyticsSeries["students_daily"];
}) {
  const trend = trendOf(daily, (row) => row.count);
  const anyNew = trend.some((value) => value > 0);

  return (
    <Section
      id="students-heading"
      title="Students & enrolment"
      description="Account state, and whether those accounts are actually attached to a course."
    >
      <TileGrid>
        <StatTile
          label="Students"
          value={n(students.total)}
          trend={trend}
          href={toStudents()}
          linkLabel={`View all ${countWords(students.total)}student accounts`}
        />
        <StatTile
          label="Active"
          value={n(students.active)}
          hint={shareOf(students.active, students.total)}
          tone={goodIfAny(students.active)}
          href={toStudents("active")}
          linkLabel={`View ${countWords(students.active)}active student accounts`}
        />
        <StatTile
          label="New in 30 days"
          value={n(students.new_last_30d)}
          trend={trend}
          hint="Accounts created in the charted window."
        />
        <StatTile
          label="Never signed in"
          value={n(students.first_login_pending)}
          hint="Will hear the first-login greeting (NN-2)."
          tone={warnIfAny(students.first_login_pending)}
        />
      </TileGrid>

      <SplitGrid>
        <ChartCard
          title="New students — last 30 days"
          hint="Gap-filled daily, UTC."
          empty={!anyNew}
          emptyHint="No students registered in the last 30 days."
        >
          <LineChart
            data={daily.map((row) => ({ label: dayLabel(row.day), value: row.count }))}
            label="New students"
            accent={ACCENT.indigo}
            valueFormat={n}
          />
        </ChartCard>

        <MetricList
          title="Account and enrolment state"
          hint="A student with no enrolment can sign in but reaches no content."
          rows={[
            {
              label: "Inactive accounts",
              value: n(students.inactive),
              href: toStudents("inactive"),
              linkLabel: `View ${countWords(students.inactive)}inactive student accounts`,
            },
            {
              label: "Suspended accounts",
              value: n(students.suspended),
              tone: warnIfAny(students.suspended),
              href: toStudents("suspended"),
              linkLabel: `View ${countWords(students.suspended)}suspended student accounts`,
            },
            {
              label: "Without any enrolment",
              value: n(students.without_enrollment),
              hint: "No student_courses row — nothing to open.",
              tone: warnIfAny(students.without_enrollment),
            },
            {
              label: "Enrolments active",
              value: n(students.enrollments_active),
              tone: goodIfAny(students.enrollments_active),
              href: toEnrollments("active"),
              linkLabel: `View ${countWords(students.enrollments_active)}active enrolments`,
            },
            {
              label: "Enrolments completed",
              value: n(students.enrollments_completed),
              href: toEnrollments("completed"),
              linkLabel: `View ${countWords(students.enrollments_completed)}completed enrolments`,
            },
            {
              label: "Enrolments dropped",
              value: n(students.enrollments_dropped),
              tone: warnIfAny(students.enrollments_dropped),
              href: toEnrollments("dropped"),
              linkLabel: `View ${countWords(students.enrollments_dropped)}dropped enrolments`,
            },
          ]}
        />
      </SplitGrid>
    </Section>
  );
}

export function IngestionSection({
  documents,
  byStatus,
  daily,
}: {
  documents: DocumentMetrics;
  byStatus: AnalyticsSeries["documents_by_status"];
  daily: AnalyticsSeries["documents_daily"];
}) {
  const anyUploads = daily.some((row) => row.count > 0);
  const inFlight = documents.pending + documents.parsing;

  return (
    <Section
      id="ingestion-heading"
      title="Textbook ingestion"
      description="A block cannot be taught from a document that has not finished embedding."
    >
      <TileGrid>
        <StatTile
          label="Documents"
          value={n(documents.total)}
          trend={trendOf(daily, (row) => row.count)}
        />
        <StatTile
          label="Embedded & live"
          value={n(documents.embedded)}
          hint={shareOf(documents.embedded, documents.total)}
          tone={goodIfAny(documents.embedded)}
        />
        <StatTile
          label="Awaiting review"
          value={n(documents.pending_review)}
          hint="Low OCR confidence — held out of the syllabus until approved."
          tone={warnIfAny(documents.pending_review)}
        />
        <StatTile
          label="Failed"
          value={n(documents.failed)}
          hint="Never reached Qdrant. Re-upload or fix the source PDF."
          tone={badIfAny(documents.failed)}
        />
      </TileGrid>

      <SplitGrid>
        <ChartCard
          title="Documents by status"
          hint="Fixed label set, so the legend does not reorder between refreshes."
          empty={documents.total === 0}
          emptyHint="No textbooks uploaded yet — upload a PDF against a block to start ingestion."
        >
          <DonutChart
            data={byStatus.map((row) => ({ label: humanLabel(row.label), value: row.count }))}
            centerLabel={`${n(documents.total)} docs`}
            valueFormat={n}
          />
        </ChartCard>

        <ChartCard
          title="Uploads — last 30 days"
          empty={!anyUploads}
          emptyHint="No documents uploaded in the last 30 days."
        >
          <LineChart
            data={daily.map((row) => ({ label: dayLabel(row.day), value: row.count }))}
            label="Documents"
            accent={ACCENT.success}
            valueFormat={n}
          />
        </ChartCard>
      </SplitGrid>

      <SplitGrid>
        <MetricList
          title="Corpus"
          rows={[
            { label: "Pages parsed", value: n(documents.total_pages) },
            {
              label: "Mean OCR confidence",
              value: ratio(documents.avg_ocr_confidence),
              hint: "Null until something has been OCR'd — not zero.",
            },
            {
              label: "In flight (pending + parsing)",
              value: n(inFlight),
              hint: "Not yet retrievable.",
            },
          ]}
        />
        <MetricList
          title="Ingestion jobs"
          hint="The worker's own attempt ledger, which outlives a document's status."
          rows={[
            { label: "Queued", value: n(documents.jobs_pending) },
            { label: "Processing", value: n(documents.jobs_processing) },
            {
              label: "Completed",
              value: n(documents.jobs_completed),
              tone: goodIfAny(documents.jobs_completed),
            },
            {
              label: "Failed",
              value: n(documents.jobs_failed),
              tone: badIfAny(documents.jobs_failed),
            },
            {
              label: "Retried",
              value: n(documents.jobs_retried),
              tone: warnIfAny(documents.jobs_retried),
            },
          ]}
        />
      </SplitGrid>
    </Section>
  );
}
