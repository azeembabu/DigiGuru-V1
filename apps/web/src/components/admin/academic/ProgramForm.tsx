"use client";

import { useState } from "react";

import { AdminField, AdminInput, AdminSelect, AdminTextarea } from "@/components/admin/controls";
import { FormModal, StatusOptions } from "@/components/admin/academic/FormModal";
import {
  fieldErrorsFor,
  lengthRule,
  messageFor,
  optionalText,
} from "@/components/admin/academic/shared";
import { createProgram, updateProgram } from "@/lib/admin/client";
import type { EntityStatus, Program } from "@/lib/admin/types";

/**
 * Create/edit a program.
 *
 * `code` is write-once by contract, not by choice: `UpdateProgramRequest`
 * carries only `name`, `description`, and `status`
 * (`apps/gateway/src/admin/programs.rs`), so rendering an editable code box in
 * edit mode would promise a save the API cannot perform.
 */
export function ProgramForm({
  program,
  onClose,
  onSaved,
}: {
  /** `null` creates; a program edits it. */
  program: Program | null;
  onClose: () => void;
  onSaved: (message: string) => void;
}) {
  const [code, setCode] = useState(program?.code ?? "");
  const [name, setName] = useState(program?.name ?? "");
  const [description, setDescription] = useState(program?.description ?? "");
  const [status, setStatus] = useState<EntityStatus>(program?.status ?? "active");
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  function validate(): Record<string, string> {
    const next: Record<string, string> = {};
    if (!program) {
      const codeError = lengthRule(code, 2, 20, "Code");
      if (codeError) next.code = codeError;
    }
    const nameError = lengthRule(name, 2, 200, "Name");
    if (nameError) next.name = nameError;
    return next;
  }

  async function submit() {
    const found = validate();
    setErrors(found);
    if (Object.keys(found).length > 0) return;

    setPending(true);
    setFormError(null);
    try {
      if (program) {
        await updateProgram(program.id, {
          name: name.trim(),
          description: optionalText(description),
          status,
        });
        onSaved(`Program “${name.trim()}” updated.`);
      } else {
        await createProgram({
          code: code.trim(),
          name: name.trim(),
          description: optionalText(description),
        });
        onSaved(`Program “${name.trim()}” created.`);
      }
    } catch (caught: unknown) {
      setErrors(fieldErrorsFor(caught));
      setFormError(messageFor(caught, "Could not save the program."));
    } finally {
      setPending(false);
    }
  }

  return (
    <FormModal
      open
      title={program ? "Edit program" : "New program"}
      description={
        program
          ? "The program code is fixed once the program exists."
          : "A program is the top of the hierarchy: Program › Semester › Course › Block."
      }
      submitLabel={program ? "Save changes" : "Create program"}
      pending={pending}
      error={formError}
      onSubmit={submit}
      onClose={onClose}
    >
      {program ? null : (
        <AdminField id="program-code" label="Code" required error={errors.code} hint="2–20 characters, e.g. BA_ML.">
          <AdminInput
            id="program-code"
            value={code}
            onChange={(event) => setCode(event.target.value)}
            hasError={Boolean(errors.code)}
            hasHint
            autoComplete="off"
            placeholder="BA_ML"
          />
        </AdminField>
      )}

      <AdminField id="program-name" label="Name" required error={errors.name}>
        <AdminInput
          id="program-name"
          value={name}
          onChange={(event) => setName(event.target.value)}
          hasError={Boolean(errors.name)}
          autoComplete="off"
          placeholder="BA Malayalam"
        />
      </AdminField>

      <AdminField id="program-description" label="Description" error={errors.description}>
        <AdminTextarea
          id="program-description"
          value={description}
          onChange={(event) => setDescription(event.target.value)}
          hasError={Boolean(errors.description)}
          placeholder="Optional description…"
        />
      </AdminField>

      {program ? (
        <AdminField id="program-status" label="Status" required error={errors.status}>
          <AdminSelect
            id="program-status"
            value={status}
            onChange={(event) =>
              setStatus(event.target.value === "inactive" ? "inactive" : "active")
            }
            hasError={Boolean(errors.status)}
          >
            <StatusOptions />
          </AdminSelect>
        </AdminField>
      ) : null}
    </FormModal>
  );
}
