"use client";

import { useCallback, useMemo, useState } from "react";

import { AdminButton, Pagination, SearchInput } from "@/components/admin/controls";
import { DataTable, type Column } from "@/components/admin/DataTable";
import {
  Banner,
  PageHeader,
  StatusPill,
  panelClass,
  statusTone,
} from "@/components/admin/primitives";
import { CourseForm } from "@/components/admin/academic/CourseForm";
import { ProgramForm } from "@/components/admin/academic/ProgramForm";
import { SemesterManager } from "@/components/admin/academic/SemesterManager";
import { Blank, Breadcrumbs, CodeTag, RowLink, useResource } from "@/components/admin/academic/shared";
import { getProgram, listCoursesForProgram, listSemesters } from "@/lib/admin/client";
import type { Course, EntityStatus, Program, Semester } from "@/lib/admin/types";

const PAGE_SIZE = 25;

type Frame = { program: Program; semesters: Semester[] };

/**
 * A program and every course in it, in one flat list.
 *
 * Semester is a column here rather than a level of navigation: an admin thinks
 * in Program › Course › Block, and making them pick a semester first hid the
 * course they were actually looking for behind a guess. The semester level is
 * untouched in the schema — `students.semester_id` and the Qdrant metadata
 * filter still key off it.
 *
 * Two resources rather than one: the program header and its semester list do
 * not change as the admin searches or pages, so they are fetched once and the
 * course page is fetched on its own.
 */
export function ProgramDetail({ programId }: { programId: string }) {
  const [programForm, setProgramForm] = useState(false);
  const [courseForm, setCourseForm] = useState<{ course: Course | null } | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const [offset, setOffset] = useState(0);

  const frameLoader = useCallback(
    async (): Promise<Frame> => {
      const [program, semesters] = await Promise.all([
        getProgram(programId),
        listSemesters(programId),
      ]);
      return { program, semesters };
    },
    [programId],
  );
  const frame = useResource(frameLoader, "Could not load this program.");

  // Ordered semester_number then course code, searched and paged server-side.
  const coursesLoader = useCallback(
    () => listCoursesForProgram(programId, { q: q || undefined, limit: PAGE_SIZE, offset }),
    [programId, q, offset],
  );
  const courses = useResource(coursesLoader, "Could not load this program's courses.");

  /**
   * `CourseResponse` carries the semester's number and name but not its
   * status, so an inactive semester is read off the program's semester list —
   * which this screen already loads once for the semester panel and the course
   * form. One lookup table for the page, never a request per row.
   */
  const semesterStatus = useMemo(() => {
    const map = new Map<string, EntityStatus>();
    for (const semester of frame.data?.semesters ?? []) map.set(semester.id, semester.status);
    return map;
  }, [frame.data]);

  const columns: Column<Course>[] = [
    {
      key: "code",
      header: "Code",
      className: "w-[130px]",
      cell: (row) => <CodeTag>{row.code}</CodeTag>,
    },
    {
      key: "name",
      header: "Course",
      cell: (row) => <RowLink href={`/admin/courses/${row.id}`}>{row.name}</RowLink>,
    },
    {
      key: "semester",
      header: "Semester",
      className: "w-[200px]",
      // Kept visible because the data model still has it: a student is placed
      // in a semester, and retrieval filters on it. The inactive marker is what
      // keeps "why can't students see this course?" answerable from the list.
      cell: (row) => (
        <span className="flex flex-wrap items-center gap-1.5">
          <span className="text-gray-500">
            {row.semester_number}. {row.semester_name}
          </span>
          {semesterStatus.get(row.semester_id) === "inactive" ? (
            <StatusPill tone={statusTone("inactive")}>semester inactive</StatusPill>
          ) : null}
        </span>
      ),
    },
    {
      key: "description",
      header: "Description",
      className: "max-w-[320px]",
      cell: (row) =>
        row.description ? (
          <span className="line-clamp-2 text-gray-500">{row.description}</span>
        ) : (
          <Blank />
        ),
    },
    {
      key: "actions",
      header: "Actions",
      align: "right",
      className: "w-[100px]",
      cell: (row) => (
        <AdminButton
          type="button"
          variant="outline-light"
          onClick={() => setCourseForm({ course: row })}
        >
          Edit
        </AdminButton>
      ),
    },
  ];

  function handleSaved(message: string) {
    setProgramForm(false);
    setCourseForm(null);
    setNotice(message);
    frame.reload();
    courses.reload();
  }

  const error = frame.error ?? courses.error;

  return (
    <div className="space-y-6">
      <Breadcrumbs
        items={[
          { label: "Programs", href: "/admin/programs" },
          { label: frame.data?.program.name ?? "Program" },
        ]}
      />

      <PageHeader
        title={frame.data?.program.name ?? "Program"}
        description="Every course in this program. Open one to manage its blocks and units."
        action={
          frame.data ? (
            <div className="flex gap-2">
              <AdminButton
                type="button"
                variant="outline-light"
                onClick={() => setProgramForm(true)}
              >
                Edit program
              </AdminButton>
              <AdminButton type="button" onClick={() => setCourseForm({ course: null })}>
                Add new course
              </AdminButton>
            </div>
          ) : null
        }
      />

      {notice ? (
        <Banner tone="success" onDismiss={() => setNotice(null)}>
          {notice}
        </Banner>
      ) : null}
      {error ? <Banner tone="danger">{error}</Banner> : null}

      {frame.data ? (
        <section aria-label="Program details" className={`${panelClass} p-5`}>
          <dl className="flex flex-wrap gap-x-10 gap-y-4">
            <div>
              <dt className="text-xs font-medium uppercase tracking-wide text-gray-500">Code</dt>
              <dd className="mt-1">
                <CodeTag>{frame.data.program.code}</CodeTag>
              </dd>
            </div>
            <div>
              <dt className="text-xs font-medium uppercase tracking-wide text-gray-500">Status</dt>
              <dd className="mt-1">
                <StatusPill tone={statusTone(frame.data.program.status)}>
                  {frame.data.program.status}
                </StatusPill>
              </dd>
            </div>
            <div className="min-w-[16rem] flex-1">
              <dt className="text-xs font-medium uppercase tracking-wide text-gray-500">
                Description
              </dt>
              <dd className="mt-1 text-sm text-gray-500">
                {frame.data.program.description ?? "No description."}
              </dd>
            </div>
          </dl>
        </section>
      ) : null}

      <div className="flex flex-wrap items-center gap-3">
        <SearchInput
          value={q}
          label="Search courses"
          placeholder="Search courses by name or code…"
          onChange={(next) => {
            setQ(next);
            setOffset(0);
          }}
        />
      </div>

      <DataTable
        columns={columns}
        rows={courses.data?.items ?? []}
        rowKey={(row) => row.id}
        loading={courses.loading}
        empty={q ? "No courses match that search." : "No courses in this program yet."}
        emptyHint={
          q ? "Try a shorter search term." : "Add the first one with “Add new course”."
        }
      />

      <Pagination
        offset={offset}
        limit={PAGE_SIZE}
        total={courses.data?.total ?? 0}
        onOffsetChange={setOffset}
      />

      {frame.data ? (
        <SemesterManager
          programId={programId}
          semesters={frame.data.semesters}
          onSaved={handleSaved}
        />
      ) : null}

      {/* Each dialog is mounted only while open, so it starts from the row it
          was opened on without a re-seeding effect. */}
      {programForm && frame.data ? (
        <ProgramForm
          program={frame.data.program}
          onClose={() => setProgramForm(false)}
          onSaved={handleSaved}
        />
      ) : null}

      {courseForm && frame.data ? (
        <CourseForm
          programId={programId}
          semesters={frame.data.semesters}
          course={courseForm.course}
          onClose={() => setCourseForm(null)}
          onSaved={handleSaved}
        />
      ) : null}
    </div>
  );
}
