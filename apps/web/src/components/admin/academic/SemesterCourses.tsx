"use client";

import { useCallback, useState } from "react";

import { AdminButton } from "@/components/admin/controls";
import { DataTable, type Column } from "@/components/admin/DataTable";
import { Banner, PageHeader, StatusPill, statusTone } from "@/components/admin/primitives";
import { CourseForm } from "@/components/admin/academic/CourseForm";
import {
  Blank,
  Breadcrumbs,
  CodeTag,
  RowLink,
  useResource,
} from "@/components/admin/academic/shared";
import { getProgram, getSemester, listCourses } from "@/lib/admin/client";
import type { Course, Program, Semester } from "@/lib/admin/types";

type Loaded = { semester: Semester; program: Program; courses: Course[] };

/**
 * Courses in one semester — a deep link, not the main path.
 *
 * The program page now lists every course in the program with semester as a
 * column, so this screen exists for links that already point at a semester.
 * Creating a course happens there, where the semester is a field rather than
 * the route.
 */
export function SemesterCourses({ semesterId }: { semesterId: string }) {
  const [form, setForm] = useState<{ course: Course } | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // `getSemester` gives this screen its own program, so it is correct from its
  // URL alone — the old `?program=` hint is no longer load-bearing.
  const loader = useCallback(async (): Promise<Loaded> => {
    const [semester, courses] = await Promise.all([
      getSemester(semesterId),
      listCourses(semesterId),
    ]);
    const program = await getProgram(semester.program_id);
    return { semester, program, courses };
  }, [semesterId]);

  const { data, loading, error, reload } = useResource(loader, "Could not load this semester.");

  const columns: Column<Course>[] = [
    {
      key: "code",
      header: "Code",
      className: "w-[140px]",
      cell: (row) => <CodeTag>{row.code}</CodeTag>,
    },
    {
      key: "name",
      header: "Name",
      cell: (row) => <RowLink href={`/admin/courses/${row.id}`}>{row.name}</RowLink>,
    },
    {
      key: "description",
      header: "Description",
      className: "max-w-[360px]",
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
        <AdminButton type="button" variant="outline-light" onClick={() => setForm({ course: row })}>
          Edit
        </AdminButton>
      ),
    },
  ];

  function handleSaved(message: string) {
    setForm(null);
    setNotice(message);
    reload();
  }

  return (
    <div className="space-y-6">
      <Breadcrumbs
        items={[
          { label: "Programs", href: "/admin/programs" },
          ...(data
            ? [{ label: data.program.name, href: `/admin/programs/${data.semester.program_id}` }]
            : []),
          {
            label: data ? `${data.semester.semester_number}. ${data.semester.name}` : "Semester",
          },
        ]}
      />

      <PageHeader
        title={data?.semester.name ?? "Semester"}
        description="Courses in this semester. Open one to manage its blocks and units."
        action={
          data ? (
            <RowLink href={`/admin/programs/${data.semester.program_id}`}>
              All courses in this program
            </RowLink>
          ) : null
        }
      />

      {notice ? (
        <Banner tone="success" onDismiss={() => setNotice(null)}>
          {notice}
        </Banner>
      ) : null}
      {error ? <Banner tone="danger">{error}</Banner> : null}

      {data && data.semester.status === "inactive" ? (
        <Banner tone="info">
          This semester is{" "}
          <StatusPill tone={statusTone("inactive")}>inactive</StatusPill> — students are not served
          content from its courses.
        </Banner>
      ) : null}

      <Banner tone="info">
        Courses are added from the program page, where every semester&rsquo;s courses are listed
        together.
      </Banner>

      <DataTable
        columns={columns}
        rows={data?.courses ?? []}
        rowKey={(row) => row.id}
        loading={loading}
        empty="No courses in this semester yet."
      />

      {/* Edit only: `UpdateCourseRequest` has no semester field, so the form's
          semester select is not rendered and no semester list is needed. */}
      {form && data ? (
        <CourseForm
          programId={data.semester.program_id}
          semesters={[]}
          course={form.course}
          onClose={() => setForm(null)}
          onSaved={handleSaved}
        />
      ) : null}
    </div>
  );
}
