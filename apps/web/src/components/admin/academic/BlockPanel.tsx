"use client";

import { useCallback, useState } from "react";

import { AdminButton } from "@/components/admin/controls";
import { Banner, StatusPill, panelClass, statusTone } from "@/components/admin/primitives";
import { AddUnitForm } from "@/components/admin/academic/AddUnitForm";
import { RowLink, useResource } from "@/components/admin/academic/shared";
import { UnitsTable } from "@/components/admin/academic/units";
import { listBlockDocuments } from "@/lib/admin/client";
import type { AdminDocument, Block } from "@/lib/admin/types";

/**
 * One block as a collapsible panel, with its units inside.
 *
 * Native `<details>`/`<summary>`: the platform already gives keyboard
 * operation, the expanded/collapsed announcement, and find-in-page into
 * collapsed content. A div with an onClick would have to reimplement all
 * three.
 *
 * Units load the first time the panel opens, not on page load — a course with
 * twenty blocks would otherwise fire twenty document requests to render a list
 * nobody has looked at yet.
 */
export function BlockPanel({
  block,
  onEdit,
  onNotice,
}: {
  block: Block;
  onEdit: () => void;
  onNotice: (message: string) => void;
}) {
  const [opened, setOpened] = useState(false);

  // The loader is the gate: while the panel has never been opened it resolves
  // to `null` without touching the network, and opening it changes the loader
  // identity, which is what triggers the one real fetch.
  const loader = useCallback(
    async (): Promise<AdminDocument[] | null> => (opened ? listBlockDocuments(block.id) : null),
    [opened, block.id],
  );
  const { data, loading, error, reload } = useResource(loader, "Could not load this block's units.");

  const units = data ?? [];
  const notLive = units.filter((unit) => unit.status !== "embedded").length;

  function handleUploaded(message: string) {
    onNotice(message);
    reload();
  }

  return (
    <details
      className={`${panelClass} group overflow-hidden`}
      onToggle={(event) => {
        if (event.currentTarget.open) setOpened(true);
      }}
    >
      <summary className="flex cursor-pointer list-none flex-wrap items-center gap-x-3 gap-y-2 px-4 py-3 hover:bg-lavender-50/70 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-inset">
        <span
          aria-hidden="true"
          className="inline-block text-gray-500 transition-transform group-open:rotate-90"
        >
          &rsaquo;
        </span>
        <span className="font-mono text-xs text-gray-500">Block {block.block_no}</span>
        <span className="flex-1 font-medium text-gray-900">{block.title}</span>
        <StatusPill tone={statusTone(block.status)}>{block.status}</StatusPill>
        <span className="text-xs text-gray-500">
          {data ? `${units.length} unit${units.length === 1 ? "" : "s"}` : "Units hidden"}
        </span>
      </summary>

      <div className="space-y-4 border-t border-lavender-200 px-4 py-4">
        {error ? <Banner tone="danger">{error}</Banner> : null}

        {data && notLive > 0 ? (
          <Banner tone="info">
            {notLive} unit{notLive === 1 ? "" : "s"} in this block{" "}
            {notLive === 1 ? "is" : "are"} not live to students yet — only an embedded unit can be
            taught from.
          </Banner>
        ) : null}

        <UnitsTable units={units} loading={loading} />

        <AddUnitForm blockId={block.id} onUploaded={handleUploaded} />

        <div className="flex flex-wrap justify-between gap-2">
          <RowLink href={`/admin/blocks/${block.id}`}>Open full block view</RowLink>
          <AdminButton type="button" variant="outline-light" onClick={onEdit}>
            Edit block
          </AdminButton>
        </div>
      </div>
    </details>
  );
}
