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
import { createLsc, updateLsc } from "@/lib/admin/client";
import type { EntityStatus, Lsc } from "@/lib/admin/types";

/** Create/edit a learner support centre. Super admins only, server-side. */
export function LscForm({
  lsc,
  onClose,
  onSaved,
}: {
  lsc: Lsc | null;
  onClose: () => void;
  onSaved: (message: string) => void;
}) {
  const [code, setCode] = useState(lsc?.code ?? "");
  const [name, setName] = useState(lsc?.name ?? "");
  const [location, setLocation] = useState(lsc?.location ?? "");
  const [status, setStatus] = useState<EntityStatus>(lsc?.status ?? "active");
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [formError, setFormError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  function validate(): Record<string, string> {
    const next: Record<string, string> = {};
    if (!lsc) {
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
      if (lsc) {
        await updateLsc(lsc.id, {
          name: name.trim(),
          location: optionalText(location),
          status,
        });
        onSaved(`Centre “${name.trim()}” updated.`);
      } else {
        await createLsc({
          code: code.trim(),
          name: name.trim(),
          location: optionalText(location),
        });
        onSaved(`Centre “${name.trim()}” created.`);
      }
    } catch (caught: unknown) {
      setErrors(fieldErrorsFor(caught));
      setFormError(messageFor(caught, "Could not save the centre."));
    } finally {
      setPending(false);
    }
  }

  return (
    <FormModal
      open
      title={lsc ? "Edit centre" : "New centre"}
      description={
        lsc
          ? "The centre code is fixed once the centre exists."
          : "Students pick a centre at signup, so the code and name are student-facing."
      }
      submitLabel={lsc ? "Save changes" : "Create centre"}
      pending={pending}
      error={formError}
      onSubmit={submit}
      onClose={onClose}
    >
      {lsc ? null : (
        <AdminField id="lsc-code" label="Code" required error={errors.code} hint="2–20 characters.">
          <AdminInput
            id="lsc-code"
            value={code}
            onChange={(event) => setCode(event.target.value)}
            hasError={Boolean(errors.code)}
            hasHint
            autoComplete="off"
            placeholder="LSC-KOCHI-01"
          />
        </AdminField>
      )}

      <AdminField id="lsc-name" label="Name" required error={errors.name}>
        <AdminInput
          id="lsc-name"
          value={name}
          onChange={(event) => setName(event.target.value)}
          hasError={Boolean(errors.name)}
          autoComplete="off"
          placeholder="Kochi Centre"
        />
      </AdminField>

      <AdminField id="lsc-location" label="Location" error={errors.location}>
        <AdminTextarea
          id="lsc-location"
          rows={2}
          value={location}
          onChange={(event) => setLocation(event.target.value)}
          hasError={Boolean(errors.location)}
          placeholder="City / address (optional)"
        />
      </AdminField>

      {lsc ? (
        <AdminField id="lsc-status" label="Status" required error={errors.status}>
          <AdminSelect
            id="lsc-status"
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
