"use client";

import { useState } from "react";

import { AdminButton, AdminField, AdminInput, AdminSelect } from "@/components/admin/controls";
import { Modal } from "@/components/admin/Modal";
import { Banner } from "@/components/admin/primitives";
import { createAdmin } from "@/lib/admin/client";
import type { Program, Role } from "@/lib/admin/types";
import { ApiError } from "@/lib/api";

type AdminRole = Exclude<Role, "student">;

/**
 * Create an admin account.
 *
 * `student` is deliberately absent from the role list: the gateway rejects it
 * outright (students exist only through `/auth/signup`), so offering it would
 * be offering a guaranteed failure.
 *
 * Scopes and role are coupled — a super admin is unrestricted by definition
 * and the gateway refuses a non-empty `program_scopes` for one. The picker is
 * therefore disabled *and cleared* when super admin is chosen, with the reason
 * stated rather than left for the admin to infer from a 400.
 */
export function NewAdminModal({
  open,
  programs,
  onClose,
  onCreated,
}: {
  open: boolean;
  programs: Program[];
  onClose: () => void;
  onCreated: () => void;
}) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [fullName, setFullName] = useState("");
  const [role, setRole] = useState<AdminRole>("sub_admin");
  const [scopes, setScopes] = useState<string[]>([]);

  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});

  function reset() {
    setEmail("");
    setPassword("");
    setFullName("");
    setRole("sub_admin");
    setScopes([]);
    setError(null);
    setFieldErrors({});
  }

  function onRoleChange(next: AdminRole) {
    setRole(next);
    // Clearing rather than merely hiding: a hidden-but-retained scope would
    // still be submitted, and rejected.
    if (next === "super_admin") setScopes([]);
  }

  /** Mirrors the gateway's `validator` rules; the gateway is the enforcement. */
  function validate(): Record<string, string> {
    const errors: Record<string, string> = {};
    if (!email.includes("@") || email.trim().length < 3) {
      errors.email = "Enter a valid email address.";
    }
    if (password.length < 8 || password.length > 128) {
      errors.password = "Password must be 8-128 characters.";
    }
    const name = fullName.trim();
    if (name.length < 2 || name.length > 120) {
      errors.full_name = "Name must be 2-120 characters.";
    }
    return errors;
  }

  async function onSubmit(event: React.FormEvent) {
    event.preventDefault();
    const errors = validate();
    if (Object.keys(errors).length > 0) {
      setFieldErrors(errors);
      setError(null);
      return;
    }

    setSaving(true);
    setError(null);
    setFieldErrors({});
    try {
      await createAdmin({
        email: email.trim(),
        password,
        full_name: fullName.trim(),
        role,
        program_scopes: scopes,
      });
      reset();
      onCreated();
    } catch (caught: unknown) {
      if (caught instanceof ApiError) {
        setError(caught.message);
        setFieldErrors(caught.fieldErrors());
      } else {
        setError("Could not create the account. Please try again.");
      }
    } finally {
      setSaving(false);
    }
  }

  function close() {
    reset();
    onClose();
  }

  return (
    <Modal
      open={open}
      title="New admin"
      description="Admin accounts are created here; students register themselves."
      onClose={close}
      footer={
        <>
          <AdminButton type="button" variant="outline-light" disabled={saving} onClick={close}>
            Cancel
          </AdminButton>
          <AdminButton
            type="submit"
            form="new-admin-form"
            pending={saving}
            pendingLabel="Creating..."
          >
            Create admin
          </AdminButton>
        </>
      }
    >
      <form id="new-admin-form" onSubmit={onSubmit} className="space-y-4" noValidate>
        {error ? (
          <Banner tone="danger" onDismiss={() => setError(null)}>
            {error}
          </Banner>
        ) : null}

        <AdminField id="admin_full_name" label="Full name" required error={fieldErrors.full_name}>
          <AdminInput
            id="admin_full_name"
            value={fullName}
            autoComplete="off"
            hasError={Boolean(fieldErrors.full_name)}
            onChange={(event) => setFullName(event.target.value)}
          />
        </AdminField>

        <AdminField id="admin_email" label="Email" required error={fieldErrors.email}>
          <AdminInput
            id="admin_email"
            type="email"
            value={email}
            autoComplete="off"
            hasError={Boolean(fieldErrors.email)}
            onChange={(event) => setEmail(event.target.value)}
          />
        </AdminField>

        <AdminField
          id="admin_password"
          label="Temporary password"
          required
          error={fieldErrors.password}
          hint="8-128 characters. Share it out of band and ask them to change it."
        >
          <AdminInput
            id="admin_password"
            type="password"
            value={password}
            autoComplete="new-password"
            hasError={Boolean(fieldErrors.password)}
            hasHint
            onChange={(event) => setPassword(event.target.value)}
          />
        </AdminField>

        <AdminField id="admin_role" label="Role" required error={fieldErrors.role}>
          <AdminSelect
            id="admin_role"
            value={role}
            hasError={Boolean(fieldErrors.role)}
            onChange={(event) =>
              onRoleChange(event.target.value === "super_admin" ? "super_admin" : "sub_admin")
            }
          >
            <option value="sub_admin">Sub-admin &mdash; limited to assigned programs</option>
            <option value="super_admin">Super admin &mdash; unrestricted</option>
          </AdminSelect>
        </AdminField>

        <fieldset disabled={role === "super_admin"} className="disabled:opacity-60">
          <legend className="text-sm font-medium text-gray-900">Program scopes</legend>
          <p className="mt-1 text-xs text-gray-500">
            {role === "super_admin"
              ? "A super admin already reaches every program, so scopes cannot be set."
              : "A sub-admin sees nothing at all until at least one program is assigned."}
          </p>
          {fieldErrors.program_scopes ? (
            <p role="alert" className="mt-1 text-xs text-danger">
              {fieldErrors.program_scopes}
            </p>
          ) : null}

          <div className="mt-2 max-h-40 space-y-1 overflow-y-auto rounded-sm border border-lavender-200 p-2">
            {programs.length === 0 ? (
              <p className="px-1 py-2 text-xs text-gray-500">No programs exist yet.</p>
            ) : (
              programs.map((program) => (
                <label key={program.id} className="flex items-center gap-2 px-1 py-1 text-sm">
                  <input
                    type="checkbox"
                    className="rounded-sm border-lavender-200 text-lime-500 focus-visible:ring-2 focus-visible:ring-indigo-400 focus-visible:ring-offset-2"
                    checked={scopes.includes(program.id)}
                    onChange={(event) =>
                      setScopes((current) =>
                        event.target.checked
                          ? [...current, program.id]
                          : current.filter((id) => id !== program.id),
                      )
                    }
                  />
                  <span>
                    {program.code} &mdash; {program.name}
                  </span>
                </label>
              ))
            )}
          </div>
        </fieldset>
      </form>
    </Modal>
  );
}
