"use client";

import { useState } from "react";

import { AdminButton } from "@/components/admin/controls";
import { DataTable, type Column } from "@/components/admin/DataTable";
import { StatusPill, panelClass, statusTone } from "@/components/admin/primitives";
import { SemesterForm } from "@/components/admin/academic/SemesterForm";
import { RowLink } from "@/components/admin/academic/shared";
import type { Semester } from "@/lib/admin/types";

/**
 * Semester administration, folded away under the course list.
 *
 * Semesters are flattened out of the navigation, not out of the product: a
 * course cannot be created without one, and `students.semester_id` and the
 * Qdrant metadata filter still key off them. Deleting this panel would leave a
 * fresh program with no way to ever add its first course — so it stays,
 * collapsed, out of the way of the main Program › Course › Block flow.
 */
export function SemesterManager({
  programId,
  semesters,
  onSaved,
}: {
  programId: string;
  semesters: Semester[];
  onSaved: (message: string) => void;
}) {
  const [form, setForm] = useState<{ semester: Semester | null } | null>(null);

  const columns: Column<Semester>[] = [
    {
      key: "number",
      header: "#",
      className: "w-[70px]",
      align: "right",
      cell: (row) => <span className="tabular-nums">{row.semester_number}</span>,
    },
    {
      key: "name",
      header: "Name",
      cell: (row) => (
        <RowLink href={`/admin/semesters/${row.id}`}>{row.name}</RowLink>
      ),
    },
    {
      key: "status",
      header: "Status",
      className: "w-[120px]",
      cell: (row) => <StatusPill tone={statusTone(row.status)}>{row.status}</StatusPill>,
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
          onClick={() => setForm({ semester: row })}
        >
          Edit
        </AdminButton>
      ),
    },
  ];

  // Folded away once the program is set up, but open on arrival while there
  // are none — a program with no semesters cannot have a course at all, so
  // hiding the only way to fix that behind a closed panel makes the screen
  // look like a dead end.
  const hasNone = semesters.length === 0;

  return (
    <details open={hasNone} className={`${panelClass} group px-5 py-4`}>
      <summary className="cursor-pointer list-none rounded-sm text-sm font-medium text-gray-900 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2 focus-visible:ring-offset-lavender-50">
        <span aria-hidden="true" className="mr-2 inline-block transition-transform group-open:rotate-90">
          &rsaquo;
        </span>
        Semesters ({semesters.length})
        <span className="ml-2 font-normal text-gray-500">
          {hasNone
            ? "— add one before you can add a course."
            : "— courses are grouped by these; students are placed in one."}
        </span>
      </summary>

      <div className="mt-4 space-y-4">
        <div className="flex justify-end">
          <AdminButton type="button" onClick={() => setForm({ semester: null })}>
            New semester
          </AdminButton>
        </div>

        <DataTable
          columns={columns}
          rows={semesters}
          rowKey={(row) => row.id}
          empty="No semesters yet."
          emptyHint="A course must belong to a semester, so add one before adding courses."
        />
      </div>

      {form ? (
        <SemesterForm
          programId={programId}
          semester={form.semester}
          onClose={() => setForm(null)}
          onSaved={(message) => {
            setForm(null);
            onSaved(message);
          }}
        />
      ) : null}
    </details>
  );
}
