"use client";

import { useCallback, useState } from "react";

import { AdminButton } from "@/components/admin/controls";
import { Banner, EmptyState, PageHeader, TableSkeleton, panelClass } from "@/components/admin/primitives";
import { BlockForm } from "@/components/admin/academic/BlockForm";
import { BlockPanel } from "@/components/admin/academic/BlockPanel";
import { Breadcrumbs, useResource } from "@/components/admin/academic/shared";
import { getCourse, getProgram, listBlocks } from "@/lib/admin/client";
import type { Block, Course, Program } from "@/lib/admin/types";

type Loaded = { course: Course; program: Program; blocks: Block[] };

/**
 * The main content screen: a course's blocks, each holding its units.
 *
 * Blocks are an accordion rather than a table because a course can have a
 * dozen of them and each one contains a list of its own — expanded inline,
 * that is a page an admin has to scroll past to reach anything. Collapsed, the
 * whole course fits on one screen and only the block being worked on is open.
 */
export function CourseBlocks({ courseId }: { courseId: string }) {
  const [form, setForm] = useState<{ block: Block | null } | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // The course row carries its own ancestry (`program_id`, `semester_number`,
  // `semester_name`), so the only extra hop the trail needs is the program's
  // name — the blocks load in parallel with all of it.
  const loader = useCallback(async (): Promise<Loaded> => {
    const [course, blocks] = await Promise.all([getCourse(courseId), listBlocks(courseId)]);
    const program = await getProgram(course.program_id);
    return { course, program, blocks };
  }, [courseId]);

  const { data, loading, error, reload } = useResource(loader, "Could not load this course.");

  const blocks = data?.blocks ?? [];

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
            ? [
                { label: data.program.name, href: `/admin/programs/${data.course.program_id}` },
                {
                  label: `${data.course.semester_number}. ${data.course.semester_name}`,
                  href: `/admin/semesters/${data.course.semester_id}`,
                },
              ]
            : []),
          { label: data?.course.name ?? "Course" },
        ]}
      />

      <PageHeader
        title={data?.course.name ?? "Blocks"}
        description="Expand a block to see and add its units. A unit is a PDF the tutor is allowed to teach from."
        action={
          <AdminButton type="button" onClick={() => setForm({ block: null })}>
            Add block
          </AdminButton>
        }
      />

      {notice ? (
        <Banner tone="success" onDismiss={() => setNotice(null)}>
          {notice}
        </Banner>
      ) : null}
      {error ? <Banner tone="danger">{error}</Banner> : null}

      {loading ? (
        <div className={`${panelClass} overflow-hidden`}>
          <TableSkeleton rows={4} columns={3} />
        </div>
      ) : blocks.length === 0 ? (
        <div className={panelClass}>
          <EmptyState
            title="No blocks in this course yet."
            hint="Add the first one with “Add block”, then upload its units."
          />
        </div>
      ) : (
        <div className="space-y-3">
          {blocks.map((block) => (
            <BlockPanel
              key={block.id}
              block={block}
              onEdit={() => setForm({ block })}
              onNotice={setNotice}
            />
          ))}
        </div>
      )}

      {form ? (
        <BlockForm
          courseId={courseId}
          block={form.block}
          onClose={() => setForm(null)}
          onSaved={handleSaved}
        />
      ) : null}
    </div>
  );
}
