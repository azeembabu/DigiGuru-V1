"use client";

import { DataTable, type Column } from "@/components/admin/DataTable";
import { StatusPill, statusTone } from "@/components/admin/primitives";
import { Blank, formatDate } from "@/components/admin/academic/shared";
import type { AdminDocument, DocumentStatus } from "@/lib/admin/types";

// A "unit" is a `documents` row. There is no separate units table on purpose:
// a unit that did not go through ingestion (sha256 dedupe, OCR scoring,
// `pending_review` gating, `ingestion_jobs`) would never reach the RAG index,
// and the tutor can only teach from what is in that index (NN-4). So the unit
// name is the document title, and the unit's PDF is the ingested document.

/**
 * Ingestion status in words — the pill colour is never the only signal, and
 * `parsing` has no tone of its own in `statusTone`, so the label is the only
 * thing distinguishing it from the queued state.
 */
export const STATUS_LABEL: Record<DocumentStatus, string> = {
  pending: "Queued",
  parsing: "Parsing",
  pending_review: "Awaiting review",
  embedded: "Embedded",
  failed: "Failed",
};

/** A unit is only teachable once its document is embedded. */
export function isLive(status: DocumentStatus): boolean {
  return status === "embedded";
}

/** OCR confidence as a percentage; `null` until the parser has scored it. */
export function confidenceLabel(value: number | null): string | null {
  if (value === null) return null;
  return `${Math.round(value * 100)}% OCR confidence`;
}

/** Why this unit is, or is not, available to students. */
function unitDetail(row: AdminDocument) {
  if (row.status === "failed") {
    // The failure reason lives on the newest ingestion job, not the document
    // row, so a failed unit with no job yet legitimately has nothing to show.
    return (
      <span className="text-[#a3281a]">
        {row.last_error ?? "Ingestion failed."} Not live to students — re-upload the PDF.
      </span>
    );
  }
  if (row.status === "pending_review") {
    const confidence = confidenceLabel(row.ocr_confidence);
    return (
      <span className="text-gray-500">
        {confidence ? `${confidence}. ` : ""}Not live to students until a sub admin approves it.
      </span>
    );
  }
  if (row.status === "embedded") {
    return <span className="text-gray-500">Live to students.</span>;
  }
  return <span className="text-gray-500">Not live to students yet — still ingesting.</span>;
}

export function unitColumns(options: { showUploaded: boolean }): Column<AdminDocument>[] {
  const columns: Column<AdminDocument>[] = [
    {
      key: "title",
      header: "Unit",
      cell: (row) => <span className="font-medium">{row.title}</span>,
    },
    {
      key: "status",
      header: "Status",
      className: "w-[160px]",
      cell: (row) => (
        <StatusPill tone={statusTone(row.status)}>{STATUS_LABEL[row.status]}</StatusPill>
      ),
    },
    {
      key: "pages",
      header: "Pages",
      align: "right",
      className: "w-[80px]",
      cell: (row) =>
        row.page_count > 0 ? <span className="tabular-nums">{row.page_count}</span> : <Blank />,
    },
  ];

  if (options.showUploaded) {
    columns.push({
      key: "uploaded",
      header: "Uploaded",
      className: "w-[140px]",
      cell: (row) => <span className="text-gray-500">{formatDate(row.created_at)}</span>,
    });
  }

  columns.push({
    key: "detail",
    header: "Availability",
    className: "max-w-[320px]",
    cell: unitDetail,
  });

  return columns;
}

/** The unit list, shared by the block panel and the block detail screen. */
export function UnitsTable({
  units,
  loading = false,
  showUploaded = false,
}: {
  units: AdminDocument[];
  loading?: boolean;
  showUploaded?: boolean;
}) {
  return (
    <DataTable
      columns={unitColumns({ showUploaded })}
      rows={units}
      rowKey={(row) => row.id}
      loading={loading}
      empty="No units in this block yet."
      emptyHint="Until a PDF is embedded, the tutor has nothing it is allowed to teach from."
    />
  );
}

/**
 * Client-side checks before a unit upload.
 *
 * `accept="application/pdf"` on the input is a filter, not enforcement — a file
 * can still arrive by drag or by a picker that ignores it — and the gateway
 * sniffs the bytes itself. This just spares the admin an upload that is certain
 * to be rejected.
 */
export function validateUnit(title: string, file: File | null): Record<string, string> {
  const errors: Record<string, string> = {};
  if (title.trim() === "") errors.title = "A unit name is required.";

  if (!file) {
    errors.file = "Choose a PDF to upload.";
    return errors;
  }
  const looksPdf = file.type === "application/pdf" || file.name.toLowerCase().endsWith(".pdf");
  if (!looksPdf) errors.file = "Only PDF files can be ingested.";
  else if (file.size === 0) errors.file = "That file is empty.";
  else if (file.size > MAX_UNIT_BYTES) {
    // Caught here rather than after a long upload that the gateway will reject
    // at the body limit anyway — the admin finds out before waiting, not after.
    errors.file = `That PDF is ${formatBytes(file.size)}. The limit is ${MAX_UNIT_LABEL}.`;
  }
  return errors;
}

/**
 * Largest PDF the gateway will accept, mirroring its upload body limit
 * (`apps/gateway/src/admin/documents.rs`). Keep the two in step: this is a
 * courtesy check so a large file fails instantly instead of after a long
 * upload, never the enforcement.
 */
export const MAX_UNIT_BYTES = 64 * 1024 * 1024;
export const MAX_UNIT_LABEL = "64 MB";

/** The gateway rejects anything over its upload body limit with this code. */
export const TOO_LARGE_CODE = "PAYLOAD_TOO_LARGE";

function formatBytes(bytes: number): string {
  const mb = bytes / (1024 * 1024);
  return mb >= 1 ? `${mb.toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

/** The gateway dedupes by SHA-256, so the same PDF twice is a conflict. */
export const DUPLICATE_CODE = "DOCUMENT_ALREADY_EXISTS";
export const DUPLICATE_MESSAGE = "This PDF has already been uploaded.";
