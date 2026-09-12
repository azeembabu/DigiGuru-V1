"use client";

import { useId, useRef, useState, type FormEvent } from "react";

import { AdminButton, AdminField, AdminInput } from "@/components/admin/controls";
import { Banner, adminControlBorder, adminControlClass } from "@/components/admin/primitives";
import { codeFor, fieldErrorsFor, messageFor } from "@/components/admin/academic/shared";
import {
  DUPLICATE_CODE,
  DUPLICATE_MESSAGE,
  MAX_UNIT_LABEL,
  TOO_LARGE_CODE,
  validateUnit,
} from "@/components/admin/academic/units";
import { uploadDocument } from "@/lib/admin/client";

/**
 * Add a unit to a block: a name and its PDF.
 *
 * Inline rather than in a dialog because it lives inside an expanded block
 * panel — an admin adding unit after unit to the same block should not have a
 * modal open and close between each one.
 *
 * Several of these can be on screen at once (one per open panel), so every
 * control id is namespaced with `useId`.
 */
export function AddUnitForm({
  blockId,
  onUploaded,
}: {
  blockId: string;
  onUploaded: (message: string) => void;
}) {
  const uid = useId();
  const titleId = `${uid}-unit-name`;
  const fileId = `${uid}-unit-file`;

  const [title, setTitle] = useState("");
  const [file, setFile] = useState<File | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const found = validateUnit(title, file);
    setErrors(found);
    setFormError(null);
    if (Object.keys(found).length > 0 || !file) return;

    setPending(true);
    try {
      await uploadDocument(blockId, title.trim(), file);
      setTitle("");
      setFile(null);
      if (fileRef.current) fileRef.current.value = "";
      onUploaded(`Unit “${title.trim()}” uploaded. Ingestion has been queued.`);
    } catch (caught: unknown) {
      // A raw `DOCUMENT_ALREADY_EXISTS` tells an admin nothing actionable.
      const code = codeFor(caught);
      if (code === DUPLICATE_CODE) {
        setErrors({ file: DUPLICATE_MESSAGE });
        setFormError(DUPLICATE_MESSAGE);
      } else if (code === TOO_LARGE_CODE) {
        // The client check above should catch this first; this covers the
        // case where the two limits have drifted apart.
        const tooLarge = `That PDF is larger than the ${MAX_UNIT_LABEL} limit.`;
        setErrors({ file: tooLarge });
        setFormError(tooLarge);
      } else {
        setErrors(fieldErrorsFor(caught));
        setFormError(messageFor(caught, "Could not upload the unit."));
      }
    } finally {
      setPending(false);
    }
  }

  return (
    <form onSubmit={submit} noValidate className="space-y-3 rounded-sm bg-lavender-50/70 p-4">
      <h4 className="text-sm font-semibold text-gray-900">Add unit</h4>

      {formError ? <Banner tone="danger">{formError}</Banner> : null}

      <div className="grid gap-3 sm:grid-cols-2">
        <AdminField id={titleId} label="Unit name" required error={errors.title}>
          <AdminInput
            id={titleId}
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            hasError={Boolean(errors.title)}
            autoComplete="off"
            placeholder="Unit 1 — Early poetry"
          />
        </AdminField>

        <AdminField id={fileId} label="PDF" required error={errors.file} hint={`PDF only, up to ${MAX_UNIT_LABEL}.`}>
          <input
            id={fileId}
            name={fileId}
            ref={fileRef}
            type="file"
            accept="application/pdf"
            aria-invalid={Boolean(errors.file) || undefined}
            aria-describedby={errors.file ? `${fileId}-error` : undefined}
            onChange={(event) => setFile(event.target.files?.[0] ?? null)}
            className={`${adminControlClass} ${adminControlBorder(Boolean(errors.file))} file:mr-3 file:rounded-sm file:border-0 file:bg-lavender-100 file:px-3 file:py-1 file:text-sm file:text-gray-900`}
          />
        </AdminField>
      </div>

      <div className="flex justify-end">
        <AdminButton type="submit" pending={pending} pendingLabel="Uploading…">
          Add unit
        </AdminButton>
      </div>
    </form>
  );
}
