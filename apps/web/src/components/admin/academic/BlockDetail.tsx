"use client";

import { useCallback, useState } from "react";

import { AdminButton } from "@/components/admin/controls";
import {
  Banner,
  PageHeader,
  StatusPill,
  panelClass,
  statusTone,
} from "@/components/admin/primitives";
import { AddUnitForm } from "@/components/admin/academic/AddUnitForm";
import { BlockForm } from "@/components/admin/academic/BlockForm";
import { Breadcrumbs, useResource } from "@/components/admin/academic/shared";
import { UnitsTable } from "@/components/admin/academic/units";
import { getBlock, getCourse, getProgram, listBlockDocuments } from "@/lib/admin/client";
import type { AdminDocument, Block, Course, Program } from "@/lib/admin/types";

type Loaded = { units: AdminDocument[]; block: Block; course: Course; program: Program };

/**
 * One block and its units, in full.
 *
 * The deep-link view of what the course screen shows collapsed. This is where
 * the syllabus boundary is actually set: NN-4 means the tutor can only ever say
 * what these units contain, so each one's ingestion state is operational
 * information, not a detail.
 */
export function BlockDetail({ blockId }: { blockId: string }) {
  const [edit, setEdit] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  // A block response carries its own ancestry (`course_id`, `semester_id`,
  // `program_id`), so this screen is correct from its URL alone — no query
  // parameter has to be carried in from the page that linked here.
  const loader = useCallback(async (): Promise<Loaded> => {
    const [block, units] = await Promise.all([
      getBlock(blockId),
      listBlockDocuments(blockId),
    ]);
    const [course, program] = await Promise.all([
      getCourse(block.course_id),
      getProgram(block.program_id),
    ]);
    return { units, block, course, program };
  }, [blockId]);

  const { data, loading, error, reload } = useResource(loader, "Could not load this block.");

  const units = data?.units ?? [];
  const awaitingReview = units.filter((row) => row.status === "pending_review").length;

  function handleSaved(message: string) {
    setEdit(false);
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
                { label: data.program.name, href: `/admin/programs/${data.block.program_id}` },
                {
                  label: `${data.course.semester_number}. ${data.course.semester_name}`,
                  href: `/admin/semesters/${data.block.semester_id}`,
                },
                { label: data.course.name, href: `/admin/courses/${data.block.course_id}` },
              ]
            : []),
          { label: data?.block.title ?? "Block" },
        ]}
      />

      <PageHeader
        title={data?.block.title ?? "Block"}
        description="Units in this block. A unit is a PDF, and the tutor can only teach from one that has finished embedding."
        action={
          data ? (
            <AdminButton type="button" variant="outline-light" onClick={() => setEdit(true)}>
              Edit block
            </AdminButton>
          ) : null
        }
      />

      {notice ? (
        <Banner tone="success" onDismiss={() => setNotice(null)}>
          {notice}
        </Banner>
      ) : null}
      {error ? <Banner tone="danger">{error}</Banner> : null}

      {awaitingReview > 0 ? (
        <Banner tone="info">
          {awaitingReview} unit{awaitingReview === 1 ? "" : "s"} came back with low OCR confidence
          and {awaitingReview === 1 ? "is" : "are"} awaiting review. Nothing in{" "}
          {awaitingReview === 1 ? "it" : "them"} is retrievable by the tutor until a sub admin
          approves {awaitingReview === 1 ? "it" : "them"}.
        </Banner>
      ) : null}

      {data ? (
        <section aria-label="Block details" className={`${panelClass} p-5`}>
          <dl className="flex flex-wrap gap-x-10 gap-y-4">
            <div>
              <dt className="text-xs font-medium uppercase tracking-wide text-gray-500">Block</dt>
              <dd className="mt-1 font-display text-lg font-semibold tabular-nums text-gray-900">
                {data.block.block_no}
              </dd>
            </div>
            <div>
              <dt className="text-xs font-medium uppercase tracking-wide text-gray-500">Status</dt>
              <dd className="mt-1">
                <StatusPill tone={statusTone(data.block.status)}>{data.block.status}</StatusPill>
              </dd>
            </div>
            <div className="min-w-[16rem] flex-1">
              <dt className="text-xs font-medium uppercase tracking-wide text-gray-500">
                Description
              </dt>
              <dd className="mt-1 text-sm text-gray-500">
                {data.block.description ?? "No description."}
              </dd>
            </div>
          </dl>
        </section>
      ) : null}

      <UnitsTable units={units} loading={loading} showUploaded />

      <AddUnitForm
        blockId={blockId}
        onUploaded={(message) => {
          setNotice(message);
          reload();
        }}
      />

      <p className="text-xs text-gray-500">
        Queued → Parsing → Embedded is automatic. A unit that comes back with low OCR confidence
        stops at Awaiting review and stays out of the syllabus until a sub admin approves it; a
        failed unit is never retrievable and must be re-uploaded.
      </p>

      {edit && data ? (
        <BlockForm
          courseId={data.block.course_id}
          block={data.block}
          onClose={() => setEdit(false)}
          onSaved={handleSaved}
        />
      ) : null}
    </div>
  );
}
