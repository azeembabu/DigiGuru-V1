"use client";

import type { ReactNode } from "react";

import { EmptyState, TableSkeleton, panelClass } from "@/components/admin/primitives";

export type Column<T> = {
  /** Stable key, also used for the React key of each cell. */
  key: string;
  header: string;
  /** Cell renderer. Returning a plain string is fine. */
  cell: (row: T) => ReactNode;
  /** Tailwind width/alignment overrides for this column. */
  className?: string;
  /** Right-align numeric and action columns. */
  align?: "left" | "right";
};

/**
 * One table for every admin list.
 *
 * Data-dense by design (DESIGN.md §7: "Admin console ... data-dense CRUD
 * surfaces"), so rows are tight and the header is sticky. It owns no fetching,
 * paging, or filtering — a screen passes rows in and renders controls around
 * it, which keeps the table trivially testable and stops each screen inventing
 * its own empty and loading states.
 *
 * The horizontal scroll container is on the table alone: an admin table is one
 * of the few things allowed to be wider than the viewport, but the page itself
 * must never scroll sideways.
 */
export function DataTable<T>({
  columns,
  rows,
  rowKey,
  loading = false,
  empty,
  emptyHint,
}: {
  columns: Column<T>[];
  rows: T[];
  rowKey: (row: T) => string;
  loading?: boolean;
  empty: string;
  emptyHint?: string;
}) {
  if (loading) {
    return (
      <div className={`${panelClass} overflow-hidden`}>
        <TableSkeleton columns={columns.length} />
      </div>
    );
  }

  if (rows.length === 0) {
    return (
      <div className={panelClass}>
        <EmptyState title={empty} hint={emptyHint} />
      </div>
    );
  }

  return (
    <div className={`${panelClass} overflow-x-auto`}>
      <table className="w-full min-w-[640px] border-collapse text-sm">
        <thead>
          <tr className="border-b border-lavender-200 bg-lavender-50/60">
            {columns.map((column) => (
              <th
                key={column.key}
                scope="col"
                className={`px-4 py-2.5 text-xs font-semibold uppercase tracking-wide text-gray-500 ${
                  column.align === "right" ? "text-right" : "text-left"
                } ${column.className ?? ""}`}
              >
                {column.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-lavender-200">
          {rows.map((row) => (
            <tr key={rowKey(row)} className="hover:bg-lavender-50/70">
              {columns.map((column) => (
                <td
                  key={column.key}
                  className={`px-4 py-3 align-middle text-gray-900 ${
                    column.align === "right" ? "text-right" : "text-left"
                  } ${column.className ?? ""}`}
                >
                  {column.cell(row)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
