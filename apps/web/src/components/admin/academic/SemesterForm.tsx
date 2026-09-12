"use client";

import { useState } from "react";

import { AdminField, AdminInput, AdminSelect } from "@/components/admin/controls";
import { FormModal, StatusOptions } from "@/components/admin/academic/FormModal";
import {
  fieldErrorsFor,
  lengthRule,
  messageFor,
  rangeRule,
} from "@/components/admin/academic/shared";
import { createSemester, updateSemester } from "@/lib/admin/client";
import type { EntityStatus, Semester } from "@/lib/admin/types";

/**
 * Create/edit a semester of one program.
 *
 * `semester_number` is create-only for the same reason `code` is on a program:
 * `UpdateSemesterRequest` carries only `name` and `status`
 * (`apps/gateway/src/admin/semesters.rs`).
 */
export function SemesterForm({
  programId,
  semester,
  onClose,
  onSaved,
}: {
  programId: string;
  semester: Semester | null;
  onClose: () => void;
  onSaved: (message: string) => void;
}) {
  const [number, setNumber] = useState(semester ? String(semester.semester_number) : "1");
  const [name, setName] = useState(semester?.name ?? "");
  const [status, setStatus] = useState<EntityStatus>(semester?.status ?? "active");
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  function validate(): Record<string, string> {
    const next: Record<string, string> = {};
    if (!semester) {
      const numberError = rangeRule(number, 1, 12, "Semester number");
      if (numberError) next.semester_number = numberError;
    }
    const nameError = lengthRule(name, 2, 100, "Name");
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
      if (semester) {
        await updateSemester(semester.id, { name: name.trim(), status });
        onSaved(`Semester “${name.trim()}” updated.`);
      } else {
        await createSemester({
          program_id: programId,
          semester_number: Number(number),
          name: name.trim(),
        });
        onSaved(`Semester “${name.trim()}” created.`);
      }
    } catch (caught: unknown) {
      setErrors(fieldErrorsFor(caught));
      setFormError(messageFor(caught, "Could not save the semester."));
    } finally {
      setPending(false);
    }
  }

  return (
    <FormModal
      open
      title={semester ? "Edit semester" : "New semester"}
      description={
        semester
          ? "The semester number is fixed once the semester exists."
          : "Semester numbers are unique within a program."
      }
      submitLabel={semester ? "Save changes" : "Create semester"}
      pending={pending}
      error={formError}
      onSubmit={submit}
      onClose={onClose}
    >
      {semester ? null : (
        <AdminField
          id="semester-number"
          label="Semester number"
          required
          error={errors.semester_number}
          hint="1–12."
        >
          <AdminInput
            id="semester-number"
            type="number"
            min={1}
            max={12}
            value={number}
            onChange={(event) => setNumber(event.target.value)}
            hasError={Boolean(errors.semester_number)}
            hasHint
          />
        </AdminField>
      )}

      <AdminField id="semester-name" label="Name" required error={errors.name}>
        <AdminInput
          id="semester-name"
          value={name}
          onChange={(event) => setName(event.target.value)}
          hasError={Boolean(errors.name)}
          autoComplete="off"
          placeholder="Semester 1"
        />
      </AdminField>

      {semester ? (
        <AdminField id="semester-status" label="Status" required error={errors.status}>
          <AdminSelect
            id="semester-status"
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
