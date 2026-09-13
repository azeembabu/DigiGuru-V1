"use client";

import Link from "next/link";
import { useRef, useState } from "react";

import { AdminButton, AdminField, AdminSelect, AdminTextarea } from "@/components/admin/controls";
import { Banner, PageHeader, panelClass } from "@/components/admin/primitives";
import { codeFor, messageFor } from "@/components/admin/academic/shared";
import { bulkImportPoolQuestions, type BulkFormat } from "@/lib/admin/client";

// Bulk import (A1.3 channel two, extended by AMENDMENT A2.3 to four formats).
//
// Two things this screen must get right:
//
// 1. The import is ATOMIC on the server — every row is validated before any row
//    is written, and one bad row rejects the file. So there is no
//    "14 of 20 imported" state to render, and the failure is always phrased as
//    "nothing was imported, fix these rows and re-upload".
// 2. `.xlsx` and `.docx` are only read against ONE fixed template. An admin must
//    be told that BEFORE they upload, not discover it from a rejection — so the
//    template is stated on the page beside the format picker, for the chosen
//    format, rather than buried in help text.

/**
 * The column headers every tabular format shares (CSV, XLSX, and the DOCX table).
 *
 * The parser resolves columns by NAME, case-insensitively, so this is the set a
 * file must contain rather than an order it must follow. Stated that way on
 * screen too — an admin who has reordered their spreadsheet should not think
 * they have to rebuild it.
 */
const COLUMNS =
  "course_id,block_id,topic,question_text,option_a,option_b,option_c,option_d,correct_option,explanation,assessment_type,difficulty_level";

const JSON_SAMPLE = `[
  {
    "course_id": "00000000-0000-0000-0000-000000000000",
    "block_id": null,
    "topic": "Prosody",
    "question_text": "Which metre is used here?",
    "options": ["Keka", "Kakali", "Manjari", "Natonnata"],
    "correct_option_index": 0,
    "explanation": "The line scans as Keka because …",
    "assessment_type": "assignment",
    "difficulty_level": "beginner"
  }
]`;

/** Text formats can be pasted; the two Office formats are ZIP containers and
 *  must be uploaded as a file. */
const IS_TEXT: Record<BulkFormat, boolean> = { json: true, csv: true, xlsx: false, docx: false };

const FORMAT_LABEL: Record<BulkFormat, string> = {
  json: "JSON array (.json)",
  csv: "CSV (.csv)",
  xlsx: "Excel workbook (.xlsx)",
  docx: "Word document (.docx)",
};

const ACCEPT = ".csv,.json,.xlsx,.docx";

/**
 * What the file has to look like, per format.
 *
 * Stated up front because for `.xlsx` and `.docx` there is exactly one layout
 * the parser accepts — anything else is refused rather than guessed at, since
 * silently misreading an answer key is far worse than rejecting a file.
 */
function templateFor(format: BulkFormat): { title: string; body: string; sample: string } {
  switch (format) {
    case "json":
      return {
        title: "Expected shape",
        body: "An array of objects. Exactly four options per question. This is the only channel that takes a number: correct_option_index is zero-based, 0–3. course_id is required; block_id is optional and may be null (a course-wide question), and difficulty_level may be omitted for beginner.",
        sample: JSON_SAMPLE,
      };
    case "csv":
      return {
        title: "Required columns — order does not matter",
        body: `One question per row, with this header line first; columns are matched by name, case-insensitively, so you may reorder them. All four options are required. course_id is required; leave block_id blank for a course-wide question and difficulty_level blank for beginner. ${LETTER_RULE}`,
        sample: COLUMNS,
      };
    case "xlsx":
      return {
        title: "Required template — first worksheet, header in row 1",
        body: `Only the first worksheet is read. Row 1 must be the header; one question per row after it, and trailing blank rows are ignored. Do NOT merge header cells — a merged heading duplicates a column name, and a renamed or duplicated column is rejected rather than guessed at. Further worksheets are ignored entirely. ${LETTER_RULE}`,
        sample: COLUMNS,
      };
    case "docx":
      return {
        title: "Required template — the first table in the document",
        body: `The FIRST table in the document is read: its first row must be this header, with one question per row after it. Nothing else in the document is parsed — there is no heading-and-answer-line format. A document without that table is refused outright, because misreading an answer key is worse than refusing a file. If your questions are written as ordinary Word paragraphs, retype them into a table or use .csv/.xlsx instead. ${LETTER_RULE}`,
        sample: COLUMNS,
      };
  }
}

/**
 * The answer-key rule, called out on every tabular format.
 *
 * `correct_option` is a LETTER. A digit is refused outright rather than
 * interpreted, because "1" could mean the first option or the zero-based index 1
 * — and guessing wrong would silently mark wrong answers correct, which is the
 * worst failure this screen can have. Only the JSON channel takes a number, as
 * the explicitly zero-based `correct_option_index`.
 */
const LETTER_RULE =
  "correct_option must be the letter A, B, C or D (case does not matter). A number is rejected, not interpreted — 1 could mean the first option or index 1, and guessing would mark wrong answers correct.";

type Result =
  | { kind: "idle" }
  | { kind: "ok"; imported: number }
  | { kind: "rejected"; summary: string; rows: string[] };

/**
 * Count the rows in a pasted text payload, and catch a file that is not even
 * parseable before it crosses the network.
 *
 * Returns a message rather than throwing: a malformed paste is the normal case
 * on this screen, not an exception. Binary formats are not pre-checked here —
 * reading a ZIP container in the browser to count rows would mean shipping a
 * parser whose disagreement with the server's would be its own bug.
 */
function precheck(body: string, format: BulkFormat): { rows: number } | { error: string } {
  const trimmed = body.trim();
  if (trimmed.length === 0) return { error: "Paste the file contents first." };

  if (format === "json") {
    let parsed: unknown;
    try {
      parsed = JSON.parse(trimmed);
    } catch {
      return { error: "That is not valid JSON. Check for a trailing comma or a missing bracket." };
    }
    if (!Array.isArray(parsed)) {
      return { error: "The JSON must be an array of question objects, not a single object." };
    }
    if (parsed.length === 0) return { error: "The array is empty — there is nothing to import." };
    return { rows: parsed.length };
  }

  const lines = trimmed.split(/\r?\n/).filter((line) => line.trim().length > 0);
  if (lines.length < 2) {
    return { error: "The CSV needs a header row and at least one question row." };
  }
  return { rows: lines.length - 1 };
}

/** Guess the format from a dropped file's extension, so the picker follows the
 *  file instead of making the admin set it twice. The server still sniffs. */
function formatOf(name: string): BulkFormat | null {
  const lower = name.toLowerCase();
  if (lower.endsWith(".json")) return "json";
  if (lower.endsWith(".csv")) return "csv";
  if (lower.endsWith(".xlsx")) return "xlsx";
  if (lower.endsWith(".docx")) return "docx";
  return null;
}

/**
 * Split the gateway's joined validation message into one line per row.
 *
 * `PublicError::Validation` joins its field errors into a single string
 * (`"field: message; field2: message"` — see `lib/api.ts`), and the bulk
 * endpoint puts the row numbers in there. Splitting it back out is what turns
 * one unreadable paragraph into a checklist an admin can work down.
 */
function splitRows(message: string): string[] {
  return message
    .split(/;\s+/)
    .map((part) => part.trim())
    .filter((part) => part.length > 0);
}

export function BulkImportScreen() {
  const [format, setFormat] = useState<BulkFormat>("json");
  const [body, setBody] = useState("");
  const [file, setFile] = useState<File | null>(null);
  const [pending, setPending] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const [result, setResult] = useState<Result>({ kind: "idle" });
  const fileRef = useRef<HTMLInputElement>(null);

  const isText = IS_TEXT[format];
  const template = templateFor(format);

  /** Clear the inputs. Deliberately leaves `result` alone, so the success or
   *  rejection of the import that just ran stays on screen. */
  function clearInputs() {
    setBody("");
    setFile(null);
    setLocalError(null);
    if (fileRef.current) fileRef.current.value = "";
  }

  function onPickFile(picked: File | null) {
    setLocalError(null);
    setResult({ kind: "idle" });
    setFile(picked);
    if (picked === null) return;

    const detected = formatOf(picked.name);
    if (detected === null) {
      setLocalError("That file type is not accepted. Use .csv, .json, .xlsx or .docx.");
      setFile(null);
      return;
    }
    setFormat(detected);
    // A text file picked through the browser is still sent as a file — there is
    // no need to read it into the textarea, and doing so would double the
    // memory for a large CSV.
  }

  async function onSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (pending) return;

    let payload: string | File;

    if (file !== null) {
      payload = file;
    } else if (isText) {
      const checked = precheck(body, format);
      if ("error" in checked) {
        setLocalError(checked.error);
        setResult({ kind: "idle" });
        return;
      }
      payload = body;
    } else {
      setLocalError(`Choose a ${format.toUpperCase()} file — this format cannot be pasted.`);
      return;
    }

    setLocalError(null);
    setResult({ kind: "idle" });
    setPending(true);

    try {
      const { imported } = await bulkImportPoolQuestions(payload, format);
      clearInputs();
      setResult({ kind: "ok", imported });
    } catch (caught: unknown) {
      const message = messageFor(caught, "The import was rejected.");
      const rows = codeFor(caught) === "VALIDATION_ERROR" ? splitRows(message) : [];
      setResult({
        kind: "rejected",
        summary:
          rows.length > 0
            ? "Nothing was imported. Fix every row listed below and re-upload the whole file."
            : message,
        rows,
      });
    } finally {
      setPending(false);
    }
  }

  const checked = isText && file === null && body.trim().length > 0 ? precheck(body, format) : null;
  const rowCount = checked !== null && "rows" in checked ? checked.rows : null;

  return (
    <div className="space-y-6">
      <PageHeader
        title="Bulk import questions"
        description="Upload a CSV, JSON, Excel or Word file. Every row is validated before any row is written — one bad row rejects the whole file, so nothing is ever half-loaded. In CSV, Excel and Word the correct answer is a letter (A–D); a digit is rejected rather than guessed at."
        action={
          <Link
            href="/admin/question-pool"
            className="text-sm font-medium text-indigo-500 underline-offset-2 hover:underline"
          >
            Back to the pool
          </Link>
        }
      />

      {result.kind === "ok" ? (
        <Banner tone="success" onDismiss={() => setResult({ kind: "idle" })}>
          {result.imported} question{result.imported === 1 ? "" : "s"} imported.
        </Banner>
      ) : null}

      {result.kind === "rejected" ? (
        <div
          role="alert"
          className="rounded-sm border border-danger/40 bg-danger/10 px-4 py-3 text-sm text-[#a3281a]"
        >
          <p className="font-semibold">Import rejected — nothing was written</p>
          <p className="mt-1">{result.summary}</p>
          {result.rows.length > 0 ? (
            <ul className="mt-3 space-y-1 border-t border-danger/30 pt-3 font-mono text-xs">
              {result.rows.map((row, index) => (
                <li key={index}>• {row}</li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}

      <form onSubmit={onSubmit} className={`${panelClass} space-y-5 p-5`} noValidate>
        <div className="grid gap-4 sm:grid-cols-[220px_1fr] sm:items-start">
          <AdminField id="format" label="Format" required>
            <AdminSelect
              id="format"
              value={format}
              onChange={(e) => {
                setFormat(e.target.value as BulkFormat);
                setLocalError(null);
              }}
            >
              {(Object.keys(FORMAT_LABEL) as BulkFormat[]).map((value) => (
                <option key={value} value={value}>
                  {FORMAT_LABEL[value]}
                </option>
              ))}
            </AdminSelect>
          </AdminField>

          {/* The template, for the format currently selected. Shown before the
              upload control on purpose: for .xlsx and .docx there is exactly one
              accepted layout, and an admin must know that before they pick a
              file rather than after a rejection. */}
          <div className="rounded-sm border border-lavender-200 bg-lavender-50/60 p-3 text-xs text-gray-500">
            <p className="font-semibold text-gray-900">{template.title}</p>
            <p className="mt-1">{template.body}</p>
            <pre className="mt-2 overflow-x-auto whitespace-pre font-mono text-[11px] leading-[1.6] text-gray-900">
              {template.sample}
            </pre>
          </div>
        </div>

        <AdminField
          id="file"
          label="Upload a file"
          required={!isText}
          hint={
            isText
              ? "Or paste the contents below instead."
              : "Excel and Word files must be uploaded — they cannot be pasted."
          }
        >
          <input
            ref={fileRef}
            id="file"
            name="file"
            type="file"
            accept={ACCEPT}
            onChange={(e) => onPickFile(e.target.files?.[0] ?? null)}
            className="block w-full text-sm text-gray-900 file:mr-3 file:rounded-full file:border-0 file:bg-lime-500 file:px-4 file:py-2 file:text-sm file:font-semibold file:text-ink-950 hover:file:bg-lime-400"
          />
        </AdminField>

        {file !== null ? (
          <p className="text-sm text-gray-500">
            Ready to upload <span className="font-mono text-gray-900">{file.name}</span> (
            {Math.max(1, Math.round(file.size / 1024))} KB) as {format.toUpperCase()}.
          </p>
        ) : isText ? (
          <AdminField
            id="payload"
            label="Or paste the file contents"
            error={localError ?? undefined}
            hint={
              rowCount === null
                ? "Paste the whole file, including the CSV header row."
                : `${rowCount} question row${rowCount === 1 ? "" : "s"} detected.`
            }
          >
            <AdminTextarea
              id="payload"
              value={body}
              rows={14}
              spellCheck={false}
              hasError={Boolean(localError)}
              hasHint
              className="font-mono text-xs"
              onChange={(e) => {
                setBody(e.target.value);
                setLocalError(null);
              }}
            />
          </AdminField>
        ) : null}

        {localError !== null && (file !== null || !isText) ? (
          <p role="alert" className="text-sm text-danger">
            {localError}
          </p>
        ) : null}

        <div className="flex flex-wrap items-center gap-3 border-t border-lavender-200 pt-5">
          <AdminButton type="submit" pending={pending} pendingLabel="Validating and importing…">
            Import {rowCount === null ? "questions" : `${rowCount} question${rowCount === 1 ? "" : "s"}`}
          </AdminButton>
          <AdminButton
            type="button"
            variant="outline-light"
            onClick={() => {
              clearInputs();
              setResult({ kind: "idle" });
            }}
          >
            Clear
          </AdminButton>
        </div>
      </form>
    </div>
  );
}
